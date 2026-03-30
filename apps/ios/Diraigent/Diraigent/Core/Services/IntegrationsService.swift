import Foundation
import SwiftUI

/// Request body for creating an integration.
struct CreateIntegrationRequest: Encodable, Sendable {
    let name: String
    let kind: String
    let provider: String
    let baseUrl: String?
    let authType: String?
}

/// Request body for toggling an integration.
struct UpdateIntegrationRequest: Encodable, Sendable {
    let enabled: Bool
}

/// Service for managing external integrations.
@Observable
@MainActor
final class IntegrationsService {
    private let apiClient: APIClient

    var integrations: [Integration] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all integrations for a project.
    func fetchIntegrations(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [Integration] = try await apiClient.get(Endpoints.integrations(projectId))
            integrations = result
        } catch {
            self.error = error.localizedDescription
            print("[IntegrationsService] fetchIntegrations failed: \(error)")
        }
        isLoading = false
    }

    /// Create a new integration.
    func createIntegration(projectId: UUID, request: CreateIntegrationRequest) async -> Integration? {
        do {
            let result: Integration = try await apiClient.post(Endpoints.integrations(projectId), body: request)
            integrations.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            print("[IntegrationsService] createIntegration failed: \(error)")
            return nil
        }
    }

    /// Delete an integration.
    func deleteIntegration(integrationId: UUID) async -> Bool {
        do {
            try await apiClient.delete(Endpoints.integration(integrationId))
            integrations.removeAll { $0.id == integrationId }
            return true
        } catch {
            self.error = error.localizedDescription
            print("[IntegrationsService] deleteIntegration failed: \(error)")
            return false
        }
    }

    /// Toggle an integration's enabled status.
    func toggleIntegration(integrationId: UUID, enabled: Bool) async -> Bool {
        do {
            let body = UpdateIntegrationRequest(enabled: enabled)
            let updated: Integration = try await apiClient.put(Endpoints.integration(integrationId), body: body)
            if let index = integrations.firstIndex(where: { $0.id == integrationId }) {
                integrations[index] = updated
            }
            return true
        } catch {
            self.error = error.localizedDescription
            print("[IntegrationsService] toggleIntegration failed: \(error)")
            return false
        }
    }
}
