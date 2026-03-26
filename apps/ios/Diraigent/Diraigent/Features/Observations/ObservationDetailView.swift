import SwiftUI

/// Detail view for a single observation.
struct ObservationDetailView: View {
    @Environment(AppState.self) private var appState
    let observation: DgObservation

    @State private var isActioning = false

    var body: some View {
        List {
            // MARK: - Header
            Section {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
                    HStack(spacing: DiraigentTheme.spacingMD) {
                        KindIcon(kind: observation.kind ?? "insight")
                            .font(.title2)

                        VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                            if let kind = observation.kind {
                                Text(kind.capitalized)
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(kindColor)
                            }
                            if let status = observation.status {
                                Text(status)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }

                        Spacer()

                        if let severity = observation.severity {
                            SeverityBadge(severity: severity)
                        }
                    }
                }
            }

            // MARK: - Description
            if let description = observation.description, !description.isEmpty {
                Section("Description") {
                    Text(description)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            // MARK: - Evidence
            if let evidence = observation.evidence, !evidence.isEmpty {
                Section("Evidence") {
                    ForEach(Array(evidence.keys.sorted()), id: \.self) { key in
                        HStack {
                            Text(key)
                                .font(.callout.weight(.medium))
                            Spacer()
                            Text(evidence[key]?.stringValue ?? "-")
                                .foregroundStyle(.secondary)
                                .font(.callout)
                        }
                    }
                }
            }

            // MARK: - Metadata
            Section("Details") {
                if let source = observation.source, !source.isEmpty {
                    HStack {
                        Text("Source")
                        Spacer()
                        Text(source)
                            .foregroundStyle(.secondary)
                    }
                }

                if let createdAt = observation.createdAt {
                    HStack {
                        Text("Created")
                        Spacer()
                        Text(formatTimestamp(createdAt))
                            .foregroundStyle(.secondary)
                    }
                }
            }

            // MARK: - Actions
            if observation.status == "open" {
                Section("Actions") {
                    Button {
                        Task { await dismissObservation() }
                    } label: {
                        Label("Dismiss", systemImage: "xmark.circle")
                    }
                    .tint(.red)
                    .disabled(isActioning)

                    Button {
                        Task { await promoteObservation() }
                    } label: {
                        Label("Promote to Task", systemImage: "arrow.up.circle")
                    }
                    .tint(.blue)
                    .disabled(isActioning)
                }
            }
        }
        .navigationTitle(observation.title)
    }

    private var kindColor: Color {
        switch (observation.kind ?? "").lowercased() {
        case "insight": .blue
        case "risk": .red
        case "smell": .orange
        case "improvement": .green
        default: .secondary
        }
    }

    private func dismissObservation() async {
        guard let projectId = appState.selectedProjectId else { return }
        isActioning = true
        _ = await appState.observationsService.dismissObservation(
            projectId: projectId,
            observationId: observation.id
        )
        isActioning = false
    }

    private func promoteObservation() async {
        guard let projectId = appState.selectedProjectId else { return }
        isActioning = true
        _ = await appState.observationsService.promoteObservation(
            projectId: projectId,
            observationId: observation.id
        )
        isActioning = false
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
