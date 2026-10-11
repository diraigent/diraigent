import SwiftUI

/// Project execution logs are available to viewers; deployment-wide Loki logs are not.
struct ProjectLogsView: View {
    @Environment(AppState.self) private var appState
    @State private var logs: [ExecutionLog] = []
    @State private var error: String?
    @State private var isLoading = false
    @State private var offset = 0
    @State private var hasMore = false
    @State private var generation = 0

    var body: some View {
        List {
            ForEach(logs) { log in
                NavigationLink(log.stepName ?? "Execution log") {
                    ExecutionLogView(id: log.id)
                }
            }
            if let error { Text(error).foregroundStyle(.secondary) }
            if isLoading { ProgressView() }
            if hasMore || error != nil {
                Button(error == nil ? "Load More" : "Retry") { Task { await load() } }
                    .disabled(isLoading)
            }
            if logs.isEmpty && !isLoading && error == nil {
                Text("No execution logs yet.").foregroundStyle(.secondary)
            }
        }
        .navigationTitle("Logs")
        .task(id: appState.selectedProjectId) {
            generation += 1
            isLoading = false
            logs = []
            offset = 0
            await load()
        }
        .refreshable {
            generation += 1
            isLoading = false
            logs = []
            offset = 0
            await load()
        }
    }

    private func load() async {
        guard let project = appState.selectedProjectId, !isLoading else { return }
        isLoading = true
        let current = generation
        error = nil
        defer { if current == generation { isLoading = false } }
        do {
            let page: PaginatedResponse<ExecutionLog> = try await appState.apiClient.get(
                "/\(project)/task-logs", query: ["offset": String(offset), "limit": "50"])
            guard current == generation, !Task.isCancelled else { return }
            logs += page.data
            offset += page.data.count
            hasMore = page.hasMore
        } catch { if current == generation && !Task.isCancelled { self.error = error.localizedDescription } }
    }
}

private struct ExecutionLog: Codable, Identifiable, Sendable {
    let id: UUID
    let stepName: String?
    let content: String?
}

private struct ExecutionLogView: View {
    @Environment(AppState.self) private var appState
    let id: UUID
    @State private var content: String?
    @State private var error: String?

    var body: some View {
        ScrollView {
            if let content {
                Text(content).font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding()
            } else if let error { Text(error).padding() }
            else { ProgressView() }
        }
        .navigationTitle("Execution Log")
        .task {
            do {
                let log: ExecutionLog = try await appState.apiClient.get("/task-logs/\(id)")
                content = log.content ?? "No content"
            } catch { self.error = error.localizedDescription }
        }
    }
}
