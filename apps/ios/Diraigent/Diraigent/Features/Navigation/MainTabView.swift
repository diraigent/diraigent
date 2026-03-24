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
                TaskListView()
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
                    WorkListView()
                } label: {
                    Label("Work", systemImage: "hammer")
                }

                NavigationLink {
                    EventListView()
                } label: {
                    Label("Events", systemImage: "bell")
                }

                NavigationLink {
                    AuditListView()
                } label: {
                    Label("Audit Log", systemImage: "clock.arrow.circlepath")
                }

                NavigationLink {
                    ReportListView()
                } label: {
                    Label("Reports", systemImage: "doc.text.magnifyingglass")
                }

                NavigationLink {
                    GitView()
                } label: {
                    Label("Git", systemImage: "arrow.triangle.branch")
                }

                NavigationLink {
                    SearchView()
                } label: {
                    Label("Search", systemImage: "magnifyingglass")
                }
            }

            Section("App") {
                NavigationLink {
                    SettingsView()
                } label: {
                    Label("Settings", systemImage: "gearshape")
                }
            }
        }
        .navigationTitle("More")
        .navigationDestination(for: WorkID.self) { workId in
            if let work = appState.workService.workItems.first(where: { $0.id == workId.id }) {
                WorkDetailView(work: work)
            }
        }
        .navigationDestination(for: DecisionID.self) { decisionId in
            if let decision = appState.decisionsService.decisions.first(where: { $0.id == decisionId.id }) {
                DecisionDetailView(decision: decision)
            }
        }
        .navigationDestination(for: ObservationID.self) { obsId in
            if let obs = appState.observationsService.observations.first(where: { $0.id == obsId.id }) {
                ObservationDetailView(observation: obs)
            }
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
