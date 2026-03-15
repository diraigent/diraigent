//! Composite task scoring for priority ordering.
//!
//! Tasks are ranked by a composite score rather than a single priority number.
//! The score combines multiple components — age (staleness), manual priority,
//! goal alignment, and dependency graph position — so that stale tasks,
//! goal-aligned tasks, and critical-path tasks bubble up even if their static
//! priority is lower.
//!
//! # Weights
//!
//! Each component has a configurable weight. Defaults:
//!
//! | Component    | Weight | Formula |
//! |--------------|--------|---------|
//! | age          | 1.0    | `days_since_state_entered * weight` |
//! | priority     | 2.0    | `(6 - priority) * weight` |
//! | goal         | 1.0    | `sum((6 - goal_priority) for each active goal) * weight` |
//! | dependency   | 1.0    | `(blocking_count * 1.5 - blocked_by_count * 3.0) * weight` |
//!
//! Override via environment variables `SCORE_WEIGHT_AGE`,
//! `SCORE_WEIGHT_PRIORITY`, `SCORE_WEIGHT_GOAL`, and `SCORE_WEIGHT_DEPENDENCY`,
//! or construct [`ScoreWeights`] directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configurable weights for each score component.
///
/// Defaults:
/// - `age_weight`: **1.0** — each day in the current state adds 1.0 to the score.
/// - `priority_weight`: **2.0** — priority-1 (critical) contributes 10.0,
///   priority-5 (lowest) contributes 2.0.
/// - `goal_weight`: **1.0** — each active goal contributes `(6 - goal_priority)`
///   to the score.
/// - `dependency_weight`: **1.0** — critical-path tasks (blocking others) get a
///   bonus; blocked tasks get a penalty.
///
/// Override via env vars `SCORE_WEIGHT_AGE` / `SCORE_WEIGHT_PRIORITY` /
/// `SCORE_WEIGHT_GOAL` / `SCORE_WEIGHT_DEPENDENCY` or by constructing the
/// struct directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreWeights {
    /// Multiplier applied to the age component (days since state entry).
    pub age_weight: f64,
    /// Multiplier applied to the priority component `(6 - priority)`.
    pub priority_weight: f64,
    /// Multiplier applied to the goal-alignment component.
    pub goal_weight: f64,
    /// Multiplier applied to the dependency component.
    pub dependency_weight: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            age_weight: 1.0,
            priority_weight: 2.0,
            goal_weight: 1.0,
            dependency_weight: 1.0,
        }
    }
}

