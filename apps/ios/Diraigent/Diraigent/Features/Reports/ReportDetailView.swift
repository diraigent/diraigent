import SwiftUI

/// Detail view for a single report.
struct ReportDetailView: View {
    let report: Report

    var body: some View {
        List {
            // MARK: - Header
            Section {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
                    HStack(spacing: DiraigentTheme.spacingMD) {
                        Image(systemName: kindIcon)
                            .font(.title2)
                            .foregroundStyle(kindColor)

                        VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                            if let kind = report.kind {
                                ReportKindBadge(kind: kind)
                            }
                            if let status = report.status {
                                ReportStatusBadge(status: status)
                            }
                        }

                        Spacer()
                    }
                }
            }

            // MARK: - Prompt
            if let prompt = report.prompt, !prompt.isEmpty {
                Section("Prompt") {
                    Text(prompt)
                        .font(DiraigentTheme.bodyFont)
                        .textSelection(.enabled)
                }
            }

            // MARK: - Result
            if let result = report.result, !result.isEmpty {
                Section("Result") {
                    ScrollView {
                        Text(result)
                            .font(DiraigentTheme.bodyFont)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .frame(maxHeight: 500)
                }
            }

            // MARK: - Details
            Section("Details") {
                if let kind = report.kind {
                    HStack {
                        Text("Kind")
                        Spacer()
                        Text(kind.capitalized)
                            .foregroundStyle(.secondary)
                    }
                }

                if let status = report.status {
                    HStack {
                        Text("Status")
                        Spacer()
                        Text(status.replacingOccurrences(of: "_", with: " ").capitalized)
                            .foregroundStyle(.secondary)
                    }
                }

                if let taskId = report.taskId {
                    HStack {
                        Text("Task")
                        Spacer()
                        Text(taskId.uuidString.prefix(8) + "...")
                            .foregroundStyle(.secondary)
                            .font(.callout.monospaced())
                    }
                }

                if let createdAt = report.createdAt {
                    HStack {
                        Text("Created")
                        Spacer()
                        Text(formatTimestamp(createdAt))
                            .foregroundStyle(.secondary)
                    }
                }

                if let updatedAt = report.updatedAt {
                    HStack {
                        Text("Updated")
                        Spacer()
                        Text(formatTimestamp(updatedAt))
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .navigationTitle(report.title)
    }

    private var kindIcon: String {
        switch (report.kind ?? "").lowercased() {
        case "security": "lock.shield"
        case "component": "square.stack.3d.up"
        case "architecture": "building.columns"
        case "performance": "gauge.with.dots.needle.33percent"
        case "custom": "doc.text"
        default: "doc.text"
        }
    }

    private var kindColor: Color {
        switch (report.kind ?? "").lowercased() {
        case "security": .red
        case "component": .blue
        case "architecture": .purple
        case "performance": .orange
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
