import SwiftUI

/// Detail view for an individual agent.
struct AgentDetailView: View {
    @Environment(AppState.self) private var appState
    let agent: Agent

    @State private var tasks: [DgTask] = []
    @State private var isLoadingTasks = false

    var body: some View {
        List {
            // MARK: - Status Section
            Section {
                HStack {
                    Text("Status")
                    Spacer()
                    HStack(spacing: DiraigentTheme.spacingXS) {
                        Circle()
                            .fill(DiraigentTheme.agentStatusColor(agent.status))
                            .frame(width: 8, height: 8)
                        Text(agent.status ?? "unknown")
                            .foregroundStyle(DiraigentTheme.agentStatusColor(agent.status))
                    }
                }

                if let lastSeen = agent.lastSeenAt {
                    HStack {
                        Text("Last Seen")
                        Spacer()
                        Text(formatTimestamp(lastSeen))
                            .foregroundStyle(.secondary)
                    }
                }
            }

            // MARK: - Capabilities Section
            if let capabilities = agent.capabilities, !capabilities.isEmpty {
                Section("Capabilities") {
                    FlowLayout(spacing: DiraigentTheme.spacingSM) {
                        ForEach(capabilities, id: \.self) { cap in
                            Text(cap)
                                .font(.callout)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(DiraigentTheme.primary.opacity(0.1))
                                .foregroundStyle(DiraigentTheme.primary)
                                .clipShape(Capsule())
                        }
                    }
                    .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
                }
            }

            // MARK: - Metadata Section
            if let metadata = agent.metadata, !metadata.isEmpty {
                Section("Metadata") {
                    ForEach(Array(metadata.keys.sorted()), id: \.self) { key in
                        HStack {
                            Text(key)
                            Spacer()
                            Text(metadata[key]?.stringValue ?? "-")
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }

            // MARK: - Current Tasks
            Section("Assigned Tasks") {
                if isLoadingTasks {
                    ProgressView("Loading tasks...")
                } else if tasks.isEmpty {
                    Text("No tasks assigned")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(tasks) { task in
                        HStack {
                            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                                Text(task.title)
                                    .font(DiraigentTheme.headlineFont)
                                if let kind = task.kind {
                                    Text(kind)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Spacer()
                            StateBadge(state: task.state)
                        }
                    }
                }
            }
        }
        .navigationTitle(agent.name)
        .task {
            isLoadingTasks = true
            tasks = await appState.agentsService.fetchAgentTasks(agentId: agent.id)
            isLoadingTasks = false
        }
    }

    private func formatTimestamp(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: isoString) else {
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: isoString) else { return isoString }
            return formatDate(date)
        }
        return formatDate(date)
    }

    private func formatDate(_ date: Date) -> String {
        let display = DateFormatter()
        display.dateStyle = .medium
        display.timeStyle = .short
        return display.string(from: date)
    }
}

/// Simple flow layout for tags/chips.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = layoutSubviews(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = layoutSubviews(proposal: proposal, subviews: subviews)
        for (index, position) in result.positions.enumerated() {
            subviews[index].place(
                at: CGPoint(x: bounds.minX + position.x, y: bounds.minY + position.y),
                proposal: ProposedViewSize(result.sizes[index])
            )
        }
    }

    private struct LayoutResult {
        var positions: [CGPoint]
        var sizes: [CGSize]
        var size: CGSize
    }

    private func layoutSubviews(proposal: ProposedViewSize, subviews: Subviews) -> LayoutResult {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var sizes: [CGSize] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var maxX: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > maxWidth, x > 0 {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            positions.append(CGPoint(x: x, y: y))
            sizes.append(size)
            rowHeight = max(rowHeight, size.height)
            x += size.width + spacing
            maxX = max(maxX, x - spacing)
        }

        return LayoutResult(
            positions: positions,
            sizes: sizes,
            size: CGSize(width: maxX, height: y + rowHeight)
        )
    }
}
