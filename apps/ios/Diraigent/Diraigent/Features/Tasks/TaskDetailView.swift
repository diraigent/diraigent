import SwiftUI

/// Full detail view for a single task.
struct TaskDetailView: View {
    @Environment(AppState.self) private var appState

    let task: DgTask

    @State private var updates: [TaskUpdate] = []
    @State private var comments: [TaskComment] = []
    @State private var dependencies: TaskDependencies?
    @State private var isLoadingDetails = false
    @State private var newComment = ""
    @State private var isSubmittingComment = false

    var body: some View {
        List {
            // MARK: - Header
            Section {
                headerSection
            }

            // MARK: - Spec
            if let spec = taskSpec, !spec.isEmpty {
                Section("Spec") {
                    Text(spec)
                        .font(DiraigentTheme.bodyFont)
                        .textSelection(.enabled)
                }
            }

            // MARK: - Acceptance Criteria
            if let criteria = acceptanceCriteria, !criteria.isEmpty {
                Section("Acceptance Criteria") {
                    ForEach(criteria, id: \.self) { criterion in
                        HStack(alignment: .top, spacing: DiraigentTheme.spacingSM) {
                            Image(systemName: "circle")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .padding(.top, 2)
                            Text(criterion)
                                .font(DiraigentTheme.bodyFont)
                        }
                    }
                }
            }

            // MARK: - Dependencies
            if let deps = dependencies {
                dependenciesSection(deps)
            }

            // MARK: - Updates
            updatesSection

            // MARK: - Comments
            commentsSection

            // MARK: - Metadata
            metadataSection
        }
        .navigationTitle(task.title)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await loadDetails()
        }
    }

    // MARK: - Header Section

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingMD) {
            HStack {
                if let number = task.number {
                    Text("#\(number)")
                        .font(.title3.weight(.bold).monospacedDigit())
                        .foregroundStyle(.secondary)
                }

                Text(task.title)
                    .font(DiraigentTheme.titleFont)
            }

            HStack(spacing: DiraigentTheme.spacingSM) {
                TaskStateBadge(state: task.state)

                if let kind = task.kind {
                    TaskKindBadge(kind: kind)
                }

                if task.urgent == true {
                    Label("Urgent", systemImage: "exclamationmark.triangle.fill")
                        .font(.caption2.weight(.semibold))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.orange.opacity(0.15))
                        .foregroundStyle(.orange)
                        .clipShape(Capsule())
                }

                if task.flagged == true {
                    Label("Flagged", systemImage: "flag.fill")
                        .font(.caption2.weight(.semibold))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.red.opacity(0.15))
                        .foregroundStyle(.red)
                        .clipShape(Capsule())
                }
            }

            // Cost & tokens
            if let cost = task.costUsd, cost > 0 {
                HStack(spacing: DiraigentTheme.spacingMD) {
                    Label(String(format: "$%.4f", cost), systemImage: "dollarsign.circle")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    if let input = task.inputTokens, let output = task.outputTokens {
                        Label("\(formatTokens(input))/\(formatTokens(output))", systemImage: "arrow.left.arrow.right")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    // MARK: - Dependencies

    @ViewBuilder
    private func dependenciesSection(_ deps: TaskDependencies) -> some View {
        if let dependsOn = deps.dependsOn, !dependsOn.isEmpty {
            Section("Depends On") {
                ForEach(dependsOn) { dep in
                    dependencyRow(dep)
                }
            }
        }

        if let dependedOnBy = deps.dependedOnBy, !dependedOnBy.isEmpty {
            Section("Depended On By") {
                ForEach(dependedOnBy) { dep in
                    dependencyRow(dep)
                }
            }
        }
    }

    private func dependencyRow(_ dep: DgTask) -> some View {
        HStack(spacing: DiraigentTheme.spacingSM) {
            Image(systemName: taskStateIcon(dep.state))
                .foregroundStyle(DiraigentTheme.taskStateColor(dep.state))
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: DiraigentTheme.spacingXS) {
                    if let number = dep.number {
                        Text("#\(number)")
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                    Text(dep.title)
                        .font(DiraigentTheme.bodyFont)
                        .lineLimit(1)
                }

                Text(dep.state)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Updates

    @ViewBuilder
    private var updatesSection: some View {
        Section("Updates") {
            if isLoadingDetails && updates.isEmpty {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            } else if updates.isEmpty {
                Text("No updates yet.")
                    .font(DiraigentTheme.captionFont)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(updates) { update in
                    VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                        HStack {
                            UpdateKindBadge(kind: update.kind)
                            Spacer()
                            if let date = update.createdAt {
                                Text(formatDate(date))
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }

                        Text(update.content)
                            .font(DiraigentTheme.bodyFont)
                            .textSelection(.enabled)
                    }
                    .padding(.vertical, DiraigentTheme.spacingXS)
                }
            }
        }
    }

    // MARK: - Comments

    @ViewBuilder
    private var commentsSection: some View {
        Section("Comments") {
            if isLoadingDetails && comments.isEmpty {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            } else if comments.isEmpty {
                Text("No comments yet.")
                    .font(DiraigentTheme.captionFont)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(comments) { comment in
                    VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                        HStack {
                            Text(comment.authorName ?? "Unknown")
                                .font(.caption.weight(.semibold))
                            Spacer()
                            if let date = comment.createdAt {
                                Text(formatDate(date))
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }

                        Text(comment.content)
                            .font(DiraigentTheme.bodyFont)
                            .textSelection(.enabled)
                    }
                    .padding(.vertical, DiraigentTheme.spacingXS)
                }
            }

            // Add comment
            HStack {
                TextField("Add a comment...", text: $newComment, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...4)

                Button {
                    Task { await submitComment() }
                } label: {
                    Image(systemName: "paperplane.fill")
                }
                .disabled(newComment.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSubmittingComment)
            }
        }
    }

    // MARK: - Metadata

    private var metadataSection: some View {
        Section("Details") {
            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ], spacing: DiraigentTheme.spacingSM) {
                if let created = task.createdAt {
                    MetadataItem(label: "Created", value: formatDate(created))
                }
                if let updated = task.updatedAt {
                    MetadataItem(label: "Updated", value: formatDate(updated))
                }
                if let claimed = task.claimedAt {
                    MetadataItem(label: "Claimed", value: formatDate(claimed))
                }
                if let completed = task.completedAt {
                    MetadataItem(label: "Completed", value: formatDate(completed))
                }
                if let step = task.playbookStep {
                    MetadataItem(label: "Playbook Step", value: "\(step)")
                }
                if task.assignedAgentId != nil {
                    MetadataItem(label: "Assigned Agent", value: task.assignedAgentId?.uuidString.prefix(8).description ?? "—")
                }
            }
        }
    }

    // MARK: - Actions

    private func loadDetails() async {
        guard let projectId = appState.selectedProjectId else { return }
        isLoadingDetails = true

        async let updatesResult = appState.tasksService.getTaskUpdates(projectId: projectId, taskId: task.id)
        async let commentsResult = appState.tasksService.getTaskComments(projectId: projectId, taskId: task.id)
        async let depsResult = appState.tasksService.getTaskDependencies(projectId: projectId, taskId: task.id)

        updates = await updatesResult
        comments = await commentsResult
        dependencies = await depsResult

        isLoadingDetails = false
    }

    private func submitComment() async {
        guard let projectId = appState.selectedProjectId else { return }
        let text = newComment.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        isSubmittingComment = true
        if let comment = await appState.tasksService.addTaskComment(projectId: projectId, taskId: task.id, content: text) {
            comments.append(comment)
            newComment = ""
        }
        isSubmittingComment = false
    }

    // MARK: - Helpers

    private var taskSpec: String? {
        guard let context = task.context,
              let specValue = context["spec"] else { return nil }
        return specValue.stringValue
    }

    private var acceptanceCriteria: [String]? {
        guard let context = task.context,
              let criteriaValue = context["acceptance_criteria"] else { return nil }
        if let array = criteriaValue.value as? [Any] {
            return array.compactMap { item in
                if let str = item as? String { return str }
                return nil
            }
        }
        return nil
    }

    private func taskStateIcon(_ state: String) -> String {
        switch state.lowercased() {
        case "done": "checkmark.circle.fill"
        case "cancelled": "xmark.circle.fill"
        case "ready": "circle"
        case "backlog": "circle.dashed"
        default: "gearshape"
        }
    }

    private func formatTokens(_ count: Int) -> String {
        if count >= 1_000_000 {
            return String(format: "%.1fM", Double(count) / 1_000_000)
        } else if count >= 1_000 {
            return String(format: "%.1fK", Double(count) / 1_000)
        }
        return "\(count)"
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

/// Colored badge for task update kinds.
struct UpdateKindBadge: View {
    let kind: String

    private var color: Color {
        switch kind.lowercased() {
        case "progress": .blue
        case "artifact": .purple
        case "blocker": .red
        case "comment": .green
        default: .secondary
        }
    }

    private var icon: String {
        switch kind.lowercased() {
        case "progress": "arrow.forward.circle"
        case "artifact": "doc.fill"
        case "blocker": "exclamationmark.octagon"
        case "comment": "bubble.left"
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
