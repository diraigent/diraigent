import Foundation
import SwiftUI

/// Request body for creating an observation.
struct CreateObservationRequest: Encodable, Sendable {
    let title: String
    let description: String?
    let kind: String
    let severity: String
}

/// Service for managing observations.
@Observable
@MainActor
final class ObservationsService {
    private let apiClient: APIClient

    var observations: [DgObservation] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all observations for a project.
    func fetchObservations(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [DgObservation] = try await apiClient.get(Endpoints.observations(projectId))
            observations = result
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    /// Create a new observation.
    func createObservation(projectId: UUID, request: CreateObservationRequest) async -> DgObservation? {
        do {
            let result: DgObservation = try await apiClient.post(
                Endpoints.observations(projectId),
                body: request
            )
            observations.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    /// Dismiss an observation.
    func dismissObservation(projectId: UUID, observationId: UUID) async -> Bool {
        do {
            try await apiClient.post(Endpoints.dismissObservation(projectId, observationId: observationId))
            observations.removeAll { $0.id == observationId }
            return true
        } catch {
            self.error = error.localizedDescription
            return false
        }
    }

    /// Promote an observation to a task.
    func promoteObservation(projectId: UUID, observationId: UUID) async -> Bool {
        do {
            try await apiClient.post(Endpoints.promoteObservation(projectId, observationId: observationId))
            // Update the observation status locally
            if let idx = observations.firstIndex(where: { $0.id == observationId }) {
                observations.remove(at: idx)
            }
            return true
        } catch {
            self.error = error.localizedDescription
            return false
        }
    }
}
