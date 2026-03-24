import Foundation

/// API path constants for the diraigent API.
///
/// The API mounts project routes at the root of `/v1`, so project list is
/// `GET /v1/` and project-scoped resources are `GET /v1/{project_id}/...`.
public enum Endpoints {

    // MARK: - Projects

    static let projects = ""
    static func project(_ id: UUID) -> String { "/\(id)" }
    static func projectMetrics(_ id: UUID) -> String { "/\(id)/metrics" }

    // MARK: - Tasks

    static func tasks(_ projectId: UUID) -> String { "/\(projectId)/tasks" }
    static func task(_ projectId: UUID, taskId: UUID) -> String { "/tasks/\(taskId)" }
    static func claimTask(_ projectId: UUID, taskId: UUID) -> String { "/tasks/\(taskId)/claim" }
    static func transitionTask(_ projectId: UUID, taskId: UUID) -> String { "/tasks/\(taskId)/transition" }
    static func taskUpdates(_ projectId: UUID, taskId: UUID) -> String { "/tasks/\(taskId)/updates" }
    static func taskComments(_ projectId: UUID, taskId: UUID) -> String { "/tasks/\(taskId)/comments" }
    static func taskDependencies(_ projectId: UUID, taskId: UUID) -> String { "/tasks/\(taskId)/dependencies" }

    // MARK: - Agents

    static let agents = "/agents"
    static func agent(_ id: UUID) -> String { "/agents/\(id)" }
    static func agentTasks(_ id: UUID) -> String { "/agents/\(id)/tasks" }

    // MARK: - Decisions

    static func decisions(_ projectId: UUID) -> String { "/\(projectId)/decisions" }
    static func decision(_ projectId: UUID, decisionId: UUID) -> String { "/decisions/\(decisionId)" }

    // MARK: - Observations

    static func observations(_ projectId: UUID) -> String { "/\(projectId)/observations" }
    static func observation(_ projectId: UUID, observationId: UUID) -> String { "/observations/\(observationId)" }
    static func dismissObservation(_ projectId: UUID, observationId: UUID) -> String { "/observations/\(observationId)/dismiss" }
    static func promoteObservation(_ projectId: UUID, observationId: UUID) -> String { "/observations/\(observationId)/promote" }

    // MARK: - Knowledge

    static func knowledge(_ projectId: UUID) -> String { "/\(projectId)/knowledge" }

    // MARK: - Work

    static func work(_ projectId: UUID) -> String { "/\(projectId)/work" }
    static func workItem(_ projectId: UUID, workId: UUID) -> String { "/work/\(workId)" }
    static func workTasks(_ projectId: UUID, workId: UUID) -> String { "/work/\(workId)/tasks" }
    static func workProgress(_ projectId: UUID, workId: UUID) -> String { "/work/\(workId)/progress" }

    // MARK: - Git

    static func gitBranches(_ projectId: UUID) -> String { "/\(projectId)/git/branches" }
    static func gitTaskStatus(_ projectId: UUID, taskId: UUID) -> String { "/\(projectId)/git/task-status/\(taskId)" }

    // MARK: - Search

    static func search(_ projectId: UUID) -> String { "/\(projectId)/search" }

    // MARK: - Chat

    static func chat(_ projectId: UUID) -> String { "/\(projectId)/chat" }

    // MARK: - Dashboard

    static let dashboard = "/dashboard/summary"

    // MARK: - Events

    static func events(_ projectId: UUID) -> String { "/\(projectId)/events" }

    // MARK: - Reports

    static func reports(_ projectId: UUID) -> String { "/\(projectId)/reports" }

    // MARK: - Audit

    static func audit(_ projectId: UUID) -> String { "/\(projectId)/audit" }

    // MARK: - Logs

    static let logs = "/logs"
    static let logLabels = "/logs/labels"
}
