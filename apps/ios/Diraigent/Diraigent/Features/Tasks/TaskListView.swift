import SwiftUI

/// Task list with filtering, search, and navigation to detail.
struct TaskListView: View {
    @Environment(AppState.self) private var appState
    @State private var tasksService: TasksService?
    @State private var selectedFilter = "all"
    @State private var searchText = ""
    @State private var showCreateSheet = false

    private let stateFilters = ["all", "ready", "working", "done", "backlog"]

    var body: some View {
        VStack(spacing: 0) {
            // State filter
            Picker("Filter", selection: $selectedFilter) {
                ForEach(stateFilters, id: \.self) { filter in
                    Text(filter.capitalized).tag(filter)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)
            .padding(.vertical, 8)

            // Task list
            Group {
                if let service = tasksService {
                    if service.isLoading && service.tasks.isEmpty {
                        ProgressView("Loading tasks\u{2026}")
                            .frame(maxHeight: .infinity)
                    } else if filteredTasks.isEmpty {
                        ContentUnavailableView(
                            "No Tasks",
                            systemImage: "checklist",
                            description: Text(
                                selectedFilter == "all"
                                    ? "No tasks yet. Tap + to create one."
                                    : "No \(selectedFilter) tasks."
                            )
                        )
                    } else {
                        List {
                            ForEach(filteredTasks) { task in
                                NavigationLink(value: task.id) {
                                    TaskRowView(task: task)
                                }
                            }
                        }
                        .listStyle(.plain)
                        .refreshable {
                            await refresh()
                        }
                    }

                    if let error = service.error {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                            .padding(.horizontal)
                    }
                } else {
                    ProgressView()
                        .frame(maxHeight: .infinity)
                }
            }
        }
        .searchable(text: $searchText, prompt: "Search tasks")
        .navigationTitle("Tasks")
        .navigationDestination(for: UUID.self) { taskId in
            TaskDetailView(taskId: taskId)
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showCreateSheet = true
                } label: {
                    Image(systemName: "plus")
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            if let projectId = appState.selectedProjectId, let service = tasksService {
                CreateTaskSheet(
                    projectId: projectId,
                    tasksService: service,
                    isPresented: $showCreateSheet
                )
            }
        }
        .task {
            await setup()
        }
        .onChange(of: selectedFilter) {
            Task { await refresh() }
        }
    }

    // MARK: - Filtering

    private var filteredTasks: [DgTask] {
        guard let service = tasksService else { return [] }
        var result = service.tasks

        // Apply state filter for "working" which matches non-lifecycle states
        if selectedFilter == "working" {
            let lifecycle = Set(["backlog", "ready", "done", "cancelled", "human_review"])
            result = result.filter { !lifecycle.contains($0.state) }
        }

        // Apply search
        if !searchText.isEmpty {
            result = result.filter {
                $0.title.localizedCaseInsensitiveContains(searchText)
            }
        }

        return result
    }

    // MARK: - Data

    private func setup() async {
        if tasksService == nil {
            tasksService = TasksService(apiClient: appState.apiClient)
        }
        await refresh()
    }

    private func refresh() async {
        guard let projectId = appState.selectedProjectId else { return }
        let filterState = selectedFilter == "working" ? nil : selectedFilter
        await tasksService?.fetchTasks(projectId: projectId, state: filterState)
    }
}
