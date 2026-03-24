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
            } else {
                LoginView()
            }
        }
    }
}

/// Main tab bar navigation — placeholder until feature views are built.
struct MainTabView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        TabView {
            NavigationStack {
                VStack(spacing: 16) {
                    Image(systemName: "cpu")
                        .font(.system(size: 48))
                        .foregroundStyle(.tint)
                    Text("Projects")
                        .font(.title2.bold())
                    Text("Coming soon")
                        .foregroundStyle(.secondary)
                }
                .navigationTitle("Projects")
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Sign Out", systemImage: "rectangle.portrait.and.arrow.right") {
                            appState.authService.logout()
                        }
                    }
                }
            }
            .tabItem { Label("Projects", systemImage: "folder.fill") }

            NavigationStack {
                VStack(spacing: 16) {
                    Image(systemName: "checklist")
                        .font(.system(size: 48))
                        .foregroundStyle(.tint)
                    Text("Tasks")
                        .font(.title2.bold())
                    Text("Coming soon")
                        .foregroundStyle(.secondary)
                }
                .navigationTitle("Tasks")
            }
            .tabItem { Label("Tasks", systemImage: "checklist") }

            NavigationStack {
                VStack(spacing: 16) {
                    Image(systemName: "person.3.fill")
                        .font(.system(size: 48))
                        .foregroundStyle(.tint)
                    Text("Agents")
                        .font(.title2.bold())
                    Text("Coming soon")
                        .foregroundStyle(.secondary)
                }
                .navigationTitle("Agents")
            }
            .tabItem { Label("Agents", systemImage: "person.3.fill") }

            NavigationStack {
                VStack(spacing: 16) {
                    Image(systemName: "gearshape.fill")
                        .font(.system(size: 48))
                        .foregroundStyle(.tint)
                    Text("Settings")
                        .font(.title2.bold())
                    Text("Coming soon")
                        .foregroundStyle(.secondary)
                }
                .navigationTitle("Settings")
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Sign Out", systemImage: "rectangle.portrait.and.arrow.right") {
                            appState.authService.logout()
                        }
                    }
                }
            }
            .tabItem { Label("Settings", systemImage: "gearshape.fill") }
        }
    }
}

#Preview("Authenticated") {
    ContentView()
        .environment(AppState())
}
