import SwiftUI

/// Detail view for a single webhook.
struct WebhookDetailView: View {
    @Environment(AppState.self) private var appState
    let webhook: Webhook

    @State private var isToggling = false
    @State private var isTesting = false
    @State private var testResult: String?

    private var service: WebhooksService { appState.webhooksService }

    var body: some View {
        List {
            // MARK: - Header
            Section {
                HStack(spacing: DiraigentTheme.spacingMD) {
                    Image(systemName: "link")
                        .font(.title2)
                        .foregroundStyle(.tint)

                    VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                        HStack(spacing: DiraigentTheme.spacingSM) {
                            Image(systemName: webhook.enabled ? "checkmark.circle.fill" : "xmark.circle.fill")
                                .foregroundStyle(webhook.enabled ? .green : .red)
                            Text(webhook.enabled ? "Enabled" : "Disabled")
                                .font(DiraigentTheme.bodyFont)
                                .foregroundStyle(webhook.enabled ? .green : .red)
                        }
                    }

                    Spacer()
                }
            }

            // MARK: - Details
            Section("Details") {
                HStack {
                    Text("Name")
                    Spacer()
                    Text(webhook.name.isEmpty ? "Unnamed" : webhook.name)
                        .foregroundStyle(.secondary)
                }

                HStack {
                    Text("URL")
                    Spacer()
                    Text(webhook.url)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                HStack {
                    Text("Secret")
                    Spacer()
                    if let secret = webhook.secret, !secret.isEmpty {
                        Text(String(repeating: "*", count: min(secret.count, 12)))
                            .foregroundStyle(.secondary)
                    } else {
                        Text("None")
                            .foregroundStyle(.secondary)
                    }
                }

                if let createdAt = webhook.createdAt {
                    HStack {
                        Text("Created")
                        Spacer()
                        Text(formatTimestamp(createdAt))
                            .foregroundStyle(.secondary)
                    }
                }

                if let updatedAt = webhook.updatedAt {
                    HStack {
                        Text("Updated")
                        Spacer()
                        Text(formatTimestamp(updatedAt))
                            .foregroundStyle(.secondary)
                    }
                }
            }

            // MARK: - Subscribed Events
            if !webhook.events.isEmpty {
                Section("Subscribed Events") {
                    ForEach(webhook.events, id: \.self) { event in
                        HStack {
                            Image(systemName: "antenna.radiowaves.left.and.right")
                                .foregroundStyle(.tint)
                            Text(event)
                                .font(DiraigentTheme.bodyFont)
                        }
                    }
                }
            }

            // MARK: - Recent Deliveries
            Section("Recent Deliveries") {
                if service.deliveries.isEmpty {
                    Text("No deliveries yet")
                        .foregroundStyle(.secondary)
                        .font(DiraigentTheme.captionFont)
                } else {
                    ForEach(service.deliveries) { delivery in
                        DeliveryRowView(delivery: delivery)
                    }
                }
            }

            // MARK: - Actions
            Section {
                // Toggle enabled
                Button {
                    Task { await toggleEnabled() }
                } label: {
                    HStack {
                        Spacer()
                        if isToggling {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Label(
                                webhook.enabled ? "Disable Webhook" : "Enable Webhook",
                                systemImage: webhook.enabled ? "xmark.circle" : "checkmark.circle"
                            )
                        }
                        Spacer()
                    }
                }
                .foregroundStyle(webhook.enabled ? .red : .green)
                .disabled(isToggling)

                // Test webhook
                Button {
                    Task { await testWebhook() }
                } label: {
                    HStack {
                        Spacer()
                        if isTesting {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Label("Test Webhook", systemImage: "paperplane")
                        }
                        Spacer()
                    }
                }
                .disabled(isTesting)

                if let testResult {
                    Text(testResult)
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(testResult.contains("Success") ? .green : .red)
                }
            }
        }
        .navigationTitle(webhook.name.isEmpty ? "Webhook" : webhook.name)
        .task {
            await service.listDeliveries(webhookId: webhook.id)
        }
    }

    private func toggleEnabled() async {
        isToggling = true
        _ = await service.toggleWebhook(
            webhookId: webhook.id,
            enabled: !webhook.enabled
        )
        // Refresh the list to pick up updated state
        if let projectId = appState.selectedProjectId {
            await service.fetchWebhooks(projectId: projectId)
        }
        isToggling = false
    }

    private func testWebhook() async {
        isTesting = true
        testResult = nil
        let success = await service.testWebhook(webhookId: webhook.id)
        testResult = success ? "Success — test delivery sent" : "Failed — \(service.error ?? "unknown error")"
        // Refresh deliveries to show the test delivery
        await service.listDeliveries(webhookId: webhook.id)
        isTesting = false
    }

    private func formatTimestamp(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: isoString) else {
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: isoString) else { return isoString }
            return displayFormat(date)
        }
        return displayFormat(date)
    }

    private func displayFormat(_ date: Date) -> String {
        let display = DateFormatter()
        display.dateStyle = .medium
        display.timeStyle = .short
        return display.string(from: date)
    }
}

/// Row showing a single delivery attempt.
struct DeliveryRowView: View {
    let delivery: WebhookDelivery

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            Image(systemName: delivery.success ? "checkmark.circle.fill" : "xmark.circle.fill")
                .foregroundStyle(delivery.success ? .green : .red)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(delivery.eventType)
                    .font(DiraigentTheme.bodyFont)
                    .lineLimit(1)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let status = delivery.responseStatus {
                        Text("HTTP \(status)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    if let deliveredAt = delivery.deliveredAt {
                        Text(formatTimestamp(deliveredAt))
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Spacer()
        }
    }

    private func formatTimestamp(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: isoString) else {
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: isoString) else { return isoString }
            return displayFormat(date)
        }
        return displayFormat(date)
    }

    private func displayFormat(_ date: Date) -> String {
        let display = DateFormatter()
        display.dateStyle = .medium
        display.timeStyle = .short
        return display.string(from: date)
    }
}
