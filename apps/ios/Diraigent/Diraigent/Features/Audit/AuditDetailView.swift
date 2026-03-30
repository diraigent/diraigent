import SwiftUI

/// Detail view for a single audit log entry.
struct AuditDetailView: View {
    @Environment(AppState.self) private var appState
    let entry: AuditEntry

    var body: some View {
        List {
            // MARK: - Header
            Section {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
                    HStack(spacing: DiraigentTheme.spacingSM) {
                        if let entityType = entry.entityType {
                            AuditEntityTypeBadge(entityType: entityType)
                        }
                        Spacer()
                        if let date = entry.createdAt {
                            Text(formatDate(date))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    if let action = entry.action {
                        Text(action)
                            .font(DiraigentTheme.headlineFont)
                    }
                }
            }

            // MARK: - Summary
            if let summary = entry.summary, !summary.isEmpty {
                Section("Summary") {
                    Text(summary)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            // MARK: - Details
            Section("Details") {
                if let entityType = entry.entityType {
                    LabeledContent("Entity Type", value: entityType)
                }

                if let entityId = entry.entityId {
                    LabeledContent("Entity ID", value: entityId.uuidString.prefix(12).description)
                }

                if let action = entry.action {
                    LabeledContent("Action", value: action)
                }

                // Actor
                if let agentId = entry.actorAgentId {
                    let agentName = appState.agentsService.agents.first(where: { $0.id == agentId })?.name
                    LabeledContent("Actor (Agent)", value: agentName ?? agentId.uuidString.prefix(8).description)
                }

                if let userId = entry.actorUserId {
                    LabeledContent("Actor (User)", value: userId.uuidString.prefix(8).description)
                }

                if entry.actorAgentId == nil && entry.actorUserId == nil {
                    LabeledContent("Actor", value: "system")
                }
            }

            // MARK: - Before State
            if let beforeState = entry.beforeState {
                Section("Before State") {
                    Text(prettyPrint(beforeState))
                        .font(.caption.monospaced())
                        .foregroundStyle(.red)
                        .textSelection(.enabled)
                }
            }

            // MARK: - After State
            if let afterState = entry.afterState {
                Section("After State") {
                    Text(prettyPrint(afterState))
                        .font(.caption.monospaced())
                        .foregroundStyle(.green)
                        .textSelection(.enabled)
                }
            }

            // MARK: - Metadata
            if let metadata = entry.metadata, !metadata.isEmpty {
                Section("Metadata") {
                    ForEach(Array(metadata.keys.sorted()), id: \.self) { key in
                        if let value = metadata[key] {
                            LabeledContent(key) {
                                Text(String(describing: value.value))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(3)
                            }
                        }
                    }
                }
            }
        }
        .navigationTitle(entry.action ?? "Audit Entry")
    }

    private func formatDate(_ isoString: String) -> String {
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
        display.timeStyle = .medium
        return display.string(from: date)
    }

    private func prettyPrint(_ value: AnyCodable) -> String {
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(value)
            return String(data: data, encoding: .utf8) ?? String(describing: value.value)
        } catch {
            return String(describing: value.value)
        }
    }
}
