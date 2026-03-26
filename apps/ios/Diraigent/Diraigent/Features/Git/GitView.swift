import SwiftUI

/// Git branches view showing task branches and other branches with ahead/behind indicators.
struct GitView: View {
    @Environment(AppState.self) private var appState

    private var gitService: GitService { appState.gitService }

    /// Task branches (agent/task-*).
    private var taskBranches: [BranchInfo] {
        gitService.branches.filter { $0.name.hasPrefix("agent/task-") }
    }

    /// All other branches.
    private var otherBranches: [BranchInfo] {
        gitService.branches.filter { !$0.name.hasPrefix("agent/task-") }
    }

    var body: some View {
        Group {
            if gitService.isLoading && gitService.branches.isEmpty {
                LoadingView("Loading branches...")
            } else if let error = gitService.error, gitService.branches.isEmpty {
                ErrorView(error) {
                    Task { await loadBranches() }
                }
            } else if gitService.branches.isEmpty {
                EmptyStateView(
                    icon: "arrow.triangle.branch",
                    title: "No Branches",
                    subtitle: "No git branch information available."
                )
            } else {
                List {
                    if !gitService.currentBranch.isEmpty {
                        Section {
                            HStack {
                                Image(systemName: "arrow.triangle.branch")
                                    .foregroundStyle(.green)
                                Text(gitService.currentBranch)
                                    .font(DiraigentTheme.headlineFont)
                                Spacer()
                                Text("Current")
                                    .font(.caption)
                                    .foregroundStyle(.green)
                            }
                        } header: {
                            Text("Current Branch")
                        }
                    }

                    if !taskBranches.isEmpty {
                        Section {
                            ForEach(taskBranches) { branch in
                                BranchRowView(branch: branch, isCurrent: branch.name == gitService.currentBranch)
                            }
                        } header: {
                            Text("Task Branches (\(taskBranches.count))")
                        }
                    }

                    if !otherBranches.isEmpty {
                        Section {
                            ForEach(otherBranches) { branch in
                                BranchRowView(branch: branch, isCurrent: branch.name == gitService.currentBranch)
                            }
                        } header: {
                            Text("Branches (\(otherBranches.count))")
                        }
                    }
                }
                .refreshable {
                    await loadBranches()
                }
            }
        }
        .navigationTitle("Git")
        .task {
            await loadBranches()
        }
    }

    private func loadBranches() async {
        guard let projectId = appState.selectedProjectId else { return }
        await gitService.fetchBranches(projectId: projectId)
    }
}

/// Row for a single branch.
struct BranchRowView: View {
    let branch: BranchInfo
    let isCurrent: Bool

    var body: some View {
        HStack(spacing: DiraigentTheme.spacingMD) {
            Image(systemName: "arrow.triangle.branch")
                .foregroundStyle(isCurrent ? .green : .secondary)
                .font(.caption)

            VStack(alignment: .leading, spacing: DiraigentTheme.spacingXS) {
                Text(branch.name)
                    .font(DiraigentTheme.headlineFont)
                    .lineLimit(1)

                if let commit = branch.commit {
                    Text(String(commit.prefix(8)))
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            // Ahead/behind indicators
            HStack(spacing: DiraigentTheme.spacingSM) {
                if let ahead = branch.aheadRemote, ahead > 0 {
                    HStack(spacing: 2) {
                        Image(systemName: "arrow.up")
                            .font(.caption2)
                        Text("\(ahead)")
                            .font(.caption.monospacedDigit())
                    }
                    .foregroundStyle(.green)
                }

                if let behind = branch.behindRemote, behind > 0 {
                    HStack(spacing: 2) {
                        Image(systemName: "arrow.down")
                            .font(.caption2)
                        Text("\(behind)")
                            .font(.caption.monospacedDigit())
                    }
                    .foregroundStyle(.orange)
                }

                if branch.isPushed == true {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.caption)
                        .foregroundStyle(.green)
                }
            }
        }
        .padding(.vertical, DiraigentTheme.spacingXS)
    }
}
