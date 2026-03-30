import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct AuditEntryID: Hashable {
    let id: UUID
}

/// List of audit log entries with entity type filtering.
struct AuditListView: View {
    @Environment(AppState.self) private var appState

    @State private var entityTypeFilter: String = "all"

    private static let entityTypeFilters = ["all", "task", "work", "decision", "observation", "member", "role"]

    private var auditService: AuditService { appState.auditService }

    private var filteredEntries: [AuditEntry] {
        auditService.entries.filter { entry in
            entityTypeFilter == "all" || (entry.entityType ?? "") == entityTypeFilter
        }
    }

    var body: some View {
        Group {
            if auditService.isLoading && auditService.entries.isEmpty {
                LoadingView("Loading audit log...")
            } else if let error = auditService.error, auditService.entries.isEmpty {
                ErrorView(error) {
                    Task { await loadAudit() }
                }
            } else if auditService.entries.isEmpty {
                EmptyStateView(
                    icon: "clock.arrow.circlepath",
                    title: "No Audit Entries",
                    subtitle: "No audit log entries have been recorded yet."
                )
            } else {
                VStack(spacing: 0) {
                    // Entity type filter
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: DiraigentTheme.spacingSM) {
                            ForEach(Self.entityTypeFilters, id: \.self) { type in
                                FilterChip(
                                    title: type.capitalized,
                                    isSelected: entityTypeFilter == type
                                ) {
                                    entityTypeFilter = type
                                }
                            }
                        }
                        .padding(.horizontal)
                    }
                    .padding(.vertical, DiraigentTheme.spacingSM)

                    List {
                        ForEach(filteredEntries) { entry in
                            NavigationLink(value: AuditEntryID(id: entry.id)) {
                                AuditRowView(entry: entry)
                            }
                        }

                        // Pagination: load more
                        if auditService.hasMore && entityTypeFilter == "all" {
                            HStack {
                                Spacer()
                                if auditService.isLoadingMore {
                                    ProgressView()
                                        .controlSize(.small)
                                } else {
                                    Button("Load More") {
                                        Task { await loadMore() }
                                    }
                                    .font(.caption)
                                }
                                Spacer()
                            }
                            .listRowSeparator(.hidden)
                            .onAppear {
                                Task { await loadMore() }
                            }
                        }
                    }
                }
                .refreshable {
                    await loadAudit()
                }
            }
        }
        .navigationTitle("Audit Log")
        .navigationDestination(for: AuditEntryID.self) { entryId in
            if let entry = auditService.entries.first(where: { $0.id == entryId.id }) {
                AuditDetailView(entry: entry)
            }
        }
        .task {
            await loadAudit()
        }
    }

    private func loadAudit() async {
        guard let projectId = appState.selectedProjectId else { return }
        await auditService.fetchAuditEntries(projectId: projectId)
    }

    private func loadMore() async {
        guard let projectId = appState.selectedProjectId else { return }
        await auditService.loadMore(projectId: projectId)
    }
}

// MARK: - Row

/// Row for a single audit entry.
struct AuditRowView: View {
    let entry: AuditEntry

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let time = entry.createdAt {
                        Text(formatTime(time))
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                    }

                    if let action = entry.action {
                        Text(action)
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.primary)
                    }
                }

                if let entityType = entry.entityType {
                    AuditEntityTypeBadge(entityType: entityType)
                }

                if let summary = entry.summary {
                    Text(summary.prefix(60) + (summary.count > 60 ? "..." : ""))
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
            }

            Spacer()
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }

    private func formatTime(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: isoString) else {
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: isoString) else { return isoString }
            return timeOnly(date)
        }
        return timeOnly(date)
    }

    private func timeOnly(_ date: Date) -> String {
        let display = DateFormatter()
        display.dateFormat = "HH:mm"
        return display.string(from: date)
    }
}

// MARK: - Entity Type Badge

/// Color-coded badge for audit entity types.
struct AuditEntityTypeBadge: View {
    let entityType: String

    private var color: Color {
        switch entityType.lowercased() {
        case "task": .blue
        case "work": .green
        case "decision": .purple
        case "observation": .orange
        case "member": .teal
        case "role": .yellow
        default: .secondary
        }
    }

    private var icon: String {
        switch entityType.lowercased() {
        case "task": "checklist"
        case "work": "hammer"
        case "decision": "scale.3d"
        case "observation": "eye"
        case "member": "person"
        case "role": "person.badge.key"
        default: "circle.fill"
        }
    }

    var body: some View {
        Label(entityType, systemImage: icon)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}
