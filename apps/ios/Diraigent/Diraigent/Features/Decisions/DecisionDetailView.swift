import SwiftUI

/// Detail view for a single decision.
struct DecisionDetailView: View {
    @Environment(AppState.self) private var appState
    let decision: Decision

    @State private var isTransitioning = false

    var body: some View {
        List {
            // MARK: - Header
            Section {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
                    HStack {
                        DecisionStatusBadge(status: decision.status ?? "proposed")
                        Spacer()
                        if let date = decision.createdAt {
                            Text(formatDate(date))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    if let description = decision.description, !description.isEmpty {
                        Text(description)
                            .font(DiraigentTheme.bodyFont)
                    }
                }
            }

            // MARK: - Decision
            if let decisionText = decision.decision, !decisionText.isEmpty {
                Section("Decision") {
                    Text(decisionText)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            // MARK: - Rationale
            if let rationale = decision.rationale, !rationale.isEmpty {
                Section("Rationale") {
                    Text(rationale)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            // MARK: - Alternatives
            if let alternatives = decision.alternatives, !alternatives.isEmpty {
                Section("Alternatives") {
                    ForEach(alternatives, id: \.name) { alt in
                        VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                            Text(alt.name)
                                .font(DiraigentTheme.headlineFont)

                            if let pros = alt.pros, !pros.isEmpty {
                                HStack(alignment: .top, spacing: DiraigentTheme.spacingXS) {
                                    Image(systemName: "plus.circle.fill")
                                        .foregroundStyle(.green)
                                        .font(.caption)
                                    Text(pros)
                                        .font(.callout)
                                }
                            }

                            if let cons = alt.cons, !cons.isEmpty {
                                HStack(alignment: .top, spacing: DiraigentTheme.spacingXS) {
                                    Image(systemName: "minus.circle.fill")
                                        .foregroundStyle(.red)
                                        .font(.caption)
                                    Text(cons)
                                        .font(.callout)
                                }
                            }
                        }
                        .padding(.vertical, DiraigentTheme.spacingXS)
                    }
                }
            }

            // MARK: - Consequences
            if let consequences = decision.consequences, !consequences.isEmpty {
                Section("Consequences") {
                    Text(consequences)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            // MARK: - Tags
            if let tags = decision.tags, !tags.isEmpty {
                Section("Tags") {
                    FlowLayout(spacing: DiraigentTheme.spacingSM) {
                        ForEach(tags, id: \.self) { tag in
                            Text(tag)
                                .font(.caption)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color.secondary.opacity(0.12))
                                .clipShape(Capsule())
                        }
                    }
                    .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
                }
            }

            // MARK: - Actions
            if decision.status != "accepted" || decision.status != "rejected" {
                Section("Actions") {
                    statusTransitionButtons
                }
            }
        }
        .navigationTitle(decision.title)
    }

    @ViewBuilder
    private var statusTransitionButtons: some View {
        let currentStatus = decision.status ?? "proposed"

        if currentStatus == "proposed" {
            Button {
                Task { await transitionStatus(to: "accepted") }
            } label: {
                Label("Accept", systemImage: "checkmark.circle")
            }
            .tint(.green)
            .disabled(isTransitioning)

            Button {
                Task { await transitionStatus(to: "rejected") }
            } label: {
                Label("Reject", systemImage: "xmark.circle")
            }
            .tint(.red)
            .disabled(isTransitioning)
        }

        if currentStatus == "accepted" {
            Button {
                Task { await transitionStatus(to: "superseded") }
            } label: {
                Label("Supersede", systemImage: "arrow.uturn.backward.circle")
            }
            .tint(.purple)
            .disabled(isTransitioning)
        }
    }

    private func transitionStatus(to newStatus: String) async {
        guard let projectId = appState.selectedProjectId else { return }
        isTransitioning = true
        _ = await appState.decisionsService.updateDecision(
            projectId: projectId,
            decisionId: decision.id,
            update: UpdateDecisionRequest(title: nil, status: newStatus, description: nil, rationale: nil, consequences: nil)
        )
        isTransitioning = false
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
