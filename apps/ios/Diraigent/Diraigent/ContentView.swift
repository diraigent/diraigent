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
                        if !DemoUITestFixture.isEnabled { await appState.authService.restoreSession() }
                        hasAttemptedRestore = true
                    }
            } else if appState.authService.isAuthenticated || DemoUITestFixture.isEnabled {
                MainTabView()
                    .task(id: appState.selectedProjectId) {
                        await appState.accountAccess.refresh(projectId: appState.selectedProjectId)
                        await appState.projectService.fetchProjects()
                        // Auto-select first project if none persisted
                        if !appState.projectService.projects.contains(where: { $0.id == appState.selectedProjectId }),
                           let first = appState.projectService.projects.first {
                            appState.selectProject(first.id)
                        }
                    }
            } else {
                LoginView()
                    .onAppear { appState.accountAccess.reset() }
            }
        }
    }
}

#Preview("Authenticated") {
    ContentView()
        .environment(AppState())
}
