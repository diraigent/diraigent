import Foundation
import SwiftUI

/// Request body for creating a work item.
struct CreateWorkRequest: Encodable, Sendable {
    let title: String
    let workType: String?
    let status: String?
    let description: String?
}

/// Request body for updating a work item.
struct UpdateWorkRequest: Encodable, Sendable {
    let title: String?
    let workType: String?
    let status: String?
    let description: String?
}

/// Service for managing work items (epics, features, milestones, sprints, initiatives).
@Observable
@MainActor
final class WorkService {
    private let apiClient: APIClient

    var workItems: [Work] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all work items for a project.
    func fetchWork(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [Work] = try await apiClient.get(Endpoints.work(projectId))
            workItems = result
        } catch {
            self.error = error.localizedDescription
            print("[WorkService] fetchWork failed: \(error)")
        }
        isLoading = false
    }

    /// Create a new work item.
    func createWork(projectId: UUID, request: CreateWorkRequest) async -> Work? {
        do {
            let result: Work = try await apiClient.post(Endpoints.work(projectId), body: request)
            workItems.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    /// Fetch tasks linked to a work item.
    func fetchWorkTasks(projectId: UUID, workId: UUID) async -> [DgTask] {
        do {
            return try await apiClient.get(Endpoints.workTasks(projectId, workId: workId))
        } catch {
            self.error = error.localizedDescription
            return []
        }
    }

    /// Fetch progress for a work item.
    func fetchWorkProgress(projectId: UUID, workId: UUID) async -> WorkProgress? {
        do {
            return try await apiClient.get(Endpoints.workProgress(projectId, workId: workId))
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    /// Update an existing work item.
    func updateWork(projectId: UUID, workId: UUID, update: UpdateWorkRequest) async -> Work? {
        do {
            let result: Work = try await apiClient.put(
                Endpoints.workItem(projectId, workId: workId),
                body: update
            )
            if let idx = workItems.firstIndex(where: { $0.id == workId }) {
                workItems[idx] = result
            }
            return result
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }
}
