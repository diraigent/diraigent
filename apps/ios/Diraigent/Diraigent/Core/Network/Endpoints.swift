import Foundation

/// API path constants for the diraigent API.
public enum Endpoints {

    // MARK: - Projects

    static let projects = "/projects"
    static func project(_ id: UUID) -> String { "/projects/\(id)" }
    static func projectMetrics(_ id: UUID) -> String { "/projects/\(id)/metrics" }

    // MARK: - Tasks

    static func tasks(_ projectId: UUID) -> String { "/projects/\(projectId)/tasks" }
    static func task(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/tasks/\(taskId)" }
    static func claimTask(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/tasks/\(taskId)/claim" }
    static func transitionTask(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/tasks/\(taskId)/transition" }
    static func taskUpdates(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/tasks/\(taskId)/updates" }
    static func taskComments(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/tasks/\(taskId)/comments" }
    static func taskDependencies(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/tasks/\(taskId)/dependencies" }

    // MARK: - Agents

    static let agents = "/agents"
    static func agent(_ id: UUID) -> String { "/agents/\(id)" }
    static func agentTasks(_ id: UUID) -> String { "/agents/\(id)/tasks" }

    // MARK: - Decisions

    static func decisions(_ projectId: UUID) -> String { "/projects/\(projectId)/decisions" }
    static func decision(_ projectId: UUID, decisionId: UUID) -> String { "/projects/\(projectId)/decisions/\(decisionId)" }

    // MARK: - Observations

    static func observations(_ projectId: UUID) -> String { "/projects/\(projectId)/observations" }
    static func observation(_ projectId: UUID, observationId: UUID) -> String { "/projects/\(projectId)/observations/\(observationId)" }
    static func dismissObservation(_ projectId: UUID, observationId: UUID) -> String { "/projects/\(projectId)/observations/\(observationId)/dismiss" }
    static func promoteObservation(_ projectId: UUID, observationId: UUID) -> String { "/projects/\(projectId)/observations/\(observationId)/promote" }

    // MARK: - Knowledge

    static func knowledge(_ projectId: UUID) -> String { "/projects/\(projectId)/knowledge" }

    // MARK: - Work

    static func work(_ projectId: UUID) -> String { "/projects/\(projectId)/work" }
    static func workItem(_ projectId: UUID, workId: UUID) -> String { "/projects/\(projectId)/work/\(workId)" }
    static func workTasks(_ projectId: UUID, workId: UUID) -> String { "/projects/\(projectId)/work/\(workId)/tasks" }
    static func workProgress(_ projectId: UUID, workId: UUID) -> String { "/projects/\(projectId)/work/\(workId)/progress" }

    // MARK: - Git

    static func gitBranches(_ projectId: UUID) -> String { "/projects/\(projectId)/git/branches" }
    static func gitTaskStatus(_ projectId: UUID, taskId: UUID) -> String { "/projects/\(projectId)/git/task-status/\(taskId)" }

    // MARK: - Search

    static func search(_ projectId: UUID) -> String { "/projects/\(projectId)/search" }

    // MARK: - Chat

    static func chat(_ projectId: UUID) -> String { "/projects/\(projectId)/chat" }

    // MARK: - Dashboard

    static let dashboard = "/dashboard"

    // MARK: - Events

    static func events(_ projectId: UUID) -> String { "/projects/\(projectId)/events" }

    // MARK: - Reports

    static func reports(_ projectId: UUID) -> String { "/projects/\(projectId)/reports" }

    // MARK: - Audit

    static func audit(_ projectId: UUID) -> String { "/projects/\(projectId)/audit" }
}
