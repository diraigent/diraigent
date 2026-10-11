import SwiftUI

/// Main tab bar navigation for the app.
/// 5 tabs: Dashboard, Work, Chat, Agents, More
struct MainTabView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.scenePhase) private var scenePhase
    @State private var selectedTab: Tab = .dashboard
    @State private var previousTab: Tab = .dashboard

    private enum Tab: Hashable {
        case dashboard, work, chat, agents, more
    }

    var body: some View {
        VStack(spacing: 0) {
            if !appState.accountAccess.canWrite { accessBanner }
            TabView(selection: $selectedTab) {
                NavigationStack {
                    DashboardView()
                        .toolbar {
                            ToolbarItem(placement: .topBarLeading) {
                                ProjectSelectorButton()
                            }
                        }
                }
                .tabItem { Label("Dashboard", systemImage: "chart.bar") }
                .tag(Tab.dashboard)

                NavigationStack {
                    WorkListView()
                }
                .tabItem { Label("Work", systemImage: "hammer") }
                .tag(Tab.work)

                if appState.accountAccess.canWrite {
                    NavigationStack {
                        ChatView(onExit: { selectedTab = previousTab })
                    }
                    .tabItem { Label("Chat", systemImage: "bubble.left.and.bubble.right") }
                    .tag(Tab.chat)
                }

                NavigationStack {
                    AgentListView()
                }
                .tabItem { Label("Agents", systemImage: "cpu") }
                .tag(Tab.agents)

                NavigationStack {
                    MoreMenuView()
                }
                .tabItem { Label("More", systemImage: "ellipsis.circle") }
                .tag(Tab.more)
            }
        }
        .onChange(of: appState.accountAccess.canWrite) { _, canWrite in
            if !canWrite && selectedTab == .chat { selectedTab = .dashboard }
        }
        .onChange(of: appState.authService.isAuthenticated) { _, _ in
            appState.accountAccess.reset()
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active {
                Task { await appState.accountAccess.refresh(projectId: appState.selectedProjectId) }
            }
        }
        .onChange(of: selectedTab) { oldTab, _ in
            if oldTab != .chat { previousTab = oldTab }
        }
    }

    private var accessBanner: some View {
        HStack {
            Label(appState.accountAccess.isLoading ? "Checking access…" : "Read-only access", systemImage: "lock")
            Spacer()
            if !appState.accountAccess.isLoading {
                Button("Refresh") {
                    Task { await appState.accountAccess.refresh(projectId: appState.selectedProjectId) }
                }
            }
        }
        .font(.caption)
        .padding(8)
        .background(.regularMaterial)
        .accessibilityIdentifier("readOnlyAccess")
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
