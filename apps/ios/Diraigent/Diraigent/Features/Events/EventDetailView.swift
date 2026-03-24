import SwiftUI

/// Detail view for a single event.
struct EventDetailView: View {
    @Environment(AppState.self) private var appState
    let event: Event

    var body: some View {
        List {
            // MARK: - Header
            Section {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
                    HStack(spacing: DiraigentTheme.spacingSM) {
                        if let kind = event.kind {
                            EventKindBadge(kind: kind)
                        }
                        if let severity = event.severity {
                            EventSeverityBadge(severity: severity)
                        }
                        Spacer()
                        if let date = event.createdAt {
                            Text(formatDate(date))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }

            // MARK: - Description
            if let description = event.description, !description.isEmpty {
                Section("Description") {
                    Text(description)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            // MARK: - Details
            Section("Details") {
                if let source = event.source, !source.isEmpty {
                    LabeledContent("Source", value: source)
                }

                if let agentId = event.agentId {
                    let agentName = appState.agentsService.agents.first(where: { $0.id == agentId })?.name
                    LabeledContent("Agent", value: agentName ?? agentId.uuidString.prefix(8).description)
                }

                if let taskId = event.relatedTaskId {
                    let task = appState.tasksService.tasks.first(where: { $0.id == taskId })
                    if let task {
                        LabeledContent("Task") {
                            if let number = task.number {
                                Text("#\(number) \(task.title)")
                                    .foregroundStyle(.blue)
                            } else {
                                Text(task.title)
                                    .foregroundStyle(.blue)
                            }
                        }
                    } else {
                        LabeledContent("Task ID", value: taskId.uuidString.prefix(12).description)
                    }
                }

                if let kind = event.kind {
                    LabeledContent("Kind", value: kind)
                }

                if let severity = event.severity {
                    LabeledContent("Severity", value: severity)
                }
            }

            // MARK: - Metadata
            if let metadata = event.metadata, !metadata.isEmpty {
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
        .navigationTitle(event.title ?? "Event")
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
        display.timeStyle = .short
        return display.string(from: date)
    }
}
