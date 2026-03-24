import SwiftUI

/// Detail view for a work item showing description, success criteria, and linked tasks.
struct WorkDetailView: View {
    @Environment(AppState.self) private var appState

    let work: Work

    @State private var linkedTasks: [DgTask] = []
    @State private var progress: WorkProgress?
    @State private var isLoadingTasks = false
    @State private var currentStatus: String?
    @State private var isUpdatingStatus = false
    @State private var showStatusConfirmation = false
    @State private var pendingStatus: String?

    private static let allStatuses = ["active", "achieved", "paused", "abandoned"]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingLG) {
                // Header
                headerSection

                // Status actions
                statusActionsSection

                // Progress indicator
                if let progress {
                    progressSection(progress)
                }

                Divider()

                // Description
                if let description = work.description, !description.isEmpty {
                    descriptionSection(description)
                }

                // Success criteria
                if let criteria = work.successCriteria {
                    successCriteriaSection(criteria.displayText)
                }

                // Linked tasks
                linkedTasksSection

                // Metadata
                metadataSection
            }
            .padding()
        }
        .navigationTitle(work.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                statusMenu
            }
        }
        .alert("Change Status", isPresented: $showStatusConfirmation) {
            Button("Cancel", role: .cancel) {
                pendingStatus = nil
            }
            Button("Confirm") {
                if let status = pendingStatus {
                    Task { await changeStatus(to: status) }
                }
            }
        } message: {
            if let status = pendingStatus {
                Text("Change status to \"\(status)\"?")
            }
        }
        .task {
            currentStatus = work.status
            await loadLinkedData()
        }
    }

    private func loadLinkedData() async {
        guard let projectId = appState.selectedProjectId else { return }
        isLoadingTasks = true
        async let tasksResult = appState.workService.fetchWorkTasks(projectId: projectId, workId: work.id)
        async let progressResult = appState.workService.fetchWorkProgress(projectId: projectId, workId: work.id)
        linkedTasks = await tasksResult
        progress = await progressResult
        isLoadingTasks = false
    }

    // MARK: - Status Change

    private var statusMenu: some View {
        Menu {
            ForEach(Self.allStatuses, id: \.self) { status in
                Button {
                    pendingStatus = status
                    showStatusConfirmation = true
                } label: {
                    Label(status.capitalized, systemImage: statusIcon(status))
                }
                .disabled(status == (currentStatus ?? work.status))
            }
        } label: {
            if isUpdatingStatus {
                ProgressView()
            } else {
                Label("Status", systemImage: "arrow.triangle.2.circlepath")
            }
        }
        .disabled(isUpdatingStatus)
    }

    private var statusActionsSection: some View {
        VStack(spacing: DiraigentTheme.spacingSM) {
            HStack(spacing: DiraigentTheme.spacingSM) {
                ForEach(Self.allStatuses, id: \.self) { status in
                    let isCurrent = status == (currentStatus ?? work.status)
                    Button {
                        guard !isCurrent else { return }
                        pendingStatus = status
                        showStatusConfirmation = true
                    } label: {
                        Label(status.capitalized, systemImage: statusIcon(status))
                            .font(.caption.weight(.semibold))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, DiraigentTheme.spacingSM)
                            .background(isCurrent ? statusColor(status).opacity(0.2) : Color.secondary.opacity(0.08))
                            .foregroundStyle(isCurrent ? statusColor(status) : .secondary)
                            .clipShape(RoundedRectangle(cornerRadius: 8))
                    }
                    .buttonStyle(.plain)
                    .disabled(isUpdatingStatus || isCurrent)
                }
            }
        }
    }

    private func changeStatus(to newStatus: String) async {
        guard let projectId = appState.selectedProjectId else { return }
        isUpdatingStatus = true
        let update = UpdateWorkRequest(
            title: nil,
            workType: nil,
            status: newStatus,
            priority: nil,
            description: nil
        )
        let result = await appState.workService.updateWork(
            projectId: projectId,
            workId: work.id,
            update: update
        )
        if result != nil {
            currentStatus = newStatus
        }
        isUpdatingStatus = false
        pendingStatus = nil
    }

    private func statusIcon(_ status: String) -> String {
        switch status.lowercased() {
        case "active": return "play.circle.fill"
        case "achieved": return "checkmark.circle.fill"
        case "paused": return "pause.circle.fill"
        case "abandoned": return "xmark.circle.fill"
        default: return "circle"
        }
    }

    private func statusColor(_ status: String) -> Color {
        switch status.lowercased() {
        case "active": return .green
        case "achieved": return .blue
        case "paused": return .orange
        case "abandoned": return .red
        default: return .secondary
        }
    }

    // MARK: - Sections

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingMD) {
            Text(work.title)
                .font(DiraigentTheme.titleFont)

            HStack(spacing: DiraigentTheme.spacingSM) {
                if let kind = work.workType {
                    WorkKindBadge(kind: kind)
                }
                WorkStatusBadge(status: currentStatus ?? work.status ?? "active")
                if let priority = work.priority {
                    PriorityIndicator(priority: priority)
                }
            }
        }
    }

    private func descriptionSection(_ description: String) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Description", systemImage: "text.alignleft")
                .font(DiraigentTheme.headlineFont)

            Text(description)
                .font(DiraigentTheme.bodyFont)
                .foregroundStyle(.secondary)
        }
    }

    private func successCriteriaSection(_ criteria: String) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Success Criteria", systemImage: "checkmark.circle")
                .font(DiraigentTheme.headlineFont)

            Text(criteria)
                .font(DiraigentTheme.bodyFont)
                .foregroundStyle(.secondary)
        }
    }

    private func progressSection(_ progress: WorkProgress) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            HStack {
                Label("Progress", systemImage: "chart.bar")
                    .font(DiraigentTheme.headlineFont)

                Spacer()

                Text("\(progress.doneTasks)/\(progress.totalTasks)")
                    .font(DiraigentTheme.captionFont.monospacedDigit())
                    .foregroundStyle(.secondary)
            }

            if progress.totalTasks > 0 {
                ProgressView(value: Double(progress.doneTasks), total: Double(progress.totalTasks))
                    .tint(progress.doneTasks == progress.totalTasks ? .green : .blue)
            }
        }
    }

    @ViewBuilder
    private var linkedTasksSection: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Linked Tasks", systemImage: "checklist")
                .font(DiraigentTheme.headlineFont)

            if isLoadingTasks {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                .padding(.vertical, DiraigentTheme.spacingSM)
            } else if linkedTasks.isEmpty {
                Text("No tasks linked to this work item.")
                    .font(DiraigentTheme.captionFont)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(linkedTasks) { task in
                    HStack(spacing: DiraigentTheme.spacingSM) {
                        Image(systemName: taskStateIcon(task.state))
                            .foregroundStyle(taskStateColor(task.state))
                            .frame(width: 20)

                        VStack(alignment: .leading, spacing: 2) {
                            Text(task.title)
                                .font(DiraigentTheme.bodyFont)
                                .lineLimit(1)

                            HStack(spacing: DiraigentTheme.spacingXS) {
                                Text(task.state)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)

                                if let kind = task.kind {
                                    Text("·")
                                        .foregroundStyle(.secondary)
                                    Text(kind)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
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
        }
    }

    private func taskStateIcon(_ state: String) -> String {
        switch state.lowercased() {
        case "done": "checkmark.circle.fill"
        case "cancelled": "xmark.circle.fill"
        case "ready": "circle"
        case "backlog": "circle.dashed"
        default: "gearshape" // working/implement/review etc
        }
    }

    private func taskStateColor(_ state: String) -> Color {
        switch state.lowercased() {
        case "done": .green
        case "cancelled": .secondary
        case "ready": .blue
        case "backlog": .secondary
        default: .orange
        }
    }

    private var metadataSection: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Details", systemImage: "info.circle")
                .font(DiraigentTheme.headlineFont)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ], spacing: DiraigentTheme.spacingSM) {
                if let created = work.createdAt {
                    MetadataItem(label: "Created", value: formatDate(created))
                }
                if let updated = work.updatedAt {
                    MetadataItem(label: "Updated", value: formatDate(updated))
                }
                if let priority = work.priority {
                    MetadataItem(label: "Priority", value: "\(priority)")
                }
                if work.autoStatus == true {
                    MetadataItem(label: "Auto Status", value: "Enabled")
                }
            }
        }
    }

    // MARK: - Helpers

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

/// A labeled metadata item for detail views.
struct MetadataItem: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.subheadline)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
