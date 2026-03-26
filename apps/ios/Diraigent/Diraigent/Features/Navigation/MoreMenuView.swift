import SwiftUI

/// "More" tab — categorized list of sub-features matching the TUI 16-view layout.
///
/// Categories:
/// - Core: Decisions, Observations
/// - Tools: Git, Source Browser, Search, Chat, Logs
/// - Reference: Reports
/// - System: Audit, Events, Integrations, Webhooks
/// - Settings: Project Settings
struct MoreMenuView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        List {
            Section("Core") {
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
            }

            Section("Tools") {
                NavigationLink {
                    GitView()
                } label: {
                    Label("Git", systemImage: "arrow.triangle.branch")
                }

                NavigationLink {
                    SourceBrowserView()
                } label: {
                    Label("Source Browser", systemImage: "folder")
                }

                NavigationLink {
                    SearchView()
                } label: {
                    Label("Search", systemImage: "magnifyingglass")
                }

                NavigationLink {
                    ChatView()
                } label: {
                    Label("Chat", systemImage: "bubble.left.and.bubble.right")
                }

                NavigationLink {
                    LogsView()
                } label: {
                    Label("Logs", systemImage: "terminal")
                }
            }

            Section("Reference") {
                NavigationLink {
                    ReportListView()
                } label: {
                    Label("Reports", systemImage: "doc.text")
                }
            }

            Section("System") {
                NavigationLink {
                    AuditListView()
                } label: {
                    Label("Audit", systemImage: "shield")
                }

                NavigationLink {
                    EventListView()
                } label: {
                    Label("Events", systemImage: "bell")
                }

                NavigationLink {
                    IntegrationListView()
                } label: {
                    Label("Integrations", systemImage: "puzzlepiece")
                }

                NavigationLink {
                    WebhookListView()
                } label: {
                    Label("Webhooks", systemImage: "antenna.radiowaves.left.and.right")
                }
            }

            Section("Settings") {
                NavigationLink {
                    SettingsView()
                } label: {
                    Label("Project Settings", systemImage: "gear")
                }
            }
        }
        .navigationTitle("More")
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
