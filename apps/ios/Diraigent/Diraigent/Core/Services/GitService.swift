import Foundation
import SwiftUI

/// Service for fetching git status information.
@Observable
@MainActor
final class GitService {
    private let apiClient: APIClient

    var branches: [BranchInfo] = []
    var currentBranch: String = ""
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all branches for a project.
    func fetchBranches(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: BranchListResponse = try await apiClient.get(Endpoints.gitBranches(projectId))
            branches = result.branches
            currentBranch = result.currentBranch
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    /// Fetch task branch status for a specific task.
    func fetchTaskBranchStatus(projectId: UUID, taskId: UUID) async -> GitTaskStatus? {
        do {
            let result: GitTaskStatus = try await apiClient.get(Endpoints.gitTaskStatus(projectId, taskId: taskId))
            return result
        } catch {
            return nil
        }
    }
}
