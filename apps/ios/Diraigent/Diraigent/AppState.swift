import Foundation
import SwiftUI

/// Central app state — owns API client, auth service, and shared references.
@Observable
@MainActor
final class AppState {
    let apiClient: APIClient
    let authService: AuthService
    let tasksService: TasksService
    var selectedProjectId: UUID?

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

        self.tasksService = TasksService(apiClient: apiClient)

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
}
