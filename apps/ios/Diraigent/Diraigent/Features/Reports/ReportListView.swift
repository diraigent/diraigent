import SwiftUI

/// Typed navigation wrapper to avoid UUID-based navigationDestination conflicts.
struct ReportID: Hashable {
    let id: UUID
}

/// List of reports with status badges, kind badges, and swipe-to-delete.
struct ReportListView: View {
    @Environment(AppState.self) private var appState
    @State private var showingCreateSheet = false

    private var reportsService: ReportsService { appState.reportsService }

    var body: some View {
        Group {
            if reportsService.isLoading && reportsService.reports.isEmpty {
                ProgressView("Loading reports...")
            } else if let error = reportsService.error, reportsService.reports.isEmpty {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") {
                        Task { await loadReports() }
                    }
                }
            } else if reportsService.reports.isEmpty {
                ContentUnavailableView(
                    "No Reports",
                    systemImage: "doc.text.magnifyingglass",
                    description: Text("No reports created yet.")
                )
            } else {
                List(reportsService.reports) { report in
                    NavigationLink(value: ReportID(id: report.id)) {
                        ReportRowView(report: report)
                    }
                    .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                        Button(role: .destructive) {
                            Task { await deleteReport(report) }
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                }
                .refreshable {
                    await loadReports()
                }
            }
        }
        .navigationTitle("Reports")
        .navigationDestination(for: ReportID.self) { reportId in
            if let report = reportsService.reports.first(where: { $0.id == reportId.id }) {
                ReportDetailView(report: report)
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
            CreateReportView()
                .environment(appState)
        }
        .task {
            await loadReports()
        }
    }

    private func loadReports() async {
        guard let projectId = appState.selectedProjectId else { return }
        await reportsService.fetchReports(projectId: projectId)
    }

    private func deleteReport(_ report: Report) async {
        guard let projectId = appState.selectedProjectId else { return }
        _ = await reportsService.deleteReport(projectId: projectId, reportId: report.id)
    }
}

/// Row for a single report.
struct ReportRowView: View {
    let report: Report

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(report.title)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(2)

                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let status = report.status {
                        ReportStatusBadge(status: status)
                    }
                    if let kind = report.kind {
                        ReportKindBadge(kind: kind)
                    }
                }

                if let createdAt = report.createdAt {
                    Text(formatTimestamp(createdAt))
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
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

/// Colored badge for report status.
struct ReportStatusBadge: View {
    let status: String

    private var color: Color {
        switch status.lowercased() {
        case "completed": .green
        case "in_progress": .yellow
        case "pending": .blue
        case "failed": .red
        default: .secondary
        }
    }

    private var displayText: String {
        status.replacingOccurrences(of: "_", with: " ").lowercased()
    }

    var body: some View {
        Text(displayText)
            .font(.caption2.weight(.semibold))
            .textCase(.uppercase)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}

/// Colored badge for report kind.
struct ReportKindBadge: View {
    let kind: String

    private var color: Color {
        switch kind.lowercased() {
        case "security": .red
        case "component": .blue
        case "architecture": .purple
        case "performance": .orange
        case "custom": .secondary
        default: .secondary
        }
    }

    var body: some View {
        Text(kind.lowercased())
            .font(.caption2.weight(.semibold))
            .textCase(.uppercase)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}
