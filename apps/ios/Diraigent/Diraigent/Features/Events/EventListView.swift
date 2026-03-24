import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct EventID: Hashable {
    let id: UUID
}

/// List of events with kind and severity filtering.
struct EventListView: View {
    @Environment(AppState.self) private var appState

    @State private var kindFilter: String = "all"
    @State private var severityFilter: String = "all"
    @State private var showingCreateSheet = false

    private static let kindFilters = ["all", "ci", "deploy", "error", "merge", "release", "alert", "custom"]
    private static let severityFilters = ["all", "info", "warning", "error", "critical"]

    private var eventsService: EventsService { appState.eventsService }

    private var filteredEvents: [Event] {
        eventsService.events.filter { event in
            let matchesKind = kindFilter == "all" || (event.kind ?? "") == kindFilter
            let matchesSeverity = severityFilter == "all" || (event.severity ?? "info") == severityFilter
            return matchesKind && matchesSeverity
        }
    }

    var body: some View {
        Group {
            if eventsService.isLoading && eventsService.events.isEmpty {
                LoadingView("Loading events...")
            } else if let error = eventsService.error, eventsService.events.isEmpty {
                ErrorView(error) {
                    Task { await loadEvents() }
                }
            } else if eventsService.events.isEmpty {
                EmptyStateView(
                    icon: "bell",
                    title: "No Events",
                    subtitle: "No events have been recorded yet."
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

                    // Severity filter
                    Picker("Severity", selection: $severityFilter) {
                        ForEach(Self.severityFilters, id: \.self) { severity in
                            Text(severity.capitalized).tag(severity)
                        }
                    }
                    .pickerStyle(.segmented)
                    .padding(.horizontal)
                    .padding(.bottom, DiraigentTheme.spacingSM)

                    List {
                        ForEach(filteredEvents) { event in
                            NavigationLink(value: EventID(id: event.id)) {
                                EventRowView(event: event)
                            }
                        }
                        .onDelete { offsets in
                            Task { await deleteEvents(at: offsets) }
                        }
                    }
                }
                .refreshable {
                    await loadEvents()
                }
            }
        }
        .navigationTitle("Events")
        .navigationDestination(for: EventID.self) { eventId in
            if let event = eventsService.events.first(where: { $0.id == eventId.id }) {
                EventDetailView(event: event)
            }
        }
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
            CreateEventView()
        }
        .task {
            await loadEvents()
        }
    }

    private func loadEvents() async {
        guard let projectId = appState.selectedProjectId else { return }
        await eventsService.fetchEvents(projectId: projectId)
    }

    private func deleteEvents(at offsets: IndexSet) async {
        guard let projectId = appState.selectedProjectId else { return }
        for index in offsets {
            let event = filteredEvents[index]
            _ = await eventsService.deleteEvent(projectId: projectId, eventId: event.id)
        }
    }
}

/// Row for a single event.
struct EventRowView: View {
    let event: Event

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(event.title ?? "Untitled Event")
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(2)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let kind = event.kind {
                        EventKindBadge(kind: kind)
                    }

                    if let severity = event.severity {
                        EventSeverityBadge(severity: severity)
                    }
                }

                if let date = event.createdAt {
                    Text(formatTime(date))
                        .font(.caption)
                        .foregroundStyle(.secondary)
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

/// Colored badge for event kinds.
struct EventKindBadge: View {
    let kind: String

    private var color: Color {
        switch kind.lowercased() {
        case "ci": .blue
        case "deploy": .purple
        case "error": .red
        case "merge": .green
        case "release": .teal
        case "alert": .orange
        case "custom": .secondary
        default: .secondary
        }
    }

    private var icon: String {
        switch kind.lowercased() {
        case "ci": "gearshape.2"
        case "deploy": "arrow.up.circle"
        case "error": "exclamationmark.triangle"
        case "merge": "arrow.triangle.merge"
        case "release": "tag"
        case "alert": "bell.badge"
        case "custom": "circle.fill"
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

/// Colored badge for event severity.
struct EventSeverityBadge: View {
    let severity: String

    private var color: Color {
        switch severity.lowercased() {
        case "critical": .red
        case "error": .red
        case "warning": .orange
        case "info": .blue
        default: .secondary
        }
    }

    private var icon: String {
        switch severity.lowercased() {
        case "critical": "exclamationmark.octagon.fill"
        case "error": "xmark.circle.fill"
        case "warning": "exclamationmark.triangle.fill"
        case "info": "info.circle.fill"
        default: "circle.fill"
        }
    }

    var body: some View {
        Label(severity, systemImage: icon)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}
