import SwiftUI

/// Typed navigation wrapper for task IDs.
struct TaskID: Hashable {
    let id: UUID
}

/// List of tasks with state filtering and pull-to-refresh.
struct TaskListView: View {
    @Environment(AppState.self) private var appState

    @State private var stateFilter: String = "all"
    @State private var showCreateTask = false

    private static let stateFilters = ["all", "backlog", "ready", "working", "done", "cancelled"]

    private var tasksService: TasksService { appState.tasksService }

    private var filteredTasks: [DgTask] {
        tasksService.tasks.filter { task in
            guard stateFilter != "all" else { return true }
            if stateFilter == "working" {
                // "working" matches any non-lifecycle state (implement, review, dream, etc.)
                let lifecycleStates = ["backlog", "ready", "done", "cancelled", "human_review"]
                return !lifecycleStates.contains(task.state.lowercased())
            }
            return task.state.lowercased() == stateFilter
        }
    }

    var body: some View {
        Group {
            if tasksService.isLoading && tasksService.tasks.isEmpty {
                LoadingView("Loading tasks...")
            } else if let error = tasksService.error, tasksService.tasks.isEmpty {
                ErrorView(error) {
                    Task { await loadTasks() }
                }
            } else if tasksService.tasks.isEmpty {
                EmptyStateView(
                    icon: "checklist",
                    title: "No Tasks",
                    subtitle: "No tasks have been created yet.",
                    actionTitle: "Create Task"
                ) {
                    showCreateTask = true
                }
            } else {
                VStack(spacing: 0) {
                    // State filter chips
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: DiraigentTheme.spacingSM) {
                            ForEach(Self.stateFilters, id: \.self) { state in
                                FilterChip(
                                    title: state == "all" ? "All" : state.capitalized,
                                    isSelected: stateFilter == state
                                ) {
                                    stateFilter = state
                                }
                            }
                        }
                        .padding(.horizontal)
                    }
                    .padding(.vertical, DiraigentTheme.spacingSM)

                    List(filteredTasks) { task in
                        NavigationLink(value: TaskID(id: task.id)) {
                            TaskRowView(task: task)
                        }
                    }
                }
                .refreshable {
                    await loadTasks()
                }
            }
        }
        .navigationTitle("Tasks")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showCreateTask = true
                } label: {
                    Image(systemName: "plus")
                }
            }
            ToolbarItem(placement: .topBarLeading) {
                ProjectSelectorButton()
            }
        }
        .navigationDestination(for: TaskID.self) { taskId in
            if let task = tasksService.tasks.first(where: { $0.id == taskId.id }) {
                TaskDetailView(task: task)
            }
        }
        .sheet(isPresented: $showCreateTask) {
            CreateTaskView()
        }
        .task {
            await loadTasks()
        }
    }

    private func loadTasks() async {
        guard let projectId = appState.selectedProjectId else { return }
        await tasksService.fetchTasks(projectId: projectId)
    }
}

/// Row for a single task in the list.
struct TaskRowView: View {
    let task: DgTask

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                HStack(spacing: DiraigentTheme.spacingSM) {
                    if let number = task.number {
                        Text("#\(number)")
                            .font(.caption.weight(.bold).monospacedDigit())
                            .foregroundStyle(.secondary)
                    }

                    Text(task.title)
                        .font(DiraigentTheme.headlineFont)
                        .lineLimit(2)
                }

                HStack(spacing: DiraigentTheme.spacingSM) {
                    TaskStateBadge(state: task.state)

                    if let kind = task.kind {
                        TaskKindBadge(kind: kind)
                    }
                }
            }

            Spacer()

            if task.urgent == true {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                    .font(.caption)
            }
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}

/// Colored badge for task states.
struct TaskStateBadge: View {
    let state: String

    private var color: Color {
        DiraigentTheme.taskStateColor(state)
    }

    private var icon: String {
        switch state.lowercased() {
        case "done": "checkmark.circle.fill"
        case "cancelled": "xmark.circle.fill"
        case "ready": "circle"
        case "backlog": "circle.dashed"
        case "human_review": "person.circle"
        default: "gearshape" // working/implement/review etc
        }
    }

    var body: some View {
        Label(state, systemImage: icon)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}

/// Colored badge for task kinds.
struct TaskKindBadge: View {
    let kind: String

    private var color: Color {
        switch kind.lowercased() {
        case "feature": .blue
        case "bug": .red
        case "refactor": .purple
        case "docs": .teal
        case "test": .green
        default: .secondary
        }
    }

    private var icon: String {
        switch kind.lowercased() {
        case "feature": "star.fill"
        case "bug": "ladybug.fill"
        case "refactor": "arrow.triangle.2.circlepath"
        case "docs": "doc.text"
        case "test": "checkmark.shield"
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