impl ScoreWeights {
    /// Build weights from environment variables, falling back to defaults.
    ///
    /// - `SCORE_WEIGHT_AGE`        → `age_weight`        (default 1.0)
    /// - `SCORE_WEIGHT_PRIORITY`   → `priority_weight`   (default 2.0)
    /// - `SCORE_WEIGHT_GOAL`       → `goal_weight`       (default 1.0)
    /// - `SCORE_WEIGHT_DEPENDENCY` → `dependency_weight` (default 1.0)
    pub fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            age_weight: std::env::var("SCORE_WEIGHT_AGE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults.age_weight),
            priority_weight: std::env::var("SCORE_WEIGHT_PRIORITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults.priority_weight),
            goal_weight: std::env::var("SCORE_WEIGHT_GOAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults.goal_weight),
            dependency_weight: std::env::var("SCORE_WEIGHT_DEPENDENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults.dependency_weight),
        }
    }
}

/// Composite score for a single task, broken down by component.
///
/// Higher scores mean higher scheduling priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskScore {
    /// Sum of all component scores.
    pub total: f64,
    /// Score contribution from task age (days in current state).
    pub age_score: f64,
    /// Score contribution from manual priority level.
    pub priority_score: f64,
    /// Score contribution from goal alignment.
    ///
    /// Tasks linked to active, high-priority goals receive a bonus.
    /// Formula: `sum((6 - goal_priority).clamp(0, 5)) * goal_weight` for each
    /// active goal. Tasks with no goals or only done/cancelled goals score 0.
    pub goal_score: f64,
    /// Score contribution from dependency graph position.
    ///
    /// Positive when the task is blocking other tasks (critical path).
    /// Negative when the task itself has unresolved blockers.
    pub dependency_score: f64,
}

/// Input data needed to compute a task's score.
///
/// Kept separate from the DB `Task` model so that `compute_score` remains a
/// pure function with no database dependency.
#[derive(Debug, Clone)]
pub struct TaskScoreInput {
    /// When the task entered its current state. If unavailable, fall back to
    /// `created_at` or `updated_at`.
    pub state_entered_at: DateTime<Utc>,
    /// Manual priority level (1 = critical … 5 = lowest). Tasks without a
    /// priority column should default to 3 (medium).
    pub priority: i32,
    /// Priorities of active goals linked to this task.
    ///
    /// The caller is responsible for filtering out goals with inactive statuses
    /// (e.g. "achieved", "abandoned") before passing them in. Only priorities
    /// from active goals should be included. An empty vec means the task has no
    /// active goal linkages.
    pub goal_priorities: Vec<i32>,
    /// Number of downstream tasks that are waiting on this task (i.e. this
    /// task appears in their `depends_on` column). Higher values mean this
    /// task is on the critical path.
    pub blocking_count: u32,
    /// Number of unresolved upstream blockers (dependencies not in `done` or
    /// `cancelled` state). Higher values mean this task cannot proceed yet.
    pub blocked_by_count: u32,
}

/// Compute a composite score for a task.
///
/// This is a **pure function** — no database access, no side-effects.
/// All required data is passed explicitly via [`TaskScoreInput`].
///
/// # Arguments
///
/// * `input` — the task data needed for scoring.
/// * `now`   — the reference timestamp (typically `Utc::now()`).
/// * `weights` — per-component weight multipliers.
///
/// # Score components
///
/// | Component    | Formula |
/// |--------------|---------|
/// | age          | `max(0, days_since_state_entered) * weights.age_weight` |
/// | priority     | `(6 - priority).clamp(0, 5) * weights.priority_weight` |
/// | goal         | `sum((6 - goal_priority).clamp(0, 5)) * weights.goal_weight` |
/// | dependency   | `(blocking_count * 1.5 - blocked_by_count * 3.0) * weights.dependency_weight` |
///
/// The `total` is the sum of all components.
pub fn compute_score(
    input: &TaskScoreInput,
    now: DateTime<Utc>,
    weights: &ScoreWeights,
) -> TaskScore {
    // Age: fractional days since the task entered its current state.
    let age_days = (now - input.state_entered_at).num_seconds().max(0) as f64 / 86_400.0;
    let age_score = age_days * weights.age_weight;

    // Priority: higher is better. priority=1 → 5 points, priority=5 → 1 point.
    let priority_raw = (6 - input.priority).clamp(0, 5) as f64;
    let priority_score = priority_raw * weights.priority_weight;

    // Goal alignment: sum of (6 - goal_priority) for each active goal.
    // Only active goal priorities should be present in the input; the caller
    // filters out done/cancelled goals.
    let goal_raw: f64 = input
        .goal_priorities
        .iter()
        .map(|&p| (6 - p).clamp(0, 5) as f64)
        .sum();
    let goal_score = goal_raw * weights.goal_weight;

    // Dependency: tasks on the critical path (blocking others) get a bonus;
    // tasks with unresolved blockers get a penalty.
    let dependency_raw =
        (input.blocking_count as f64 * 1.5) - (input.blocked_by_count as f64 * 3.0);
    let dependency_score = dependency_raw * weights.dependency_weight;

    let total = age_score + priority_score + goal_score + dependency_score;

    TaskScore {
        total,
        age_score,
        priority_score,
        goal_score,
        dependency_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn default_weights() -> ScoreWeights {
        ScoreWeights::default()
    }

    fn make_input(state_entered_at: DateTime<Utc>, priority: i32) -> TaskScoreInput {
        TaskScoreInput {
            state_entered_at,
            priority,
            goal_priorities: vec![],
            blocking_count: 0,
            blocked_by_count: 0,
        }
    }

    fn make_input_with_goals(
        state_entered_at: DateTime<Utc>,
        priority: i32,
        goal_priorities: Vec<i32>,
    ) -> TaskScoreInput {
        TaskScoreInput {
            state_entered_at,
            priority,
            goal_priorities,
            blocking_count: 0,
            blocked_by_count: 0,
        }
    }

    fn make_input_with_deps(
        state_entered_at: DateTime<Utc>,
        priority: i32,
        blocking_count: u32,
        blocked_by_count: u32,
    ) -> TaskScoreInput {
        TaskScoreInput {
            state_entered_at,
            priority,
            goal_priorities: vec![],
            blocking_count,
            blocked_by_count,
        }
    }

    #[test]
    fn test_fresh_task_scores_by_priority_only() {
        let now = Utc::now();
        let input = make_input(now, 3);
        let score = compute_score(&input, now, &default_weights());

        // age_score should be ~0 (same instant)
        assert!(score.age_score < 0.001, "age_score should be ~0");
        // priority_score = (6 - 3) * 2.0 = 6.0
        assert!((score.priority_score - 6.0).abs() < 0.001);
        assert!((score.total - 6.0).abs() < 0.001);
    }

    #[test]
    fn test_priority_1_scores_higher_than_priority_5() {
        let now = Utc::now();
        let high = make_input(now, 1);
        let low = make_input(now, 5);
        let weights = default_weights();

        let score_high = compute_score(&high, now, &weights);
        let score_low = compute_score(&low, now, &weights);

        assert!(
            score_high.total > score_low.total,
            "priority-1 ({}) should outscore priority-5 ({})",
            score_high.total,
            score_low.total
        );
        // priority_1: (6-1)*2 = 10, priority_5: (6-5)*2 = 2
        assert!((score_high.priority_score - 10.0).abs() < 0.001);
        assert!((score_low.priority_score - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_stale_task_outscores_fresh_same_priority() {
        let now = Utc::now();
        let stale = make_input(now - Duration::days(10), 3);
        let fresh = make_input(now, 3);
        let weights = default_weights();

        let score_stale = compute_score(&stale, now, &weights);
        let score_fresh = compute_score(&fresh, now, &weights);

        assert!(
            score_stale.total > score_fresh.total,
            "10-day stale task ({}) should outscore fresh task ({})",
            score_stale.total,
            score_fresh.total
        );
        // Stale age_score should be ~10.0
        assert!(
            (score_stale.age_score - 10.0).abs() < 0.01,
            "age_score should be ~10.0, got {}",
            score_stale.age_score
        );
    }

    #[test]
    fn test_age_eventually_outweighs_priority_difference() {
        // A priority-5 task that has been stale for 20 days should eventually
        // outscore a fresh priority-1 task.
        //
        // Fresh priority-1: age=0, priority=(6-1)*2=10 → total=10
        // 20-day stale priority-5: age=20*1=20, priority=(6-5)*2=2 → total=22
        let now = Utc::now();
        let stale_low = make_input(now - Duration::days(20), 5);
        let fresh_high = make_input(now, 1);
        let weights = default_weights();

        let score_stale = compute_score(&stale_low, now, &weights);
        let score_fresh = compute_score(&fresh_high, now, &weights);

        assert!(
            score_stale.total > score_fresh.total,
            "20-day stale priority-5 ({}) should outscore fresh priority-1 ({})",
            score_stale.total,
            score_fresh.total
        );
    }

    #[test]
    fn test_custom_weights() {
        let now = Utc::now();
        let input = make_input(now - Duration::days(5), 2);
        let weights = ScoreWeights {
            age_weight: 3.0,
            priority_weight: 1.0,
            goal_weight: 1.0,
            dependency_weight: 1.0,
        };

        let score = compute_score(&input, now, &weights);

        // age_score = 5 * 3.0 = 15.0
        assert!(
            (score.age_score - 15.0).abs() < 0.01,
            "age_score: expected ~15.0, got {}",
            score.age_score
        );
        // priority_score = (6-2) * 1.0 = 4.0
        assert!(
            (score.priority_score - 4.0).abs() < 0.001,
            "priority_score: expected 4.0, got {}",
            score.priority_score
        );
        assert!(
            (score.total - 19.0).abs() < 0.01,
            "total: expected ~19.0, got {}",
            score.total
        );
    }

    #[test]
    fn test_zero_weights_produce_zero_score() {
        let now = Utc::now();
        let input = make_input(now - Duration::days(30), 1);
        let weights = ScoreWeights {
            age_weight: 0.0,
            priority_weight: 0.0,
            goal_weight: 0.0,
            dependency_weight: 0.0,
        };

        let score = compute_score(&input, now, &weights);
        assert!(
            score.total.abs() < 0.001,
            "zero weights should give zero score"
        );
    }

    #[test]
    fn test_future_state_entered_at_clamps_to_zero() {
        // If state_entered_at is in the future (clock skew), age should be 0.
        let now = Utc::now();
        let input = make_input(now + Duration::hours(1), 3);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            score.age_score.abs() < 0.001,
            "future state_entered_at should clamp age to 0, got {}",
            score.age_score
        );
    }

    #[test]
    fn test_components_sum_to_total() {
        let now = Utc::now();
        let input = make_input(now - Duration::days(7), 2);
        let score = compute_score(&input, now, &default_weights());

        let component_sum =
            score.age_score + score.priority_score + score.goal_score + score.dependency_score;
        assert!(
            (score.total - component_sum).abs() < 0.001,
            "total ({}) should equal sum of components ({})",
            score.total,
            component_sum
        );
    }

    #[test]
    fn test_components_sum_to_total_with_goals() {
        let now = Utc::now();
        let input = make_input_with_goals(now - Duration::days(3), 2, vec![1, 3]);
        let score = compute_score(&input, now, &default_weights());

        let component_sum =
            score.age_score + score.priority_score + score.goal_score + score.dependency_score;
        assert!(
            (score.total - component_sum).abs() < 0.001,
            "total ({}) should equal sum of components ({})",
            score.total,
            component_sum
        );
    }

    #[test]
    fn test_default_priority_3_medium() {
        // Default priority (3) with default weights should give priority_score = 6.0
        let now = Utc::now();
        let input = make_input(now, 3);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            (score.priority_score - 6.0).abs() < 0.001,
            "priority-3 should score 6.0, got {}",
            score.priority_score
        );
    }

    #[test]
    fn test_from_env_fallback() {
        // Without env vars set, from_env should return defaults
        let weights = ScoreWeights::from_env();
        let defaults = ScoreWeights::default();
        assert!(
            (weights.age_weight - defaults.age_weight).abs() < 0.001,
            "from_env age_weight should match default"
        );
        assert!(
            (weights.priority_weight - defaults.priority_weight).abs() < 0.001,
            "from_env priority_weight should match default"
        );
        assert!(
            (weights.goal_weight - defaults.goal_weight).abs() < 0.001,
            "from_env goal_weight should match default"
        );
        assert!(
            (weights.dependency_weight - defaults.dependency_weight).abs() < 0.001,
            "from_env dependency_weight should match default"
        );
    }

    // ── Goal-alignment scoring tests ───────────────────────────────────────

    #[test]
    fn test_no_goals_gives_zero_goal_score() {
        let now = Utc::now();
        let input = make_input(now, 3); // no goals
        let score = compute_score(&input, now, &default_weights());

        assert!(
            score.goal_score.abs() < 0.001,
            "task with no goals should have goal_score = 0, got {}",
            score.goal_score
        );
    }

    #[test]
    fn test_one_active_goal_priority_1() {
        // A single active goal with priority 1 (highest).
        // goal_score = (6 - 1) * 1.0 = 5.0
        let now = Utc::now();
        let input = make_input_with_goals(now, 3, vec![1]);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            (score.goal_score - 5.0).abs() < 0.001,
            "one active priority-1 goal should give goal_score = 5.0, got {}",
            score.goal_score
        );
    }

    #[test]
    fn test_goal_linked_task_scores_higher_than_unlinked() {
        // Same task, same priority, same age — but one has an active goal.
        let now = Utc::now();
        let linked = make_input_with_goals(now, 3, vec![1]);
        let unlinked = make_input(now, 3);
        let weights = default_weights();

        let score_linked = compute_score(&linked, now, &weights);
        let score_unlinked = compute_score(&unlinked, now, &weights);

        assert!(
            score_linked.total > score_unlinked.total,
            "goal-linked task ({}) should outscore unlinked task ({})",
            score_linked.total,
            score_unlinked.total
        );
    }

    #[test]
    fn test_multiple_active_goals() {
        // Two active goals: priority 1 and priority 3.
        // goal_score = (6-1) + (6-3) = 5 + 3 = 8.0
        let now = Utc::now();
        let input = make_input_with_goals(now, 3, vec![1, 3]);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            (score.goal_score - 8.0).abs() < 0.001,
            "two active goals (p1, p3) should give goal_score = 8.0, got {}",
            score.goal_score
        );
    }

    #[test]
    fn test_inactive_goals_contribute_zero() {
        // When the caller filters out done/cancelled goals, goal_priorities
        // is empty even if the task was previously linked.
        let now = Utc::now();
        let input = make_input_with_goals(now, 3, vec![]); // all goals inactive → empty
        let score = compute_score(&input, now, &default_weights());

        assert!(
            score.goal_score.abs() < 0.001,
            "task with only inactive goals should have goal_score = 0, got {}",
            score.goal_score
        );
    }

    #[test]
    fn test_goal_priority_5_gives_minimal_bonus() {
        // goal_score = (6 - 5) * 1.0 = 1.0
        let now = Utc::now();
        let input = make_input_with_goals(now, 3, vec![5]);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            (score.goal_score - 1.0).abs() < 0.001,
            "priority-5 goal should give goal_score = 1.0, got {}",
            score.goal_score
        );
    }

