import Foundation

/// Summary of tasks by state.
struct TaskSummary: Codable, Sendable {
    let total: Int?
    let done: Int?
    let cancelled: Int?
    let inProgress: Int?
    let ready: Int?
    let backlog: Int?
    let humanReview: Int?
}

/// Summary of token usage and cost.
struct CostSummary: Codable, Sendable {
    let totalInputTokens: Int?
    let totalOutputTokens: Int?
    let totalCostUsd: Double?
}

/// Per-agent metrics.
struct AgentMetricEntry: Codable, Sendable {
    let agentId: UUID?
    let agentName: String?
    let tasksCompleted: Int?
    let tasksInProgress: Int?
    let avgCompletionHours: Double?
}

/// Tasks created per day.
struct DayCount: Codable, Sendable {
    let day: String
    let count: Int
}

/// Average time in each state.
struct StateAvg: Codable, Sendable {
    let state: String
    let avgHours: Double?
}

/// Playbook completion metrics.
struct PlaybookMetricEntry: Codable, Sendable {
    let playbookId: UUID?
    let playbookTitle: String?
    let totalTasks: Int?
    let completedTasks: Int?
    let completionRate: Double?
}

/// Per-task cost row.
struct TaskCostRow: Codable, Sendable {
    let taskId: UUID?
    let title: String?
    let costUsd: Double?
    let inputTokens: Int?
    let outputTokens: Int?
}

/// Full project metrics response.
struct ProjectMetrics: Codable, Sendable {
    let projectId: UUID?
    let rangeDays: Int?
    let taskSummary: TaskSummary?
    let tasksPerDay: [DayCount]?
    let avgTimeInStateHours: [StateAvg]?
    let agentBreakdown: [AgentMetricEntry]?
    let playbookCompletion: [PlaybookMetricEntry]?
    let costSummary: CostSummary?
    let taskCosts: [TaskCostRow]?
}
