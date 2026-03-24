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
                                NavigationLink {
                                    SearchResultDestinationView(result: result)
                                } label: {
                                    SearchResultRowView(result: result)
                                }
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

/// Destination view that fetches and displays the appropriate detail view for a search result.
struct SearchResultDestinationView: View {
    @Environment(AppState.self) private var appState

    let result: SearchResult

    @State private var decision: Decision?
    @State private var observation: DgObservation?
    @State private var work: Work?
    @State private var task: DgTask?
    @State private var isLoading = true
    @State private var error: String?

    var body: some View {
        Group {
            if isLoading {
                LoadingView("Loading...")
            } else if let error {
                ErrorView(error) {
                    Task { await loadEntity() }
                }
            } else {
                destinationContent
            }
        }
        .task {
            await loadEntity()
        }
    }

    @ViewBuilder
    private var destinationContent: some View {
        switch result.entityType.lowercased() {
        case "decision":
            if let decision {
                DecisionDetailView(decision: decision)
            } else {
                fallbackView
            }
        case "observation":
            if let observation {
                ObservationDetailView(observation: observation)
            } else {
                fallbackView
            }
        case "work":
            if let work {
                WorkDetailView(work: work)
            } else {
                fallbackView
            }
        case "task":
            if let task {
                TaskSearchDetailView(task: task)
            } else {
                fallbackView
            }
        default:
            fallbackView
        }
    }

    private var fallbackView: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingLG) {
            Text(result.title)
                .font(DiraigentTheme.titleFont)

            if let snippet = result.snippet, !snippet.isEmpty {
                Text(snippet)
                    .font(DiraigentTheme.bodyFont)
                    .foregroundStyle(.secondary)
            }

            HStack {
                Text(result.entityType.capitalized)
                    .font(DiraigentTheme.captionFont)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.secondary.opacity(0.12))
                    .clipShape(Capsule())
                Spacer()
            }

            Spacer()
        }
        .padding()
        .navigationTitle(result.title)
    }

    private func loadEntity() async {
        guard let projectId = appState.selectedProjectId else {
            error = "No project selected"
            isLoading = false
            return
        }

        isLoading = true
        error = nil

        do {
            switch result.entityType.lowercased() {
            case "decision":
                let d: Decision = try await appState.apiClient.get(
                    Endpoints.decision(projectId, decisionId: result.entityId)
                )
                decision = d
            case "observation":
                let o: DgObservation = try await appState.apiClient.get(
                    Endpoints.observation(projectId, observationId: result.entityId)
                )
                observation = o
            case "work":
                let w: Work = try await appState.apiClient.get(
                    Endpoints.workItem(projectId, workId: result.entityId)
                )
                work = w
            case "task":
                let t: DgTask = try await appState.apiClient.get(
                    Endpoints.task(projectId, taskId: result.entityId)
                )
                task = t
            default:
                break // fallback view used
            }
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }
}

/// Basic detail view for a task found via search.
struct TaskSearchDetailView: View {
    let task: DgTask

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
                    HStack(spacing: DiraigentTheme.spacingSM) {
                        Text(task.state)
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(stateColor.opacity(0.15))
                            .foregroundStyle(stateColor)
                            .clipShape(Capsule())

                        if let kind = task.kind {
                            Text(kind)
                                .font(.caption)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color.secondary.opacity(0.12))
                                .clipShape(Capsule())
                        }

                        if task.urgent == true {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(.red)
                                .font(.caption)
                        }

                        Spacer()

                        if task.urgent == true {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(.orange)
                                .font(.caption)
                        }
                    }
                }
            }

            if let spec = task.context?["spec"]?.stringValue, !spec.isEmpty {
                Section("Spec") {
                    Text(spec)
                        .font(DiraigentTheme.bodyFont)
                }
            }

            if let filesValue = task.context?["files"]?.value as? [Any], !filesValue.isEmpty {
                Section("Files") {
                    ForEach(filesValue.compactMap { $0 as? String }, id: \.self) { file in
                        Text(file)
                            .font(.caption.monospaced())
                    }
                }
            }

            if let criteriaValue = task.context?["acceptance_criteria"]?.value as? [Any], !criteriaValue.isEmpty {
                let criteria = criteriaValue.compactMap { $0 as? String }
                Section("Acceptance Criteria") {
                    ForEach(criteria, id: \.self) { criterion in
                        HStack(alignment: .top, spacing: DiraigentTheme.spacingSM) {
                            Image(systemName: task.state == "done" ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(task.state == "done" ? .green : .secondary)
                                .font(.caption)
                                .padding(.top, 2)
                            Text(criterion)
                                .font(DiraigentTheme.bodyFont)
                        }
                    }
                }
            }

            Section("Details") {
                if let number = task.number {
                    HStack {
                        Text("Number")
                        Spacer()
                        Text("#\(number)")
                            .foregroundStyle(.secondary)
                    }
                }
                if let cost = task.costUsd {
                    HStack {
                        Text("Cost")
                        Spacer()
                        Text(String(format: "$%.2f", cost))
                            .foregroundStyle(.secondary)
                    }
                }
                if let created = task.createdAt {
                    HStack {
                        Text("Created")
                        Spacer()
                        Text(formatDate(created))
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .navigationTitle(task.title)
    }

    private var stateColor: Color {
        switch task.state.lowercased() {
        case "done": .green
        case "cancelled": .secondary
        case "ready": .blue
        case "backlog": .secondary
        default: .orange
        }
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
