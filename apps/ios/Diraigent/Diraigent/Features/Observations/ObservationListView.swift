import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct ObservationID: Hashable {
    let id: UUID
}

/// List of observations with kind/severity filtering and swipe actions.
struct ObservationListView: View {
    @Environment(AppState.self) private var appState

    @State private var kindFilter: String = "all"
    @State private var severityFilter: String = "all"

    private static let kindFilters = ["all", "insight", "risk", "smell", "improvement"]
    private static let severityFilters = ["all", "critical", "high", "medium", "low", "info"]

    private var observationsService: ObservationsService { appState.observationsService }

    private var filteredObservations: [DgObservation] {
        var result = observationsService.observations
        if kindFilter != "all" {
            result = result.filter { ($0.kind ?? "") == kindFilter }
        }
        if severityFilter != "all" {
            result = result.filter { ($0.severity ?? "") == severityFilter }
        }
        return result
    }

    var body: some View {
        Group {
            if observationsService.isLoading && observationsService.observations.isEmpty {
                ProgressView("Loading observations...")
            } else if let error = observationsService.error, observationsService.observations.isEmpty {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") {
                        Task { await loadObservations() }
                    }
                }
            } else if observationsService.observations.isEmpty {
                ContentUnavailableView(
                    "No Observations",
                    systemImage: "eye",
                    description: Text("No observations recorded yet.")
                )
            } else {
                VStack(spacing: 0) {
                    // Filters
                    VStack(spacing: DiraigentTheme.spacingSM) {
                        Picker("Kind", selection: $kindFilter) {
                            ForEach(Self.kindFilters, id: \.self) { kind in
                                Text(kind.capitalized).tag(kind)
                            }
                        }
                        .pickerStyle(.segmented)

                        Picker("Severity", selection: $severityFilter) {
                            ForEach(Self.severityFilters, id: \.self) { sev in
                                Text(sev.capitalized).tag(sev)
                            }
                        }
                        .pickerStyle(.segmented)
                    }
                    .padding(.horizontal)
                    .padding(.vertical, DiraigentTheme.spacingSM)

                    List(filteredObservations) { observation in
                        NavigationLink(value: ObservationID(id: observation.id)) {
                            ObservationRowView(observation: observation)
                        }
                        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                            Button(role: .destructive) {
                                Task { await dismiss(observation) }
                            } label: {
                                Label("Dismiss", systemImage: "xmark.circle")
                            }
                        }
                        .swipeActions(edge: .leading, allowsFullSwipe: false) {
                            Button {
                                Task { await promote(observation) }
                            } label: {
                                Label("Promote", systemImage: "arrow.up.circle")
                            }
                            .tint(.blue)
                        }
                    }
                    .refreshable {
                        await loadObservations()
                    }
                }
            }
        }
        .navigationTitle("Observations")
        .navigationDestination(for: ObservationID.self) { obsId in
            if let obs = observationsService.observations.first(where: { $0.id == obsId.id }) {
                ObservationDetailView(observation: obs)
            }
        }
        .task {
            await loadObservations()
        }
    }

    private func loadObservations() async {
        guard let projectId = appState.selectedProjectId else { return }
        await observationsService.fetchObservations(projectId: projectId)
    }

    private func dismiss(_ observation: DgObservation) async {
        guard let projectId = appState.selectedProjectId else { return }
        _ = await observationsService.dismissObservation(projectId: projectId, observationId: observation.id)
    }

    private func promote(_ observation: DgObservation) async {
        guard let projectId = appState.selectedProjectId else { return }
        _ = await observationsService.promoteObservation(projectId: projectId, observationId: observation.id)
    }
}

/// Row for a single observation.
struct ObservationRowView: View {
    let observation: DgObservation

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            KindIcon(kind: observation.kind ?? "insight")
                .font(.title3)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(observation.title)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(2)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let kind = observation.kind {
                        Text(kind)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    if let status = observation.status {
                        Text(status)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Spacer()

            if let severity = observation.severity {
                SeverityBadge(severity: severity)
            }
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}
