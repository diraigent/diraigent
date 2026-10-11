import Foundation
import Observation

/// Fail closed until both account and selected-project permissions are known.
@Observable
@MainActor
final class AccountAccessService {
    private let apiClient: APIClient
    private var revision = 0
    private(set) var canWrite = false
    private(set) var isLoading = true
    private(set) var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    func reset() {
        revision += 1
        canWrite = false
        isLoading = true
        error = nil
    }

    func refresh(projectId: UUID?) async {
        reset()
        let current = revision
        do {
            let account: Access = try await apiClient.get("/account")
            var writable = account.readOnly == false
            if writable, let projectId {
                let project: Access = try await apiClient.get("/\(projectId)/people/me")
                writable = project.readOnly == false
            } else if projectId == nil {
                writable = false
            }
            guard current == revision, !Task.isCancelled else { return }
            canWrite = writable
            isLoading = false
        } catch {
            guard current == revision, !Task.isCancelled else { return }
            self.error = "Could not verify access. You can browse while editing remains disabled."
            isLoading = false
        }
    }

    private struct Access: Decodable, Sendable {
        let readOnly: Bool?
    }
}
