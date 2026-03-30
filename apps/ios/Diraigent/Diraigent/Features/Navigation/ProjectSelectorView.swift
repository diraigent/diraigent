import SwiftUI

/// Toolbar button that shows the current project name and opens a project picker sheet.
struct ProjectSelectorButton: View {
    @Environment(AppState.self) private var appState
    @State private var showingPicker = false

    var body: some View {
        let currentProject = appState.projectService.selectedProject(id: appState.selectedProjectId)

        Button {
            showingPicker = true
        } label: {
            HStack(spacing: DiraigentTheme.spacingXS) {
                Image(systemName: "folder.fill")
                    .font(.caption)
                Text(currentProject?.name ?? "Select Project")
                    .fontWeight(.medium)
                Image(systemName: "chevron.down")
                    .font(.caption2)
            }
        }
        .sheet(isPresented: $showingPicker) {
            ProjectSelectorView()
        }
    }
}

/// Sheet view listing all projects for selection.
struct ProjectSelectorView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Group {
                if appState.projectService.isLoading && appState.projectService.projects.isEmpty {
                    ProgressView("Loading projects\u{2026}")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if appState.projectService.projects.isEmpty {
                    ContentUnavailableView(
                        "No Projects",
                        systemImage: "folder",
                        description: Text("You don't have access to any projects yet.")
                    )
                } else {
                    projectList
                }
            }
            .navigationTitle("Projects")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .task {
                await appState.projectService.fetchProjects()
            }
        }
    }

    private var projectList: some View {
        List(appState.projectService.projects) { project in
            Button {
                appState.selectProject(project.id)
                dismiss()
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                        Text(project.name)
                            .font(DiraigentTheme.headlineFont)
                            .foregroundStyle(.primary)
                        if let slug = Optional(project.slug), !slug.isEmpty {
                            Text(slug)
                                .font(DiraigentTheme.captionFont)
                                .foregroundStyle(.secondary)
                        }
                        if let desc = project.description, !desc.isEmpty {
                            Text(desc)
                                .font(DiraigentTheme.captionFont)
                                .foregroundStyle(.tertiary)
                                .lineLimit(2)
                        }
                    }

                    Spacer()

                    if project.id == appState.selectedProjectId {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundStyle(DiraigentTheme.primary)
                    }
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }
}
