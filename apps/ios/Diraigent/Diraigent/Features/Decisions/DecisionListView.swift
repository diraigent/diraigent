import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct DecisionID: Hashable {
    let id: UUID
}

/// List of decisions with status filtering.
struct DecisionListView: View {
    @Environment(AppState.self) private var appState

    @State private var statusFilter: String = "all"

    private static let statusFilters = ["all", "proposed", "accepted", "rejected", "superseded"]

    private var decisionsService: DecisionsService { appState.decisionsService }

    private var filteredDecisions: [Decision] {
        if statusFilter == "all" {
            return decisionsService.decisions
        }
        return decisionsService.decisions.filter { ($0.status ?? "proposed") == statusFilter }
    }

    var body: some View {
        Group {
            if decisionsService.isLoading && decisionsService.decisions.isEmpty {
                ProgressView("Loading decisions...")
            } else if let error = decisionsService.error, decisionsService.decisions.isEmpty {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") {
                        Task { await loadDecisions() }
                    }
                }
            } else if decisionsService.decisions.isEmpty {
                ContentUnavailableView(
                    "No Decisions",
                    systemImage: "scale.3d",
                    description: Text("No decisions recorded yet.")
                )
            } else {
                VStack(spacing: 0) {
                    // Status filter
                    Picker("Status", selection: $statusFilter) {
                        ForEach(Self.statusFilters, id: \.self) { status in
                            Text(status.capitalized).tag(status)
                        }
                    }
                    .pickerStyle(.segmented)
                    .padding(.horizontal)
                    .padding(.vertical, DiraigentTheme.spacingSM)

                    List(filteredDecisions) { decision in
                        NavigationLink(value: DecisionID(id: decision.id)) {
                            DecisionRowView(decision: decision)
                        }
                    }
                }
            }
        }
        .navigationTitle("Decisions")
        .navigationDestination(for: DecisionID.self) { decisionId in
            if let decision = decisionsService.decisions.first(where: { $0.id == decisionId.id }) {
                DecisionDetailView(decision: decision)
            }
        }
        .task {
            await loadDecisions()
        }
    }

    private func loadDecisions() async {
        guard let projectId = appState.selectedProjectId else { return }
        await decisionsService.fetchDecisions(projectId: projectId)
    }
}

/// Row for a single decision.
struct DecisionRowView: View {
    let decision: Decision

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(decision.title)
                    .font(DiraigentTheme.headlineFont)

                if let date = decision.createdAt {
                    Text(formatDate(date))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            DecisionStatusBadge(status: decision.status ?? "proposed")
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
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
        display.timeStyle = .none
        return display.string(from: date)
    }
}

/// Colored badge for decision statuses.
struct DecisionStatusBadge: View {
    let status: String

    private var color: Color {
        switch status.lowercased() {
        case "accepted": .green
        case "rejected": .red
        case "superseded": .purple
        case "proposed": .blue
        default: .secondary
        }
    }

    var body: some View {
        Text(status)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}