    #[test]
    fn test_goal_score_with_custom_weight() {
        // goal_score = (6 - 2) * 2.5 = 10.0
        let now = Utc::now();
        let input = make_input_with_goals(now, 3, vec![2]);
        let weights = ScoreWeights {
            age_weight: 1.0,
            priority_weight: 2.0,
            goal_weight: 2.5,
            dependency_weight: 1.0,
        };
        let score = compute_score(&input, now, &weights);

        assert!(
            (score.goal_score - 10.0).abs() < 0.001,
            "priority-2 goal with weight 2.5 should give goal_score = 10.0, got {}",
            score.goal_score
        );
    }

    #[test]
    fn test_goal_priority_clamped_to_valid_range() {
        // Out-of-range priority (0 or negative) should be clamped.
        // (6 - 0).clamp(0, 5) = 5
        let now = Utc::now();
        let input = make_input_with_goals(now, 3, vec![0]);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            (score.goal_score - 5.0).abs() < 0.001,
            "priority-0 goal should clamp to 5.0, got {}",
            score.goal_score
        );

        // Very high priority (e.g. 10): (6 - 10).clamp(0, 5) = 0
        let input_high = make_input_with_goals(now, 3, vec![10]);
        let score_high = compute_score(&input_high, now, &default_weights());

