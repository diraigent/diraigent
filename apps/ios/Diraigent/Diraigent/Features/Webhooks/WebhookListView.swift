import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct WebhookID: Hashable {
    let id: UUID
}

/// List of webhooks with enabled indicator, URL, and event count.
struct WebhookListView: View {
    @Environment(AppState.self) private var appState
    @State private var showingCreateSheet = false

    private var service: WebhooksService { appState.webhooksService }

    var body: some View {
        Group {
            if service.isLoading && service.webhooks.isEmpty {
                ProgressView("Loading webhooks...")
            } else if let error = service.error, service.webhooks.isEmpty {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") {
                        Task { await loadWebhooks() }
                    }
                }
            } else if service.webhooks.isEmpty {
                ContentUnavailableView(
                    "No Webhooks",
                    systemImage: "link",
                    description: Text("No webhooks configured yet.")
                )
            } else {
                List(service.webhooks) { webhook in
                    NavigationLink(value: WebhookID(id: webhook.id)) {
                        WebhookRowView(webhook: webhook)
                    }
                    .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                        Button(role: .destructive) {
                            Task { await deleteWebhook(webhook) }
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                }
                .refreshable {
                    await loadWebhooks()
                }
            }
        }
        .navigationTitle("Webhooks")
        .navigationDestination(for: WebhookID.self) { webhookId in
            if let webhook = service.webhooks.first(where: { $0.id == webhookId.id }) {
                WebhookDetailView(webhook: webhook)
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
            CreateWebhookView()
                .environment(appState)
        }
        .task {
            await loadWebhooks()
        }
    }

    private func loadWebhooks() async {
        guard let projectId = appState.selectedProjectId else { return }
        await service.fetchWebhooks(projectId: projectId)
    }

    private func deleteWebhook(_ webhook: Webhook) async {
        _ = await service.deleteWebhook(webhookId: webhook.id)
    }
}

/// Row for a single webhook.
struct WebhookRowView: View {
    let webhook: Webhook

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            // Enabled indicator
            Image(systemName: webhook.enabled ? "checkmark.circle.fill" : "xmark.circle.fill")
                .foregroundStyle(webhook.enabled ? .green : .red)
                .font(.title3)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(webhook.name.isEmpty ? "Unnamed" : webhook.name)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(1)

                Text(webhook.url)
                    .font(DiraigentTheme.captionFont)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    Label("\(webhook.events.count) events", systemImage: "antenna.radiowaves.left.and.right")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}
