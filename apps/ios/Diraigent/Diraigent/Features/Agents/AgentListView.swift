import SwiftUI

/// List of all agents with status indicators and auto-refresh.
struct AgentListView: View {
    @Environment(AppState.self) private var appState
    @State private var refreshTimer: Timer?

    private var agentsService: AgentsService { appState.agentsService }

    var body: some View {
        Group {
            if agentsService.isLoading && agentsService.agents.isEmpty {
                ProgressView("Loading agents...")
            } else if let error = agentsService.error, agentsService.agents.isEmpty {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") {
                        Task { await agentsService.fetchAgents() }
                    }
                }
            } else if agentsService.agents.isEmpty {
                ContentUnavailableView(
                    "No Agents",
                    systemImage: "cpu",
                    description: Text("No agents are registered yet.")
                )
            } else {
                List(agentsService.agents) { agent in
                    NavigationLink(value: agent.id) {
                        AgentRowView(agent: agent)
                    }
                }
                .refreshable {
                    await agentsService.fetchAgents()
                }
            }
        }
        .navigationTitle("Agents")
        .navigationDestination(for: UUID.self) { agentId in
            if let agent = agentsService.agents.first(where: { $0.id == agentId }) {
                AgentDetailView(agent: agent)
            }
        }
        .task {
            await agentsService.fetchAgents()
        }
        .onAppear { startAutoRefresh() }
        .onDisappear { stopAutoRefresh() }
    }

    private func startAutoRefresh() {
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { _ in
            Task { @MainActor in
                await agentsService.fetchAgents()
            }
        }
    }

    private func stopAutoRefresh() {
        refreshTimer?.invalidate()
        refreshTimer = nil
    }
}

/// Row for a single agent in the list.
struct AgentRowView: View {
    let agent: Agent

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            // Status indicator
            Circle()
                .fill(DiraigentTheme.agentStatusColor(agent.status))
                .frame(width: 10, height: 10)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(agent.name)
                    .font(DiraigentTheme.headlineFont)

                if let capabilities = agent.capabilities, !capabilities.isEmpty {
                    HStack(spacing: DiraigentTheme.spacingXS) {
                        ForEach(capabilities.prefix(3), id: \.self) { cap in
                            Text(cap)
                                .font(.caption2)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.secondary.opacity(0.12))
                                .clipShape(Capsule())
                        }
                        if capabilities.count > 3 {
                            Text("+\(capabilities.count - 3)")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }

            Spacer()

            VStack(alignment: .trailing, spacing: DiraigentTheme.spacingXS) {
                Text(agent.status ?? "unknown")
                    .font(.caption)
                    .foregroundStyle(DiraigentTheme.agentStatusColor(agent.status))

                if let lastSeen = agent.lastSeenAt {
                    Text(formatRelativeTime(lastSeen))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }

    private func formatRelativeTime(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: isoString) else {
            // Try without fractional seconds
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: isoString) else { return isoString }
            return formatRelative(date)
        }
        return formatRelative(date)
    }

    private func formatRelative(_ date: Date) -> String {
        let relFormatter = RelativeDateTimeFormatter()
        relFormatter.unitsStyle = .abbreviated
        return relFormatter.localizedString(for: date, relativeTo: Date())
    }
}
