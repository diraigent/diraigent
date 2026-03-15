//! Composite task scoring for ready-task ordering.
//!
//! Tasks are ranked by a weighted combination of:
//! - **age**: how long the task has existed (days since `created_at`)
//! - **urgent**: boolean urgent flag contributes a flat bonus
//! - **dependency**: tasks that block other tasks get a bonus (critical-path)
//! - **work**: tasks linked to active, high-priority work items get a bonus

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-component score breakdown for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskScore {
    /// Final composite score (sum of all components).
    pub total: f64,
    /// Score from task age (days since created_at).
    pub age_score: f64,
    /// Score from the urgent flag.
    pub urgent_score: f64,
    /// Score from blocking downstream tasks (critical-path bonus).
    pub dependency_score: f64,
    /// Score from linked active work items (goal alignment).
    pub work_score: f64,
}

/// Inputs for scoring a single task. Keeps the scoring function pure and testable
/// without requiring a live database connection.
pub struct TaskScoreInput {
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// Whether the task is flagged as urgent.
    pub urgent: bool,
    /// How many downstream tasks are waiting on this task.
    pub blocking_count: i64,
    /// Priorities of active work items linked to this task.
    /// Each entry is a work item priority (1-5 scale, lower = higher priority).
    pub work_priorities: Vec<i32>,
}

/// Configurable weights for each scoring component.
pub struct ScoreWeights {
    /// Multiplier per day of age. Default: 1.0
    pub age_weight: f64,
    /// Flat bonus for urgent tasks. Default: 10.0
    pub urgent_bonus: f64,
    /// Multiplier per downstream blocking task. Default: 1.5
    pub blocking_weight: f64,
    /// Multiplier for work priority contribution. Default: 1.0
    pub work_weight: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            age_weight: 1.0,
            urgent_bonus: 10.0,
            blocking_weight: 1.5,
            work_weight: 1.0,
        }
    }
}

