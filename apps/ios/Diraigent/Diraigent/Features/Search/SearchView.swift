import SwiftUI

/// Search view with debounced input and results grouped by entity type.
struct SearchView: View {
    @Environment(AppState.self) private var appState

    @State private var query: String = ""

    private var searchService: SearchService { appState.searchService }

    /// Group results by entity type.
    private var groupedResults: [(String, [SearchResult])] {
        let grouped = Dictionary(grouping: searchService.results) { $0.entityType }
        let order = ["task", "decision", "observation", "report", "knowledge"]
        return order.compactMap { type in
            guard let items = grouped[type], !items.isEmpty else { return nil }
            return (type, items)
        } + grouped.filter { !order.contains($0.key) }.map { ($0.key, $0.value) }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Results
            if searchService.isLoading {
                LoadingView("Searching...")
            } else if query.isEmpty {
                EmptyStateView(
                    icon: "magnifyingglass",
                    title: "Search",
                    subtitle: "Search across tasks, decisions, observations, and more."
                )
            } else if searchService.results.isEmpty && !query.isEmpty {
                EmptyStateView(
                    icon: "magnifyingglass",
                    title: "No Results",
                    subtitle: "No results found for \"\(query)\"."
                )
            } else {
                List {
                    ForEach(groupedResults, id: \.0) { entityType, results in
                        Section {
                            ForEach(results) { result in
                                SearchResultRowView(result: result)
                            }
                        } header: {
                            HStack {
                                Image(systemName: entityTypeIcon(entityType))
                                Text(entityTypeDisplayName(entityType))
                            }
                        }
                    }
                }
            }
        }
        .searchable(text: $query, prompt: "Search project...")
        .onChange(of: query) { _, newValue in
            guard let projectId = appState.selectedProjectId else { return }
            searchService.search(projectId: projectId, query: newValue)
        }
        .onDisappear {
            searchService.clearResults()
        }
        .navigationTitle("Search")
    }

    // MARK: - Helpers

    private func entityTypeIcon(_ type: String) -> String {
        switch type.lowercased() {
        case "task": "checklist"
        case "decision": "scale.3d"
        case "observation": "eye"
        case "report": "doc.text"
        case "knowledge": "book"
        case "work": "hammer"
        default: "doc"
        }
    }

    private func entityTypeDisplayName(_ type: String) -> String {
        switch type.lowercased() {
        case "task": "Tasks"
        case "decision": "Decisions"
        case "observation": "Observations"
        case "report": "Reports"
        case "knowledge": "Knowledge"
        case "work": "Work Items"
        default: type.capitalized
        }
    }
}

/// Row for a single search result.
struct SearchResultRowView: View {
    let result: SearchResult

    private var entityIcon: String {
        switch result.entityType.lowercased() {
        case "task": "checklist"
        case "decision": "scale.3d"
        case "observation": "eye"
        case "report": "doc.text"
        case "knowledge": "book"
        case "work": "hammer"
        default: "doc"
        }
    }

    private var entityColor: Color {
        switch result.entityType.lowercased() {
        case "task": .blue
        case "decision": .purple
        case "observation": .orange
        case "report": .green
        case "knowledge": .indigo
        case "work": .brown
        default: .secondary
        }
    }

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            Image(systemName: entityIcon)
                .foregroundStyle(entityColor)
                .frame(width: 24)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(result.title)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(1)

                if let snippet = result.snippet, !snippet.isEmpty {
                    Text(snippet)
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
            }

            Spacer()

            if let relevance = result.relevance {
                Text(String(format: "%.0f%%", relevance * 100))
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}
