import Foundation
import SwiftUI

/// Request body for creating a task.
struct CreateTaskRequest: Encodable, Sendable {
    let title: String
    let kind: String?
    let urgent: Bool?
    let context: [String: AnyCodable]?
    let workId: UUID?
}

/// Request body for adding a task comment.
struct CreateTaskCommentRequest: Encodable, Sendable {
    let content: String
}

/// Task dependencies response.
struct TaskDependencies: Codable, Sendable {
    let dependsOn: [DgTask]?
    let dependedOnBy: [DgTask]?
}

/// Service for managing tasks (list, get, create, comments, updates, dependencies).
@Observable
@MainActor
final class TasksService {
    private let apiClient: APIClient

    var tasks: [DgTask] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all tasks for a project.
    func fetchTasks(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [DgTask] = try await apiClient.get(Endpoints.tasks(projectId))
            tasks = result
        } catch {
            self.error = error.localizedDescription
            print("[TasksService] fetchTasks failed: \(error)")
        }
        isLoading = false
    }

    /// Create a new task.
    func createTask(projectId: UUID, request: CreateTaskRequest) async -> DgTask? {
        do {
            let result: DgTask = try await apiClient.post(Endpoints.tasks(projectId), body: request)
            tasks.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            print("[TasksService] createTask failed: \(error)")
            return nil
        }
    }

    /// Fetch updates for a task.
    func getTaskUpdates(projectId: UUID, taskId: UUID) async -> [TaskUpdate] {
        do {
            return try await apiClient.get(Endpoints.taskUpdates(projectId, taskId: taskId))
        } catch {
            self.error = error.localizedDescription
            print("[TasksService] getTaskUpdates failed: \(error)")
            return []
        }
    }

    /// Fetch comments for a task.
    func getTaskComments(projectId: UUID, taskId: UUID) async -> [TaskComment] {
        do {
            return try await apiClient.get(Endpoints.taskComments(projectId, taskId: taskId))
        } catch {
            self.error = error.localizedDescription
            print("[TasksService] getTaskComments failed: \(error)")
            return []
        }
    }

    /// Add a comment to a task.
    func addTaskComment(projectId: UUID, taskId: UUID, content: String) async -> TaskComment? {
        do {
            let request = CreateTaskCommentRequest(content: content)
            let result: TaskComment = try await apiClient.post(
                Endpoints.taskComments(projectId, taskId: taskId),
                body: request
            )
            return result
        } catch {
            self.error = error.localizedDescription
            print("[TasksService] addTaskComment failed: \(error)")
            return nil
        }
    }

    /// Fetch dependencies for a task.
    func getTaskDependencies(projectId: UUID, taskId: UUID) async -> TaskDependencies? {
        do {
            return try await apiClient.get(Endpoints.taskDependencies(projectId, taskId: taskId))
        } catch {
            self.error = error.localizedDescription
            print("[TasksService] getTaskDependencies failed: \(error)")
            return nil
        }
    }
}
