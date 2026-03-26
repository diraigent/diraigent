import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct IntegrationID: Hashable {
    let id: UUID
}

/// List of integrations with kind/provider badges and enabled indicator.
struct IntegrationListView: View {
    @Environment(AppState.self) private var appState
    @State private var showingCreateSheet = false

    private var service: IntegrationsService { appState.integrationsService }

    var body: some View {
        Group {
            if service.isLoading && service.integrations.isEmpty {
                ProgressView("Loading integrations...")
            } else if let error = service.error, service.integrations.isEmpty {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") {
                        Task { await loadIntegrations() }
                    }
                }
            } else if service.integrations.isEmpty {
                ContentUnavailableView(
                    "No Integrations",
                    systemImage: "puzzlepiece.extension",
                    description: Text("No integrations configured yet.")
                )
            } else {
                List(service.integrations) { integration in
                    NavigationLink(value: IntegrationID(id: integration.id)) {
                        IntegrationRowView(integration: integration)
                    }
                    .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                        Button(role: .destructive) {
                            Task { await deleteIntegration(integration) }
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                }
                .refreshable {
                    await loadIntegrations()
                }
            }
        }
        .navigationTitle("Integrations")
        .navigationDestination(for: IntegrationID.self) { integrationId in
            if let integration = service.integrations.first(where: { $0.id == integrationId.id }) {
                IntegrationDetailView(integration: integration)
            }
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showingCreateSheet = true
                } label: {
                    Image(systemName: "plus")
                }
            }
        }
        .sheet(isPresented: $showingCreateSheet) {
            CreateIntegrationView()
                .environment(appState)
        }
        .task {
            await loadIntegrations()
        }
    }

    private func loadIntegrations() async {
        guard let projectId = appState.selectedProjectId else { return }
        await service.fetchIntegrations(projectId: projectId)
    }

    private func deleteIntegration(_ integration: Integration) async {
        _ = await service.deleteIntegration(integrationId: integration.id)
    }
}

/// Row for a single integration.
struct IntegrationRowView: View {
    let integration: Integration

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            // Enabled indicator
            Image(systemName: integration.enabled ? "checkmark.circle.fill" : "xmark.circle.fill")
                .foregroundStyle(integration.enabled ? .green : .red)
                .font(.title3)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(integration.name)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(1)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let kind = integration.kind {
                        IntegrationKindBadge(kind: kind)
                    }
                    if let provider = integration.provider {
                        Text(provider)
                            .font(DiraigentTheme.captionFont)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Spacer()
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}

/// Colored badge for integration kind.
struct IntegrationKindBadge: View {
    let kind: String

    private var color: Color {
        switch kind.lowercased() {
        case "ci": .orange
        case "monitoring": .blue
        case "logging": .purple
        case "vcs": .green
        case "chat": .teal
        case "custom": .secondary
        default: .secondary
        }
    }

    var body: some View {
        Text(kind.lowercased())
            .font(.caption2.weight(.semibold))
            .textCase(.uppercase)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}
