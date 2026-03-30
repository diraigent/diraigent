import SwiftUI

/// Main tab bar navigation for the app.
/// 5 tabs: Dashboard, Work, Chat, Agents, More
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
            .tabItem { Label("Dashboard", systemImage: "chart.bar") }

            NavigationStack {
                WorkListView()
            }
            .tabItem { Label("Work", systemImage: "hammer") }

            NavigationStack {
                ChatView()
            }
            .tabItem { Label("Chat", systemImage: "bubble.left.and.bubble.right") }

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
