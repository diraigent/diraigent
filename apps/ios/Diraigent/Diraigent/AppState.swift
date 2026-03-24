import Foundation
import SwiftUI

/// Central app state — owns API client, auth service, and shared references.
@Observable
@MainActor
final class AppState {
    let apiClient: APIClient
    let authService: AuthService
    let projectService: ProjectService
    let dashboardService: DashboardService
    let agentsService: AgentsService
    let decisionsService: DecisionsService
    let observationsService: ObservationsService

    var selectedProjectId: UUID? {
        didSet {
            // Persist selection
            if let id = selectedProjectId {
                UserDefaults.standard.set(id.uuidString, forKey: Self.selectedProjectKey)
            } else {
                UserDefaults.standard.removeObject(forKey: Self.selectedProjectKey)
            }
        }
    }

    private static let selectedProjectKey = "selectedProjectId"

    init() {
        let config = AppConfig.current
        let apiClient = APIClient(baseURL: config.apiBaseURL)
        self.apiClient = apiClient

        self.authService = AuthService(
            config: AuthService.Config(
                issuer: config.authIssuer,
                clientId: config.authClientId,
                redirectURI: config.authRedirectURI
            ),
            apiClient: apiClient
        )

        self.projectService = ProjectService(apiClient: apiClient)
        self.dashboardService = DashboardService(apiClient: apiClient)
        self.agentsService = AgentsService(apiClient: apiClient)
        self.decisionsService = DecisionsService(apiClient: apiClient)
        self.observationsService = ObservationsService(apiClient: apiClient)

        // Restore last selected project
        if let saved = UserDefaults.standard.string(forKey: Self.selectedProjectKey),
           let uuid = UUID(uuidString: saved) {
            self.selectedProjectId = uuid
        }

        // Wire up 401 handler so unauthorized responses trigger logout
        let authService = self.authService
        Task {
            await apiClient.setOnUnauthorized {
                await MainActor.run {
                    authService.logout()
                }
            }
        }
    }

    /// Select a project and persist the choice.
    func selectProject(_ id: UUID) {
        selectedProjectId = id
    }
}
