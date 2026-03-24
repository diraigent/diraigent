import SwiftUI

/// Root view — routes between login and main app based on auth state.
struct ContentView: View {
    @Environment(AppState.self) private var appState
    @State private var hasAttemptedRestore = false

    var body: some View {
        Group {
            if !hasAttemptedRestore {
                // Show a brief loading state while checking for existing session
                ProgressView("Loading\u{2026}")
                    .task {
                        await appState.authService.restoreSession()
                        hasAttemptedRestore = true
                    }
            } else if appState.authService.isAuthenticated {
                MainTabView()
                    .task {
                        await appState.projectService.fetchProjects()
                        // Auto-select first project if none persisted
                        if appState.selectedProjectId == nil,
                           let first = appState.projectService.projects.first {
                            appState.selectProject(first.id)
                        }
                    }
            } else {
                LoginView()
            }
        }
    }
}

#Preview("Authenticated") {
    ContentView()
        .environment(AppState())
}
