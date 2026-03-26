import SwiftUI

/// Source code browser with file tree navigation and file content viewer.
struct SourceBrowserView: View {
    @Environment(AppState.self) private var appState

    @State private var currentPath: String = ""
    @State private var pathHistory: [String] = []
    @State private var selectedFile: TreeEntry?
    @State private var showingFileContent = false

    private var sourceService: SourceService { appState.sourceService }

    var body: some View {
        VStack(spacing: 0) {
            // Current path breadcrumb
            pathHeader

            Divider()

            // File tree or loading/error/empty states
            if sourceService.isLoading && sourceService.entries.isEmpty && !showingFileContent {
                Spacer()
                LoadingView("Loading tree...")
                Spacer()
            } else if let error = sourceService.error, sourceService.entries.isEmpty && !showingFileContent {
                Spacer()
                ErrorView(error) {
                    Task { await loadTree() }
                }
                Spacer()
            } else if showingFileContent {
                fileContentView
            } else if sourceService.entries.isEmpty {
                Spacer()
                EmptyStateView(
                    icon: "folder",
                    title: "Empty Directory",
                    subtitle: "This directory has no files."
                )
                Spacer()
            } else {
                fileTreeList
            }
        }
        .navigationTitle("Source")
        .task {
            if let projectId = appState.selectedProjectId {
                await sourceService.fetchTree(projectId: projectId, path: currentPath)
            }
        }
    }

    // MARK: - Path Header

    @ViewBuilder
    private var pathHeader: some View {
        HStack(spacing: DiraigentTheme.spacingSM) {
            Image(systemName: "folder.fill")
                .foregroundStyle(.blue)
                .font(.caption)

            Text(currentPath.isEmpty ? "/" : "/\(currentPath)")
                .font(.system(.subheadline, design: .monospaced))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer()

            if sourceService.isLoading {
                ProgressView()
                    .controlSize(.small)
            }
        }
        .padding(.horizontal, DiraigentTheme.spacingLG)
        .padding(.vertical, DiraigentTheme.spacingSM)
        .background(DiraigentTheme.surface)
    }

    // MARK: - File Tree List

    @ViewBuilder
    private var fileTreeList: some View {
        List {
            // Back entry when inside a subdirectory
            if !currentPath.isEmpty {
                Button {
                    navigateUp()
                } label: {
                    HStack(spacing: DiraigentTheme.spacingSM) {
                        Image(systemName: "arrow.turn.up.left")
                            .foregroundStyle(.blue)
                            .frame(width: 20)
                        Text("..")
                            .font(.system(.body, design: .monospaced))
                            .foregroundStyle(.blue)
                        Spacer()
                    }
                }
            }

            // Tree entries
            ForEach(sourceService.entries) { entry in
                Button {
                    handleEntryTap(entry)
                } label: {
                    HStack(spacing: DiraigentTheme.spacingSM) {
                        Image(systemName: entry.icon)
                            .foregroundStyle(entry.iconColor)
                            .frame(width: 20)

                        Text(entry.name)
                            .font(.system(.body, design: .monospaced))
                            .foregroundStyle(.primary)
                            .lineLimit(1)

                        Spacer()

                        if entry.isDirectory {
                            Image(systemName: "chevron.right")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }
                }
            }
        }
        .listStyle(.plain)
        .refreshable {
            await loadTree()
        }
    }

    // MARK: - File Content View

    @ViewBuilder
    private var fileContentView: some View {
        VStack(spacing: 0) {
            // File header with back button
            HStack(spacing: DiraigentTheme.spacingSM) {
                Button {
                    showingFileContent = false
                    selectedFile = nil
                    sourceService.blobContent = nil
                } label: {
                    HStack(spacing: DiraigentTheme.spacingXS) {
                        Image(systemName: "chevron.left")
                        Text("Back")
                    }
                    .font(.subheadline)
                }

                Spacer()

                if let file = selectedFile {
                    Text(file.name)
                        .font(.system(.subheadline, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                if let size = sourceService.blobSize {
                    Text(formattedSize(size))
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }
            .padding(.horizontal, DiraigentTheme.spacingLG)
            .padding(.vertical, DiraigentTheme.spacingSM)
            .background(DiraigentTheme.surface)

            Divider()

            // Content area
            if sourceService.isLoading {
                Spacer()
                LoadingView("Loading file...")
                Spacer()
            } else if let error = sourceService.error {
                Spacer()
                ErrorView(error) {
                    if let file = selectedFile, let projectId = appState.selectedProjectId {
                        Task {
                            await sourceService.fetchBlob(projectId: projectId, path: file.path)
                        }
                    }
                }
                Spacer()
            } else if let content = sourceService.blobContent {
                fileContentScrollView(content: content)
            } else {
                Spacer()
                EmptyStateView(
                    icon: "doc",
                    title: "No Content",
                    subtitle: "Unable to load file content."
                )
                Spacer()
            }
        }
    }

    /// Scrollable file content with line numbers.
    @ViewBuilder
    private func fileContentScrollView(content: String) -> some View {
        let lines = content.components(separatedBy: "\n")
        let lineNumberWidth = max(30.0, CGFloat(String(lines.count).count) * 10 + 16)

        ScrollView([.horizontal, .vertical]) {
            HStack(alignment: .top, spacing: 0) {
                // Line numbers column
                VStack(alignment: .trailing, spacing: 0) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { index, _ in
                        Text("\(index + 1)")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.gray)
                            .frame(width: lineNumberWidth, alignment: .trailing)
                            .padding(.trailing, DiraigentTheme.spacingSM)
                            .padding(.vertical, 1)
                    }
                }
                .background(DiraigentTheme.surface)

                Divider()

                // Code content column
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                        Text(line.isEmpty ? " " : line)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.primary)
                            .textSelection(.enabled)
                            .padding(.leading, DiraigentTheme.spacingSM)
                            .padding(.vertical, 1)
                    }
                }
            }
            .padding(.vertical, DiraigentTheme.spacingXS)
        }
    }

    // MARK: - Navigation

    private func handleEntryTap(_ entry: TreeEntry) {
        if entry.isDirectory {
            navigateInto(entry)
        } else {
            viewFile(entry)
        }
    }

    private func navigateInto(_ entry: TreeEntry) {
        pathHistory.append(currentPath)
        currentPath = entry.path
        Task { await loadTree() }
    }

    private func navigateUp() {
        if let previous = pathHistory.popLast() {
            currentPath = previous
            Task { await loadTree() }
        }
    }

    private func viewFile(_ entry: TreeEntry) {
        selectedFile = entry
        showingFileContent = true
        if let projectId = appState.selectedProjectId {
            Task {
                await sourceService.fetchBlob(projectId: projectId, path: entry.path)
            }
        }
    }

    private func loadTree() async {
        if let projectId = appState.selectedProjectId {
            await sourceService.fetchTree(projectId: projectId, path: currentPath)
        }
    }

    // MARK: - Helpers

    private func formattedSize(_ bytes: Int) -> String {
        if bytes < 1024 {
            return "\(bytes) B"
        } else if bytes < 1024 * 1024 {
            return String(format: "%.1f KB", Double(bytes) / 1024)
        } else {
            return String(format: "%.1f MB", Double(bytes) / (1024 * 1024))
        }
    }
}
