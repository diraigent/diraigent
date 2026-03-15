//! Composite task scoring for priority ordering.
//!
//! Tasks are ranked by a composite score rather than a single priority number.
//! The score combines multiple components — currently age (staleness) and
//! manual priority — so that stale tasks eventually bubble up even if their
//! static priority is lower.
//!
//! # Weights
//!
//! Each component has a configurable weight. Defaults:
//!
//! | Component | Weight | Formula |
//! |-----------|--------|---------|
//! | age       | 1.0    | `days_since_state_entered * weight` |
//! | priority  | 2.0    | `(6 - priority) * weight` |
//!
//! Override via environment variables `SCORE_WEIGHT_AGE` and
//! `SCORE_WEIGHT_PRIORITY`, or construct [`ScoreWeights`] directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configurable weights for each score component.
///
/// Defaults:
/// - `age_weight`: **1.0** — each day in the current state adds 1.0 to the score.
/// - `priority_weight`: **2.0** — priority-1 (critical) contributes 10.0,
///   priority-5 (lowest) contributes 2.0.
///
/// Override via env vars `SCORE_WEIGHT_AGE` / `SCORE_WEIGHT_PRIORITY` or by
/// constructing the struct directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreWeights {
    /// Multiplier applied to the age component (days since state entry).
    pub age_weight: f64,
    /// Multiplier applied to the priority component `(6 - priority)`.
    pub priority_weight: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            age_weight: 1.0,
            priority_weight: 2.0,
        }
    }
}

impl ScoreWeights {
    /// Build weights from environment variables, falling back to defaults.
    ///
    /// - `SCORE_WEIGHT_AGE` → `age_weight` (default 1.0)
    /// - `SCORE_WEIGHT_PRIORITY` → `priority_weight` (default 2.0)
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
/// | Component | Formula |
/// |-----------|---------|
/// | age       | `max(0, days_since_state_entered) * weights.age_weight` |
/// | priority  | `(6 - priority).clamp(0, 5) * weights.priority_weight` |
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

    let total = age_score + priority_score;

    TaskScore {
        total,
        age_score,
        priority_score,
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

        let component_sum = score.age_score + score.priority_score;
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
    }
}
