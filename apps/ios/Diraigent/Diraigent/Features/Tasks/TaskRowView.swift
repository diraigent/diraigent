import SwiftUI

/// A single row in the task list showing key task metadata.
struct TaskRowView: View {
    let task: DgTask

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .top) {
                // Priority indicator
                if let priority = task.priority {
                    priorityIndicator(priority)
                }

                VStack(alignment: .leading, spacing: 4) {
                    // Title with urgent flag
                    HStack(spacing: 4) {
                        if task.urgent == true {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .font(.caption)
                                .foregroundStyle(.red)
                        }
                        Text(task.title)
                            .font(.body.weight(.medium))
                            .lineLimit(2)
                    }

                    // Badges row
                    HStack(spacing: 6) {
                        StateBadge(state: task.state)

                        if let kind = task.kind {
                            Text(kind)
                                .font(.caption2)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.secondary.opacity(0.12))
                                .foregroundStyle(.secondary)
                                .clipShape(Capsule())
                        }

                        if let number = task.number {
                            Text("#\(number)")
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                    }
                }

                Spacer()
            }
        }
        .padding(.vertical, 2)
    }

    @ViewBuilder
    private func priorityIndicator(_ priority: Int) -> some View {
        Circle()
            .fill(priorityColor(priority))
            .frame(width: 8, height: 8)
            .padding(.top, 6)
    }

    private func priorityColor(_ priority: Int) -> Color {
        switch priority {
        case 8...10: return .red
        case 6...7: return .orange
        case 4...5: return .yellow
        default: return .gray.opacity(0.5)
        }
    }
}