/// Compute the composite score for a task.
///
/// This is a pure function with no side effects or database access.
/// All inputs are pre-fetched and passed in.
pub fn compute_score(
    input: &TaskScoreInput,
    now: DateTime<Utc>,
    weights: &ScoreWeights,
) -> TaskScore {
    // Age score: days since created_at (fractional)
    let age_days = (now - input.created_at).num_seconds() as f64 / 86400.0;
    let age_score = age_days.max(0.0) * weights.age_weight;

    // Urgent score: flat bonus
    let urgent_score = if input.urgent {
        weights.urgent_bonus
    } else {
        0.0
    };

    // Dependency score: bonus for blocking other tasks (critical path)
    let dependency_score = input.blocking_count as f64 * weights.blocking_weight;

    // Work score: sum of (6 - priority) for each active linked work item
    let work_score: f64 = input
        .work_priorities
        .iter()
        .map(|&p| (6 - p).max(0) as f64 * weights.work_weight)
        .sum();

    let total = age_score + urgent_score + dependency_score + work_score;

    TaskScore {
        total,
        age_score,
        urgent_score,
        dependency_score,
        work_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn default_weights() -> ScoreWeights {
        ScoreWeights::default()
    }

    #[test]
    fn test_fresh_non_urgent_task_has_low_score() {
        let now = Utc::now();
        let input = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let score = compute_score(&input, now, &default_weights());
        assert!(score.total < 0.01, "Fresh task should have near-zero score");
        assert!(score.age_score < 0.01);
        assert_eq!(score.urgent_score, 0.0);
        assert_eq!(score.dependency_score, 0.0);
        assert_eq!(score.work_score, 0.0);
    }

    #[test]
    fn test_urgent_task_gets_bonus() {
        let now = Utc::now();
        let input = TaskScoreInput {
            created_at: now,
            urgent: true,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let score = compute_score(&input, now, &default_weights());
        assert_eq!(score.urgent_score, 10.0);
        assert!(score.total >= 10.0);
    }

    #[test]
    fn test_age_accumulates() {
        let now = Utc::now();
        let input = TaskScoreInput {
            created_at: now - Duration::days(5),
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let score = compute_score(&input, now, &default_weights());
        // 5 days * 1.0 weight = 5.0
        assert!((score.age_score - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_stale_task_outscores_fresh_task() {
        let now = Utc::now();
        let fresh = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let stale = TaskScoreInput {
            created_at: now - Duration::days(10),
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let fresh_score = compute_score(&fresh, now, &default_weights());
        let stale_score = compute_score(&stale, now, &default_weights());
        assert!(stale_score.total > fresh_score.total);
    }

    #[test]
    fn test_blocking_task_gets_bonus() {
        let now = Utc::now();
        let isolated = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let blocking = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 3,
            work_priorities: vec![],
        };
        let iso_score = compute_score(&isolated, now, &default_weights());
        let blk_score = compute_score(&blocking, now, &default_weights());
        assert!(blk_score.total > iso_score.total);
        assert!((blk_score.dependency_score - 4.5).abs() < 0.01); // 3 * 1.5
    }

    #[test]
    fn test_work_alignment_scoring() {
        let now = Utc::now();
        let unlinked = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let linked = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![1], // priority 1 → (6-1)*1.0 = 5.0
        };
        let unlinked_score = compute_score(&unlinked, now, &default_weights());
        let linked_score = compute_score(&linked, now, &default_weights());
        assert!(linked_score.total > unlinked_score.total);
        assert!((linked_score.work_score - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_multiple_work_items_stack() {
        let now = Utc::now();
        let input = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![1, 3], // (6-1) + (6-3) = 5+3 = 8
        };
        let score = compute_score(&input, now, &default_weights());
        assert!((score.work_score - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_age_eventually_outweighs_urgent() {
        let now = Utc::now();
        let urgent_fresh = TaskScoreInput {
            created_at: now,
            urgent: true,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let old_normal = TaskScoreInput {
            created_at: now - Duration::days(15),
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        let urgent_score = compute_score(&urgent_fresh, now, &default_weights());
        let old_score = compute_score(&old_normal, now, &default_weights());
        // 15 days age > 10.0 urgent bonus
        assert!(
            old_score.total > urgent_score.total,
            "15-day-old task ({}) should outscore fresh urgent task ({})",
            old_score.total,
            urgent_score.total
        );
    }

    #[test]
    fn test_composite_ordering() {
        let now = Utc::now();
        // High-priority fresh: urgent, linked to priority-1 work
        let high_fresh = TaskScoreInput {
            created_at: now,
            urgent: true,
            blocking_count: 0,
            work_priorities: vec![1],
        };
        // Medium stale: not urgent, but 10 days old
        let medium_stale = TaskScoreInput {
            created_at: now - Duration::days(10),
            urgent: false,
            blocking_count: 0,
            work_priorities: vec![],
        };
        // Low critical path: not urgent, fresh, but blocks 5 tasks
        let low_critical = TaskScoreInput {
            created_at: now,
            urgent: false,
            blocking_count: 5,
            work_priorities: vec![],
        };

        let s1 = compute_score(&high_fresh, now, &default_weights());
        let s2 = compute_score(&medium_stale, now, &default_weights());
        let s3 = compute_score(&low_critical, now, &default_weights());

        // high_fresh: 0 + 10.0 + 0 + 5.0 = 15.0
        // medium_stale: 10.0 + 0 + 0 + 0 = 10.0
        // low_critical: 0 + 0 + 7.5 + 0 = 7.5
        assert!(
            s1.total > s2.total,
            "high fresh ({}) > medium stale ({})",
            s1.total,
            s2.total
        );
        assert!(
            s2.total > s3.total,
            "medium stale ({}) > low critical ({})",
            s2.total,
            s3.total
        );
    }
}
