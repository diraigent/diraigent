import SwiftUI

/// Detail view for a single integration.
struct IntegrationDetailView: View {
    @Environment(AppState.self) private var appState
    let integration: Integration

    @State private var isToggling = false

    private var service: IntegrationsService { appState.integrationsService }

    var body: some View {
        List {
            // MARK: - Header
            Section {
                HStack(spacing: DiraigentTheme.spacingMD) {
                    Image(systemName: kindIcon)
                        .font(.title2)
                        .foregroundStyle(kindColor)

                    VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                        if let kind = integration.kind {
                            IntegrationKindBadge(kind: kind)
                        }
                        HStack(spacing: DiraigentTheme.spacingSM) {
                            Image(systemName: integration.enabled ? "checkmark.circle.fill" : "xmark.circle.fill")
                                .foregroundStyle(integration.enabled ? .green : .red)
                            Text(integration.enabled ? "Enabled" : "Disabled")
                                .font(DiraigentTheme.bodyFont)
                                .foregroundStyle(integration.enabled ? .green : .red)
                        }
                    }

                    Spacer()
                }
            }

            // MARK: - Details
            Section("Details") {
                if let kind = integration.kind {
                    HStack {
                        Text("Kind")
                        Spacer()
                        Text(kind.capitalized)
                            .foregroundStyle(.secondary)
                    }
                }

                if let provider = integration.provider {
                    HStack {
                        Text("Provider")
                        Spacer()
                        Text(provider)
                            .foregroundStyle(.secondary)
                    }
                }

                if let baseUrl = integration.baseUrl, !baseUrl.isEmpty {
                    HStack {
                        Text("Base URL")
                        Spacer()
                        Text(baseUrl)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }

                if let authType = integration.authType {
                    HStack {
                        Text("Auth Type")
                        Spacer()
                        Text(authType.capitalized)
                            .foregroundStyle(.secondary)
                    }
                }

                if let createdAt = integration.createdAt {
                    HStack {
                        Text("Created")
                        Spacer()
                        Text(formatTimestamp(createdAt))
                            .foregroundStyle(.secondary)
                    }
                }

                if let updatedAt = integration.updatedAt {
                    HStack {
                        Text("Updated")
                        Spacer()
                        Text(formatTimestamp(updatedAt))
                            .foregroundStyle(.secondary)
                    }
                }
            }

            // MARK: - Capabilities
            if let capabilities = integration.capabilities, !capabilities.isEmpty {
                Section("Capabilities") {
                    ForEach(capabilities, id: \.self) { capability in
                        HStack {
                            Image(systemName: "checkmark.seal")
                                .foregroundStyle(.tint)
                            Text(capability)
                                .font(DiraigentTheme.bodyFont)
                        }
                    }
                }
            }

            // MARK: - Metadata
            if let config = integration.config, !config.isEmpty {
                Section("Metadata") {
                    ForEach(Array(config.keys.sorted()), id: \.self) { key in
                        HStack {
                            Text(key)
                                .font(DiraigentTheme.bodyFont)
                            Spacer()
                            Text(String(describing: config[key]?.value ?? ""))
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                    }
                }
            }

            // MARK: - Actions
            Section {
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
                                integration.enabled ? "Disable Integration" : "Enable Integration",
                                systemImage: integration.enabled ? "xmark.circle" : "checkmark.circle"
                            )
                        }
                        Spacer()
                    }
                }
                .foregroundStyle(integration.enabled ? .red : .green)
                .disabled(isToggling)
            }
        }
        .navigationTitle(integration.name)
    }

    private func toggleEnabled() async {
        isToggling = true
        _ = await service.toggleIntegration(
            integrationId: integration.id,
            enabled: !integration.enabled
        )
        // Refresh the list to pick up updated state
        if let projectId = appState.selectedProjectId {
            await service.fetchIntegrations(projectId: projectId)
        }
        isToggling = false
    }

    private var kindIcon: String {
        switch (integration.kind ?? "").lowercased() {
        case "ci": "gearshape.2"
        case "monitoring": "chart.bar"
        case "logging": "doc.text"
        case "vcs": "arrow.triangle.branch"
        case "chat": "bubble.left.and.bubble.right"
        case "custom": "puzzlepiece.extension"
        default: "puzzlepiece.extension"
        }
    }

    private var kindColor: Color {
        switch (integration.kind ?? "").lowercased() {
        case "ci": .orange
        case "monitoring": .blue
        case "logging": .purple
        case "vcs": .green
        case "chat": .teal
        case "custom": .secondary
        default: .secondary
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
