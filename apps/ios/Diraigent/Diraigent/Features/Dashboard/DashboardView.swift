import SwiftUI

/// Project dashboard showing summary metrics, agents, and recent events.
struct DashboardView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            if let projectId = appState.selectedProjectId {
                dashboardContent(projectId: projectId)
            } else {
                noProjectSelected
            }
        }
        .navigationTitle("Dashboard")
    }

    // MARK: - No Project

    private var noProjectSelected: some View {
        ContentUnavailableView(
            "No Project Selected",
            systemImage: "folder",
            description: Text("Tap the project selector to choose a project.")
        )
    }

    // MARK: - Dashboard Content

    @ViewBuilder
    private func dashboardContent(projectId: UUID) -> some View {
        let service = appState.dashboardService
        let project = appState.projectService.selectedProject(id: projectId)

        ScrollView {
            LazyVStack(alignment: .leading, spacing: DiraigentTheme.spacingLG) {
                // Project header
                if let project {
                    projectHeader(project)
                }

                // Task summary
                if let taskSummary = service.metrics?.taskSummary {
                    taskSummarySection(taskSummary)
                }

                // Cost summary
                if let costSummary = service.metrics?.costSummary {
                    costSummarySection(costSummary)
                }

                // Active agents
                if !service.agents.isEmpty {
                    agentsSection(service.agents)
                }

                // Recent events
                if !service.recentEvents.isEmpty {
                    eventsSection(service.recentEvents)
                }
            }
            .padding()
        }
        .overlay {
            if service.isLoading && service.metrics == nil {
                loadingSkeleton
            }
        }
        .refreshable {
            await appState.dashboardService.fetchDashboard(projectId: projectId)
        }
        .task(id: projectId) {
            await appState.dashboardService.fetchDashboard(projectId: projectId)
        }
    }

    // MARK: - Project Header

    private func projectHeader(_ project: Project) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Text(project.name)
                .font(DiraigentTheme.titleFont)
            if let desc = project.description, !desc.isEmpty {
                Text(desc)
                    .font(DiraigentTheme.bodyFont)
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: DiraigentTheme.spacingMD) {
                if let branch = project.defaultBranch {
                    Label(branch, systemImage: "arrow.triangle.branch")
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                }
                if let repo = project.repoUrl {
                    Label(repo, systemImage: "link")
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
        }
    }

    // MARK: - Task Summary

    private func taskSummarySection(_ summary: TaskSummary) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Text("Tasks")
                .font(DiraigentTheme.headlineFont)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
                GridItem(.flexible()),
                GridItem(.flexible())
            ], spacing: DiraigentTheme.spacingSM) {
                taskCard("Backlog", count: summary.backlog ?? 0, color: DiraigentTheme.taskStateColor("backlog"))
                taskCard("Ready", count: summary.ready ?? 0, color: DiraigentTheme.taskStateColor("ready"))
                taskCard("Working", count: summary.inProgress ?? 0, color: DiraigentTheme.taskStateColor("working"))
                taskCard("Done", count: summary.done ?? 0, color: DiraigentTheme.taskStateColor("done"))
            }
        }
    }

    private func taskCard(_ label: String, count: Int, color: Color) -> some View {
        VStack(spacing: DiraigentTheme.spacingXS) {
            Text("\(count)")
                .font(.title2.bold())
                .foregroundStyle(color)
            Text(label)
                .font(DiraigentTheme.captionFont)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, DiraigentTheme.spacingSM)
        .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 10))
    }

    // MARK: - Cost Summary

    private func costSummarySection(_ cost: CostSummary) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Text("Cost")
                .font(DiraigentTheme.headlineFont)

            HStack(spacing: DiraigentTheme.spacingXL) {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                    Text("Total Spent")
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                    Text(String(format: "$%.2f", cost.totalCostUsd ?? 0))
                        .font(.title3.bold())
                        .foregroundStyle(DiraigentTheme.success)
                }

                VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                    Text("Tokens")
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                    Text("\(formatTokens(cost.totalInputTokens ?? 0))in / \(formatTokens(cost.totalOutputTokens ?? 0))out")
                        .font(.callout)
                        .foregroundStyle(.primary)
                }
            }
            .padding(DiraigentTheme.spacingMD)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(DiraigentTheme.surface, in: RoundedRectangle(cornerRadius: 10))
        }
    }

    // MARK: - Agents

    private func agentsSection(_ agents: [Agent]) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Text("Agents")
                .font(DiraigentTheme.headlineFont)

            ForEach(agents) { agent in
                HStack(spacing: DiraigentTheme.spacingMD) {
                    Circle()
                        .fill(DiraigentTheme.agentStatusColor(agent.status))
                        .frame(width: 10, height: 10)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(agent.name)
                            .font(.callout.weight(.medium))
                        Text(agent.status?.capitalized ?? "Unknown")
                            .font(DiraigentTheme.captionFont)
                            .foregroundStyle(.secondary)
                    }

                    Spacer()

                    if let capabilities = agent.capabilities, !capabilities.isEmpty {
                        Text(capabilities.prefix(3).joined(separator: ", "))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                }
                .padding(.vertical, DiraigentTheme.spacingXS)
            }
        }
    }

    // MARK: - Events

    private func eventsSection(_ events: [Event]) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Text("Recent Events")
                .font(DiraigentTheme.headlineFont)

            ForEach(events) { event in
                HStack(alignment: .top, spacing: DiraigentTheme.spacingSM) {
                    Circle()
                        .fill(DiraigentTheme.severityColor(event.severity))
                        .frame(width: 8, height: 8)
                        .padding(.top, 6)

                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            if let kind = event.kind {
                                Text(kind)
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(DiraigentTheme.severityColor(event.severity))
                            }
                            Spacer()
                            if let time = event.createdAt, time.count >= 16 {
                                Text(String(time.dropFirst(11).prefix(5)))
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        if let title = event.title {
                            Text(title)
                                .font(DiraigentTheme.captionFont)
                                .foregroundStyle(.primary)
                                .lineLimit(2)
                        }
                    }
                }
                .padding(.vertical, DiraigentTheme.spacingXS)
            }
        }
    }

    // MARK: - Loading Skeleton

    private var loadingSkeleton: some View {
        VStack(spacing: DiraigentTheme.spacingLG) {
            ProgressView()
            Text("Loading dashboard\u{2026}")
                .font(DiraigentTheme.captionFont)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Helpers

    private func formatTokens(_ n: Int) -> String {
        if n >= 1_000_000 {
            return String(format: "%.1fM", Double(n) / 1_000_000)
        } else if n >= 1_000 {
            return String(format: "%.1fK", Double(n) / 1_000)
        } else {
            return "\(n)"
        }
    }
}