        assert!(
            score_high.goal_score.abs() < 0.001,
            "priority-10 goal should clamp to 0.0, got {}",
            score_high.goal_score
        );
    }

    // ── Dependency scoring tests ───────────────────────────────────────────

    #[test]
    fn test_isolated_task_has_zero_dependency_score() {
        // A task with no dependencies in either direction gets 0 dependency_score.
        let now = Utc::now();
        let input = make_input_with_deps(now, 3, 0, 0);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            score.dependency_score.abs() < 0.001,
            "isolated task should have dependency_score ~0, got {}",
            score.dependency_score
        );
    }

    #[test]
    fn test_blocking_task_scores_higher_than_isolated() {
        // A task blocking 2 others should score higher than an equivalent
        // isolated task (same priority, same age).
        let now = Utc::now();
        let blocking = make_input_with_deps(now, 3, 2, 0);
        let isolated = make_input_with_deps(now, 3, 0, 0);
        let weights = default_weights();

        let score_blocking = compute_score(&blocking, now, &weights);
        let score_isolated = compute_score(&isolated, now, &weights);

        assert!(
            score_blocking.total > score_isolated.total,
            "blocking task ({}) should outscore isolated task ({})",
            score_blocking.total,
            score_isolated.total
        );
        // dependency_score = 2 * 1.5 * 1.0 = 3.0
        assert!(
            (score_blocking.dependency_score - 3.0).abs() < 0.001,
            "blocking 2 tasks should give dependency_score 3.0, got {}",
            score_blocking.dependency_score
        );
    }

    #[test]
    fn test_blocked_task_scores_lower_than_isolated() {
        // A task with 1 unresolved blocker should score lower than an
        // equivalent unblocked (isolated) task.
        let now = Utc::now();
        let blocked = make_input_with_deps(now, 3, 0, 1);
        let isolated = make_input_with_deps(now, 3, 0, 0);
        let weights = default_weights();

        let score_blocked = compute_score(&blocked, now, &weights);
        let score_isolated = compute_score(&isolated, now, &weights);

        assert!(
            score_blocked.total < score_isolated.total,
            "blocked task ({}) should score lower than isolated task ({})",
            score_blocked.total,
            score_isolated.total
        );
        // dependency_score = 0 * 1.5 - 1 * 3.0 = -3.0
        assert!(
            (score_blocked.dependency_score - (-3.0)).abs() < 0.001,
            "1 unresolved blocker should give dependency_score -3.0, got {}",
            score_blocked.dependency_score
        );
    }

    #[test]
    fn test_blocking_and_blocked_combined() {
        // A task that is both blocking 1 downstream task and blocked by 1
        // upstream task: (1 * 1.5) - (1 * 3.0) = -1.5
        let now = Utc::now();
        let input = make_input_with_deps(now, 3, 1, 1);
        let score = compute_score(&input, now, &default_weights());

        assert!(
            (score.dependency_score - (-1.5)).abs() < 0.001,
            "blocking=1, blocked_by=1 should give -1.5, got {}",
            score.dependency_score
        );
    }

    #[test]
    fn test_dependency_components_sum_to_total() {
        // Verify total includes the dependency component.
        let now = Utc::now();
        let input = make_input_with_deps(now - Duration::days(3), 2, 4, 1);
        let score = compute_score(&input, now, &default_weights());

        let component_sum =
            score.age_score + score.priority_score + score.goal_score + score.dependency_score;
        assert!(
            (score.total - component_sum).abs() < 0.001,
            "total ({}) should equal sum of components ({})",
            score.total,
            component_sum
        );
        // dependency_score = (4 * 1.5) - (1 * 3.0) = 6.0 - 3.0 = 3.0
        assert!(
            (score.dependency_score - 3.0).abs() < 0.001,
            "dependency_score should be 3.0, got {}",
            score.dependency_score
        );
    }

    #[test]
    fn test_dependency_weight_scales_score() {
        // Custom dependency weight of 2.0 should double the dependency component.
        let now = Utc::now();
        let input = make_input_with_deps(now, 3, 2, 0);
        let weights = ScoreWeights {
            age_weight: 0.0,
            priority_weight: 0.0,
            goal_weight: 0.0,
            dependency_weight: 2.0,
        };

        let score = compute_score(&input, now, &weights);

        // dependency_score = (2 * 1.5) * 2.0 = 6.0
        assert!(
            (score.dependency_score - 6.0).abs() < 0.001,
            "dependency_score with weight 2.0 should be 6.0, got {}",
            score.dependency_score
        );
        assert!(
            (score.total - 6.0).abs() < 0.001,
            "total should be 6.0 (only dependency), got {}",
            score.total
        );
    }
}
