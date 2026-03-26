import SwiftUI

/// Time range options for log queries.
enum LogTimeRange: String, CaseIterable {
    case m15 = "15m"
    case h1 = "1h"
    case h3 = "3h"
    case h6 = "6h"
    case h12 = "12h"
    case h24 = "24h"
    case d3 = "3d"
    case d7 = "7d"

    /// Convert to seconds for time calculation.
    var seconds: TimeInterval {
        switch self {
        case .m15: 15 * 60
        case .h1: 3600
        case .h3: 3 * 3600
        case .h6: 6 * 3600
        case .h12: 12 * 3600
        case .h24: 24 * 3600
        case .d3: 3 * 24 * 3600
        case .d7: 7 * 24 * 3600
        }
    }
}

/// Limit options for log queries.
enum LogLimit: Int, CaseIterable {
    case l50 = 50
    case l100 = 100
    case l200 = 200
    case l500 = 500
    case l1000 = 1000

    var label: String { String(rawValue) }
}

/// Direction options for log queries.
enum LogDirection: String, CaseIterable {
    case backward
    case forward

    var label: String { rawValue.capitalized }

    var icon: String {
        switch self {
        case .backward: "arrow.down"
        case .forward: "arrow.up"
        }
    }
}

/// Logs view with Loki log query interface.
struct LogsView: View {
    @Environment(AppState.self) private var appState

    @State private var query: String = "{app=~\".+\"}"
    @State private var timeRange: LogTimeRange = .h1
    @State private var limit: LogLimit = .l100
    @State private var direction: LogDirection = .backward
    @State private var filterText: String = ""

    private var logsService: LogsService { appState.logsService }

    private var filteredEntries: [LogEntry] {
        if filterText.isEmpty {
            return logsService.entries
        }
        let lower = filterText.lowercased()
        return logsService.entries.filter { entry in
            entry.line.lowercased().contains(lower)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Controls section
            controlsSection

            Divider()

            // Results section
            if logsService.isLoading {
                Spacer()
                LoadingView("Querying logs...")
                Spacer()
            } else if let error = logsService.error, logsService.entries.isEmpty {
                Spacer()
                ErrorView(error) {
                    Task { await executeFetch() }
                }
                Spacer()
            } else if logsService.entries.isEmpty {
                Spacer()
                EmptyStateView(
                    icon: "doc.text",
                    title: "No Logs",
                    subtitle: "Execute a query to view logs."
                )
                Spacer()
            } else {
                // Result count
                HStack {
                    Text("\(filteredEntries.count) of \(logsService.total) entries")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.horizontal)
                .padding(.vertical, DiraigentTheme.spacingXS)

                // Log entries
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        ForEach(filteredEntries) { entry in
                            LogEntryRow(entry: entry)
                        }
                    }
                    .padding(.horizontal, DiraigentTheme.spacingSM)
                }
            }
        }
        .navigationTitle("Logs")
    }

    @ViewBuilder
    private var controlsSection: some View {
        VStack(spacing: DiraigentTheme.spacingSM) {
            // Query text field
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                TextField("LogQL query", text: $query)
                    .font(.system(.body, design: .monospaced))
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
            .padding(DiraigentTheme.spacingSM)
            .background(Color(.systemGray6))
            .clipShape(RoundedRectangle(cornerRadius: 8))

            // Time range picker
            HStack {
                Text("Range")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(width: 50, alignment: .leading)
                Picker("Time Range", selection: $timeRange) {
                    ForEach(LogTimeRange.allCases, id: \.self) { range in
                        Text(range.rawValue).tag(range)
                    }
                }
                .pickerStyle(.segmented)
            }

            // Limit and direction row
            HStack(spacing: DiraigentTheme.spacingMD) {
                // Limit picker
                HStack {
                    Text("Limit")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Picker("Limit", selection: $limit) {
                        ForEach(LogLimit.allCases, id: \.self) { l in
                            Text(l.label).tag(l)
                        }
                    }
                    .pickerStyle(.menu)
                }

                Spacer()

                // Direction toggle
                HStack {
                    Text("Direction")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Picker("Direction", selection: $direction) {
                        ForEach(LogDirection.allCases, id: \.self) { d in
                            Label(d.label, systemImage: d.icon).tag(d)
                        }
                    }
                    .pickerStyle(.menu)
                }
            }

            // Filter text field
            HStack {
                Image(systemName: "line.3.horizontal.decrease")
                    .foregroundStyle(.secondary)
                TextField("Filter results...", text: $filterText)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                if !filterText.isEmpty {
                    Button {
                        filterText = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .padding(DiraigentTheme.spacingSM)
            .background(Color(.systemGray6))
            .clipShape(RoundedRectangle(cornerRadius: 8))

            // Fetch button
            Button {
                Task { await executeFetch() }
            } label: {
                HStack {
                    if logsService.isLoading {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Image(systemName: "play.fill")
                    }
                    Text("Fetch Logs")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(logsService.isLoading || query.trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .padding()
    }

    private func executeFetch() async {
        let now = Date()
        let start = now.addingTimeInterval(-timeRange.seconds)

        let isoFormatter = ISO8601DateFormatter()
        isoFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        let startStr = isoFormatter.string(from: start)
        let endStr = isoFormatter.string(from: now)

        await logsService.queryLogs(
            query: query,
            start: startStr,
            end: endStr,
            limit: limit.rawValue,
            direction: direction.rawValue
        )
    }
}

/// A single log entry row with color-coded level.
struct LogEntryRow: View {
    let entry: LogEntry

    private var levelColor: Color {
        switch entry.detectedLevel {
        case .error: .red
        case .warning: .yellow
        case .info: .blue
        case .debug: .gray
        }
    }

    var body: some View {
        HStack(alignment: .top, spacing: DiraigentTheme.spacingSM) {
            // Timestamp
            Text(entry.formattedTimestamp)
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 85, alignment: .leading)

            // Level indicator
            Circle()
                .fill(levelColor)
                .frame(width: 6, height: 6)
                .padding(.top, 4)

            // Log line
            Text(entry.line)
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(levelColor == .gray ? .secondary : .primary)
                .lineLimit(nil)
                .textSelection(.enabled)
        }
        .padding(.vertical, 2)
    }
}
