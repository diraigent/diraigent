import SwiftUI

/// Detailed view of a single task with tabs for comments, updates, and dependencies.
struct TaskDetailView: View {
    @Environment(AppState.self) private var appState
    let taskId: UUID

    @State private var tasksService: TasksService?
    @State private var task: DgTask?
    @State private var comments: [TaskComment] = []
    @State private var updates: [TaskUpdate] = []
    @State private var dependencies: TaskDependencies = TaskDependencies()
    @State private var subtasks: [DgTask] = []
    @State private var selectedTab = 0
    @State private var newComment = ""
    @State private var showTransitionSheet = false
    @State private var isLoading = true

    var body: some View {
        Group {
            if isLoading && task == nil {
                ProgressView("Loading\u{2026}")
            } else if let task {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        headerSection(task)
                        specSection(task)
                        filesSection(task)
                        acceptanceCriteriaSection(task)
                        agentSection(task)
                        costSection(task)
                        timestampsSection(task)
                        subtasksSection()
                        tabsSection()
                    }
                    .padding()
                }
            } else {
                ContentUnavailableView(
                    "Task Not Found",
                    systemImage: "exclamationmark.triangle",
                    description: Text("Could not load this task.")
                )
            }
        }
        .navigationTitle(task?.title ?? "Task")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let task {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        ForEach(availableTransitions(for: task.state), id: \.self) { target in
                            Button(transitionLabel(target)) {
                                Task { await performTransition(target) }
                            }
                        }
                    } label: {
                        Label("Actions", systemImage: "arrow.triangle.2.circlepath")
                    }
                    .disabled(availableTransitions(for: task.state).isEmpty)
                }
            }
        }
        .task {
            await loadTask()
        }
    }

    // MARK: - Header

    @ViewBuilder
    private func headerSection(_ task: DgTask) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                if task.urgent == true {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                }
                Text(task.title)
                    .font(.title2.bold())
            }

            HStack(spacing: 8) {
                StateBadge(state: task.state)
                if let kind = task.kind {
                    Text(kind)
                        .font(.caption)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(Color.secondary.opacity(0.12))
                        .clipShape(Capsule())
                }
                if let priority = task.priority {
                    Label("P\(priority)", systemImage: "flag.fill")
                        .font(.caption)
                        .foregroundStyle(priorityColor(priority))
                }
                if let number = task.number {
                    Text("#\(number)")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }
        }
    }

    // MARK: - Spec

    @ViewBuilder
    private func specSection(_ task: DgTask) -> some View {
        if let spec = task.context?.spec, !spec.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                sectionHeader("Spec")
                Text(spec)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Files

    @ViewBuilder
    private func filesSection(_ task: DgTask) -> some View {
        if let files = task.context?.files, !files.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                sectionHeader("Files")
                ForEach(files, id: \.self) { file in
                    HStack(spacing: 4) {
                        Image(systemName: "doc.text")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text(file)
                            .font(.caption.monospaced())
                            .lineLimit(1)
                    }
                }
            }
        }
    }

    // MARK: - Acceptance Criteria

    @ViewBuilder
    private func acceptanceCriteriaSection(_ task: DgTask) -> some View {
        if let criteria = task.context?.acceptanceCriteria, !criteria.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                sectionHeader("Acceptance Criteria")
                ForEach(Array(criteria.enumerated()), id: \.offset) { _, criterion in
                    HStack(alignment: .top, spacing: 6) {
                        Image(systemName: task.state == "done" ? "checkmark.circle.fill" : "circle")
                            .font(.caption)
                            .foregroundStyle(task.state == "done" ? .green : .secondary)
                        Text(criterion)
                            .font(.callout)
                    }
                }
            }
        }
    }

    // MARK: - Agent

    @ViewBuilder
    private func agentSection(_ task: DgTask) -> some View {
        if let agentId = task.assignedAgentId {
            VStack(alignment: .leading, spacing: 4) {
                sectionHeader("Agent")
                HStack {
                    Image(systemName: "person.circle.fill")
                        .foregroundStyle(.orange)
                    Text(agentId.uuidString.prefix(8) + "\u{2026}")
                        .font(.callout.monospaced())
                }
            }
        }
    }

    // MARK: - Cost

    @ViewBuilder
    private func costSection(_ task: DgTask) -> some View {
        let hasCost = (task.costUsd ?? 0) > 0 || (task.inputTokens ?? 0) > 0
        if hasCost {
            VStack(alignment: .leading, spacing: 4) {
                sectionHeader("Cost")
                HStack(spacing: 16) {
                    if let cost = task.costUsd, cost > 0 {
                        Label(String(format: "$%.4f", cost), systemImage: "dollarsign.circle")
                            .font(.callout)
                    }
                    if let input = task.inputTokens, input > 0 {
                        Label("\(input) in", systemImage: "arrow.down.circle")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    if let output = task.outputTokens, output > 0 {
                        Label("\(output) out", systemImage: "arrow.up.circle")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    // MARK: - Timestamps

    @ViewBuilder
    private func timestampsSection(_ task: DgTask) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            sectionHeader("Timestamps")
            if let created = task.createdAt {
                timestampRow("Created", value: created)
            }
            if let claimed = task.claimedAt {
                timestampRow("Claimed", value: claimed)
            }
            if let completed = task.completedAt {
                timestampRow("Completed", value: completed)
            }
        }
    }

    private func timestampRow(_ label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 80, alignment: .leading)
            Text(formatTimestamp(value))
                .font(.caption.monospaced())
        }
    }

    // MARK: - Subtasks

    @ViewBuilder
    private func subtasksSection() -> some View {
        if !subtasks.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                sectionHeader("Subtasks (\(subtasks.count))")
                ForEach(subtasks) { subtask in
                    NavigationLink(value: subtask.id) {
                        HStack {
                            StateBadge(state: subtask.state)
                            Text(subtask.title)
                                .font(.callout)
                                .lineLimit(1)
                        }
                    }
                }
            }
        }
    }

    // MARK: - Tabs (Comments / Updates / Dependencies)

    @ViewBuilder
    private func tabsSection() -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Picker("Section", selection: $selectedTab) {
                Text("Comments").tag(0)
                Text("Updates").tag(1)
                Text("Dependencies").tag(2)
            }
            .pickerStyle(.segmented)

            switch selectedTab {
            case 0: commentsTab()
            case 1: updatesTab()
            case 2: dependenciesTab()
            default: EmptyView()
            }
        }
    }

    @ViewBuilder
    private func commentsTab() -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if comments.isEmpty {
                Text("No comments yet.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(comments) { comment in
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(comment.authorName ?? "Unknown")
                                .font(.caption.bold())
                            Spacer()
                            if let date = comment.createdAt {
                                Text(formatTimestamp(date))
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        Text(comment.content)
                            .font(.callout)
                    }
                    .padding(8)
                    .background(Color(.secondarySystemBackground))
                    .cornerRadius(8)
                }
            }

            // Add comment
            HStack {
                TextField("Add a comment\u{2026}", text: $newComment)
                    .textFieldStyle(.roundedBorder)
                Button {
                    Task { await submitComment() }
                } label: {
                    Image(systemName: "paperplane.fill")
                }
                .disabled(newComment.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    @ViewBuilder
    private func updatesTab() -> some View {
        if updates.isEmpty {
            Text("No updates yet.")
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            ForEach(updates) { update in
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(update.kind)
                            .font(.caption.bold())
                            .textCase(.uppercase)
                            .foregroundStyle(updateKindColor(update.kind))
                        Spacer()
                        if let date = update.createdAt {
                            Text(formatTimestamp(date))
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                    }
                    Text(update.content)
                        .font(.callout)
                }
                .padding(8)
                .background(Color(.secondarySystemBackground))
                .cornerRadius(8)
            }
        }
    }

    @ViewBuilder
    private func dependenciesTab() -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if !dependencies.dependsOn.isEmpty {
                Text("Depends On")
                    .font(.caption.bold())
                    .foregroundStyle(.secondary)
                ForEach(dependencies.dependsOn) { dep in
                    NavigationLink(value: dep.dependsOn) {
                        HStack {
                            StateBadge(state: dep.state)
                            Text(dep.title)
                                .font(.callout)
                                .lineLimit(1)
                        }
                    }
                }
            }

            if !dependencies.blocks.isEmpty {
                Text("Blocks")
                    .font(.caption.bold())
                    .foregroundStyle(.secondary)
                ForEach(dependencies.blocks) { dep in
                    NavigationLink(value: dep.taskId) {
                        HStack {
                            StateBadge(state: dep.state)
                            Text(dep.title)
                                .font(.callout)
                                .lineLimit(1)
                        }
                    }
                }
            }

            if dependencies.dependsOn.isEmpty && dependencies.blocks.isEmpty {
                Text("No dependencies.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Transitions

    private func availableTransitions(for state: String) -> [String] {
        switch state {
        case "backlog":
            return ["ready", "cancelled"]
        case "ready":
            return ["backlog", "cancelled"]
        case "done":
            return ["human_review"]
        case "human_review":
            return ["done", "ready", "backlog"]
        case "cancelled":
            return ["backlog"]
        default:
            // Working/step states
            return ["done", "ready", "cancelled"]
        }
    }

    private func transitionLabel(_ state: String) -> String {
        switch state {
        case "ready": return "Move to Ready"
        case "backlog": return "Move to Backlog"
        case "done": return "Mark Done"
        case "cancelled": return "Cancel"
        case "human_review": return "Send to Review"
        default: return "Transition to \(state)"
        }
    }

    private func performTransition(_ target: String) async {
        guard let projectId = appState.selectedProjectId, let service = tasksService else { return }
        if let updated = await service.transitionTask(projectId: projectId, taskId: taskId, state: target) {
            task = updated
        }
    }

    // MARK: - Actions

    private func submitComment() async {
        let text = newComment.trimmingCharacters(in: .whitespaces)
        guard !text.isEmpty,
              let projectId = appState.selectedProjectId,
              let service = tasksService else { return }
        if let comment = await service.createComment(projectId: projectId, taskId: taskId, body: text) {
            comments.append(comment)
            newComment = ""
        }
    }

    // MARK: - Data Loading

    private func loadTask() async {
        guard let projectId = appState.selectedProjectId else { return }
        let service = TasksService(apiClient: appState.apiClient)
        tasksService = service
        isLoading = true

        async let taskResult = service.fetchTask(projectId: projectId, taskId: taskId)
        async let commentsResult = service.fetchComments(projectId: projectId, taskId: taskId)
        async let updatesResult = service.fetchUpdates(projectId: projectId, taskId: taskId)
        async let depsResult = service.fetchDependencies(projectId: projectId, taskId: taskId)
        async let subtasksResult = service.fetchSubtasks(projectId: projectId, taskId: taskId)

        task = await taskResult
        comments = await commentsResult
        updates = await updatesResult
        dependencies = await depsResult
        subtasks = await subtasksResult
        isLoading = false
    }

    // MARK: - Helpers

    private func sectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.headline)
            .padding(.top, 4)
    }

    private func priorityColor(_ priority: Int) -> Color {
        switch priority {
        case 8...10: return .red
        case 6...7: return .orange
        case 4...5: return .yellow
        default: return .gray
        }
    }

    private func updateKindColor(_ kind: String) -> Color {
        switch kind {
        case "progress": return .blue
        case "artifact": return .green
        case "blocker": return .red
        default: return .secondary
        }
    }

    private func formatTimestamp(_ iso: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: iso) else {
            // Try without fractional seconds
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: iso) else { return iso }
            return RelativeDateTimeFormatter().localizedString(for: date, relativeTo: Date())
        }
        return RelativeDateTimeFormatter().localizedString(for: date, relativeTo: Date())
    }
}
