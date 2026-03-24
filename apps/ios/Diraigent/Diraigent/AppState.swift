import Foundation
import SwiftUI

/// Central app state — owns API client and service references.
@Observable
@MainActor
final class AppState {
    let apiClient: APIClient
    var selectedProjectId: UUID?

    init() {
        let config = AppConfig.current
        self.apiClient = APIClient(baseURL: config.apiBaseURL)
    }
}
