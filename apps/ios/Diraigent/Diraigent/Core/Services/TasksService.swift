import Foundation

/// Request body for creating a task.
struct CreateTaskRequest: Encodable, Sendable {
    let title: String
    let kind: String?
    let priority: Int?
    let urgent: Bool?
    let context: CreateTaskContext?
    let playbookId: UUID?
}

/// Context payload for task creation.
struct CreateTaskContext: Encodable, Sendable {
    let spec: String?
    let files: [String]?
    let testCmd: String?
    let acceptanceCriteria: [String]?
}

/// Request body for updating a task.
struct UpdateTaskRequest: Encodable, Sendable {
    let title: String?
    let priority: Int?
    let urgent: Bool?
}

/// Request body for transitioning a task.
struct TransitionTaskRequest: Encodable, Sendable {
    let state: String
}

/// Request body for creating a comment.
struct CreateCommentRequest: Encodable, Sendable {
    let content: String
}

/// Dependency info for a single direction.
struct TaskDependencyInfo: Codable, Identifiable, Sendable {
    let taskId: UUID
    let dependsOn: UUID
    let title: String
    let state: String

    var id: UUID { taskId }
}

/// Full dependency graph for a task.
struct TaskDependencies: Codable, Sendable {
    let dependsOn: [TaskDependencyInfo]
    let blocks: [TaskDependencyInfo]

    init(dependsOn: [TaskDependencyInfo] = [], blocks: [TaskDependencyInfo] = []) {
        self.dependsOn = dependsOn
        self.blocks = blocks
    }
}

/// Service for task CRUD, transitions, comments, updates, and dependencies.
@Observable
@MainActor
final class TasksService {
    var tasks: [DgTask] = []
    var isLoading = false
    var error: String?

    private let apiClient: APIClient

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    // MARK: - Tasks CRUD

    func fetchTasks(projectId: UUID, state: String? = nil) async {
        isLoading = true
        error = nil
        do {
            var query: [String: String] = ["limit": "200"]
            if let state, state != "all" {
                query["state"] = state
            }
            let result: [DgTask] = try await apiClient.get(
                Endpoints.tasks(projectId), query: query
            )
            tasks = result
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    func fetchTask(projectId: UUID, taskId: UUID) async -> DgTask? {
        do {
            return try await apiClient.get(
                Endpoints.task(projectId, taskId: taskId)
            )
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    func createTask(projectId: UUID, request: CreateTaskRequest) async -> DgTask? {
        do {
            let task: DgTask = try await apiClient.post(
                Endpoints.tasks(projectId), body: request
            )
            tasks.insert(task, at: 0)
            return task
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    func updateTask(projectId: UUID, taskId: UUID, update: UpdateTaskRequest) async -> DgTask? {
        do {
            let task: DgTask = try await apiClient.put(
                Endpoints.task(projectId, taskId: taskId), body: update
            )
            if let idx = tasks.firstIndex(where: { $0.id == taskId }) {
                tasks[idx] = task
            }
            return task
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    func transitionTask(projectId: UUID, taskId: UUID, state: String) async -> DgTask? {
        do {
            let task: DgTask = try await apiClient.post(
                Endpoints.transitionTask(projectId, taskId: taskId),
                body: TransitionTaskRequest(state: state)
            )
            if let idx = tasks.firstIndex(where: { $0.id == taskId }) {
                tasks[idx] = task
            }
            return task
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    // MARK: - Updates & Comments

    func fetchUpdates(projectId: UUID, taskId: UUID) async -> [TaskUpdate] {
        do {
            return try await apiClient.get(
                Endpoints.taskUpdates(projectId, taskId: taskId)
            )
        } catch {
            self.error = error.localizedDescription
            return []
        }
    }

    func fetchComments(projectId: UUID, taskId: UUID) async -> [TaskComment] {
        do {
            return try await apiClient.get(
                Endpoints.taskComments(projectId, taskId: taskId)
            )
        } catch {
            self.error = error.localizedDescription
            return []
        }
    }

    func createComment(projectId: UUID, taskId: UUID, body: String) async -> TaskComment? {
        do {
            return try await apiClient.post(
                Endpoints.taskComments(projectId, taskId: taskId),
                body: CreateCommentRequest(content: body)
            )
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    // MARK: - Dependencies & Subtasks

    func fetchDependencies(projectId: UUID, taskId: UUID) async -> TaskDependencies {
        do {
            return try await apiClient.get(
                Endpoints.taskDependencies(projectId, taskId: taskId)
            )
        } catch {
            self.error = error.localizedDescription
            return TaskDependencies()
        }
    }

    func fetchSubtasks(projectId: UUID, taskId: UUID) async -> [DgTask] {
        do {
            return try await apiClient.get(
                Endpoints.taskSubtasks(projectId, taskId: taskId)
            )
        } catch {
            self.error = error.localizedDescription
            return []
        }
    }
}
