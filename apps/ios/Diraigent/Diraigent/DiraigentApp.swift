import SwiftUI

/// Main app entry point.
@main
struct DiraigentApp: App {
    @State private var appState = AppState(apiClient: DemoUITestFixture.client())

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(appState)
                .onChange(of: appState.authService.isAuthenticated) { wasAuthenticated, isAuthenticated in
                    // Drop cached project content when switching accounts.
                    if wasAuthenticated && !isAuthenticated {
                        appState = AppState(apiClient: DemoUITestFixture.client())
                    }
                }
        }
    }
}
