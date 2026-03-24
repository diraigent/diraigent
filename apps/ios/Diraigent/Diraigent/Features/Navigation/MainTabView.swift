import SwiftUI

/// Main tab bar navigation for the app.
struct MainTabView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        TabView {
            NavigationStack {
                DashboardView()
                    .toolbar {
                        ToolbarItem(placement: .topBarLeading) {
                            ProjectSelectorButton()
                        }
                    }
            }
            .tabItem { Label("Dashboard", systemImage: "house.fill") }

            NavigationStack {
                PlaceholderView(title: "Tasks", icon: "checklist")
                    .toolbar {
                        ToolbarItem(placement: .topBarLeading) {
                            ProjectSelectorButton()
                        }
                    }
            }
            .tabItem { Label("Tasks", systemImage: "checklist") }

            NavigationStack {
                AgentListView()
            }
            .tabItem { Label("Agents", systemImage: "cpu") }

            NavigationStack {
                MoreMenuView()
            }
            .tabItem { Label("More", systemImage: "ellipsis.circle") }
        }
    }
}

/// "More" tab — list of sub-features.
struct MoreMenuView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        List {
            Section("Project") {
                NavigationLink {
                    DecisionListView()
                } label: {
                    Label("Decisions", systemImage: "scale.3d")
                }

                NavigationLink {
                    ObservationListView()
                } label: {
                    Label("Observations", systemImage: "eye")
                }

                NavigationLink {
                    PlaceholderView(title: "Work", icon: "hammer")
                } label: {
                    Label("Work", systemImage: "hammer")
                }

                NavigationLink {
                    PlaceholderView(title: "Git", icon: "arrow.triangle.branch")
                } label: {
                    Label("Git", systemImage: "arrow.triangle.branch")
                }

                NavigationLink {
                    PlaceholderView(title: "Search", icon: "magnifyingglass")
                } label: {
                    Label("Search", systemImage: "magnifyingglass")
                }
            }

            Section("App") {
                NavigationLink {
                    SettingsPlaceholderView()
                } label: {
                    Label("Settings", systemImage: "gearshape")
                }
            }
        }
        .navigationTitle("More")
    }
}

/// Settings placeholder with sign-out button.
struct SettingsPlaceholderView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        List {
            Section {
                Button(role: .destructive) {
                    appState.authService.logout()
                } label: {
                    Label("Sign Out", systemImage: "rectangle.portrait.and.arrow.right")
                }
            }
        }
        .navigationTitle("Settings")
    }
}

/// Reusable placeholder for features not yet built.
struct PlaceholderView: View {
    let title: String
    let icon: String

    var body: some View {
        VStack(spacing: DiraigentTheme.spacingLG) {
            Image(systemName: icon)
                .font(.system(size: 48))
                .foregroundStyle(.tint)
            Text(title)
                .font(DiraigentTheme.headlineFont)
            Text("Coming soon")
                .foregroundStyle(.secondary)
                .font(DiraigentTheme.captionFont)
        }
        .navigationTitle(title)
    }
}
