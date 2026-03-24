import Foundation
import SwiftUI

/// Request body for creating a decision.
struct CreateDecisionRequest: Encodable, Sendable {
    let title: String
    let description: String?
    let rationale: String?
    let alternatives: [DecisionAlternative]?
    let consequences: String?
}

/// Request body for updating a decision.
struct UpdateDecisionRequest: Encodable, Sendable {
    let title: String?
    let status: String?
    let description: String?
    let rationale: String?
    let consequences: String?
}

/// Service for managing decisions.
@Observable
@MainActor
final class DecisionsService {
    private let apiClient: APIClient

    var decisions: [Decision] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all decisions for a project.
    func fetchDecisions(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [Decision] = try await apiClient.get(Endpoints.decisions(projectId))
            decisions = result
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    /// Create a new decision.
    func createDecision(projectId: UUID, request: CreateDecisionRequest) async -> Decision? {
        do {
            let result: Decision = try await apiClient.post(Endpoints.decisions(projectId), body: request)
            decisions.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    /// Update an existing decision.
    func updateDecision(projectId: UUID, decisionId: UUID, update: UpdateDecisionRequest) async -> Decision? {
        do {
            let result: Decision = try await apiClient.put(
                Endpoints.decision(projectId, decisionId: decisionId),
                body: update
            )
            if let idx = decisions.firstIndex(where: { $0.id == decisionId }) {
                decisions[idx] = result
            }
            return result
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }
}
