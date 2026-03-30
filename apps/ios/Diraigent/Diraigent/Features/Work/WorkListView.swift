import SwiftUI

/// List of work items with kind and status filtering.
struct WorkListView: View {
    @Environment(AppState.self) private var appState

    @State private var kindFilter: String = "all"
    @State private var statusFilter: String = "all"
    @State private var showingCreateSheet = false

    private static let kindFilters = ["all", "epic", "feature", "milestone", "sprint", "initiative"]
    private static let statusFilters = ["all", "active", "achieved", "paused", "abandoned"]

    private var workService: WorkService { appState.workService }

    private var filteredItems: [Work] {
        workService.workItems.filter { item in
            let matchesKind = kindFilter == "all" || (item.workType ?? "") == kindFilter
            let matchesStatus = statusFilter == "all" || (item.status ?? "active") == statusFilter
            return matchesKind && matchesStatus
        }
    }

    var body: some View {
        Group {
            if workService.isLoading && workService.workItems.isEmpty {
                LoadingView("Loading work items...")
            } else if let error = workService.error, workService.workItems.isEmpty {
                ErrorView(error) {
                    Task { await loadWork() }
                }
            } else if workService.workItems.isEmpty {
                EmptyStateView(
                    icon: "hammer",
                    title: "No Work Items",
                    subtitle: "No work items have been created yet."
                )
            } else {
                VStack(spacing: 0) {
                    // Kind filter
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: DiraigentTheme.spacingSM) {
                            ForEach(Self.kindFilters, id: \.self) { kind in
                                FilterChip(
                                    title: kind.capitalized,
                                    isSelected: kindFilter == kind
                                ) {
                                    kindFilter = kind
                                }
                            }
                        }
                        .padding(.horizontal)
                    }
                    .padding(.vertical, DiraigentTheme.spacingSM)

                    // Status filter
                    Picker("Status", selection: $statusFilter) {
                        ForEach(Self.statusFilters, id: \.self) { status in
                            Text(status.capitalized).tag(status)
                        }
                    }
                    .pickerStyle(.segmented)
                    .padding(.horizontal)
                    .padding(.bottom, DiraigentTheme.spacingSM)

                    List(filteredItems) { work in
                        NavigationLink(value: work.id) {
                            WorkRowView(work: work)
                        }
                    }
                }
                .refreshable {
                    await loadWork()
                }
            }
        }
        .navigationTitle("Work")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showingCreateSheet = true
                } label: {
                    Image(systemName: "plus")
                }
            }
        }
        .sheet(isPresented: $showingCreateSheet) {
            CreateWorkView()
        }
        .navigationDestination(for: UUID.self) { workId in
            if let work = workService.workItems.first(where: { $0.id == workId }) {
                WorkDetailView(work: work)
            }
        }
        .task {
            await loadWork()
        }
    }

    private func loadWork() async {
        guard let projectId = appState.selectedProjectId else { return }
        await workService.fetchWork(projectId: projectId)
    }
}

/// Row for a single work item.
struct WorkRowView: View {
    let work: Work

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(work.title)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(2)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let kind = work.workType {
                        WorkKindBadge(kind: kind)
                    }

                    if let status = work.status {
                        WorkStatusBadge(status: status)
                    }
                }
            }

            Spacer()
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}

/// Colored badge for work item kinds.
struct WorkKindBadge: View {
    let kind: String

    private var color: Color {
        switch kind.lowercased() {
        case "epic": .purple
        case "feature": .blue
        case "milestone": .orange
        case "sprint": .green
        case "initiative": .indigo
        default: .secondary
        }
    }

    private var icon: String {
        switch kind.lowercased() {
        case "epic": "bolt.fill"
        case "feature": "star.fill"
        case "milestone": "flag.fill"
        case "sprint": "hare"
        case "initiative": "chart.line.uptrend.xyaxis"
        default: "circle.fill"
        }
    }

    var body: some View {
        Label(kind, systemImage: icon)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}

/// Colored badge for work item statuses.
struct WorkStatusBadge: View {
    let status: String

    private var color: Color {
        switch status.lowercased() {
        case "active": .green
        case "achieved": .blue
        case "paused": .orange
        case "abandoned": .red
        default: .secondary
        }
    }

    var body: some View {
        Text(status)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}


/// Reusable filter chip button.
struct FilterChip: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.caption.weight(.medium))
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(isSelected ? Color.accentColor : Color.secondary.opacity(0.12))
                .foregroundStyle(isSelected ? .white : .primary)
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}
