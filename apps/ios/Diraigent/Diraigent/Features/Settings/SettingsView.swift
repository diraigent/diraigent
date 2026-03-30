import SwiftUI

/// Settings view with account, server, project, and app information.
struct SettingsView: View {
    @Environment(AppState.self) private var appState

    private var currentProject: Project? {
        appState.projectService.projects.first(where: { $0.id == appState.selectedProjectId })
    }

    var body: some View {
        List {
            // Account section
            Section("Account") {
                if let user = appState.authService.currentUser {
                    if let name = user.name {
                        SettingsRow(icon: "person.fill", title: "Name", value: name)
                    }
                    if let email = user.email {
                        SettingsRow(icon: "envelope.fill", title: "Email", value: email)
                    }
                    if let username = user.preferredUsername {
                        SettingsRow(icon: "at", title: "Username", value: username)
                    }
                } else {
                    SettingsRow(icon: "person.fill", title: "Status", value: appState.authService.isAuthenticated ? "Authenticated" : "Not signed in")
                }

                Button(role: .destructive) {
                    appState.authService.logout()
                } label: {
                    Label("Sign Out", systemImage: "rectangle.portrait.and.arrow.right")
                }
            }

            // Server section
            Section("Server") {
                SettingsRow(
                    icon: "server.rack",
                    title: "API URL",
                    value: AppConfig.current.apiBaseURL
                )

                SettingsRow(
                    icon: "circle.fill",
                    title: "Environment",
                    value: environmentName,
                    valueColor: environmentColor
                )
            }

            // Project section
            if let project = currentProject {
                Section("Project") {
                    SettingsRow(icon: "folder.fill", title: "Name", value: project.name)
                    SettingsRow(icon: "tag.fill", title: "Slug", value: project.slug)
                    if let branch = project.defaultBranch {
                        SettingsRow(icon: "arrow.triangle.branch", title: "Default Branch", value: branch)
                    }
                    if let gitMode = project.gitMode {
                        SettingsRow(icon: "arrow.triangle.merge", title: "Git Mode", value: gitMode)
                    }
                    if let repoUrl = project.repoUrl {
                        SettingsRow(icon: "link", title: "Repository", value: repoUrl)
                    }
                }
            }

            // App section
            Section("App") {
                SettingsRow(icon: "app.fill", title: "Version", value: appVersion)
                SettingsRow(icon: "hammer.fill", title: "Build", value: buildNumber)
            }

            #if DEBUG
            // Debug section
            Section("Debug") {
                Button {
                    // Clear UserDefaults cache
                    if let bundleId = Bundle.main.bundleIdentifier {
                        UserDefaults.standard.removePersistentDomain(forName: bundleId)
                    }
                } label: {
                    Label("Clear Cache", systemImage: "trash")
                }

                SettingsRow(icon: "ant.fill", title: "Configuration", value: "Debug")
            }
            #endif
        }
        .navigationTitle("Settings")
    }

    // MARK: - Helpers

    private var environmentName: String {
        #if DEBUG
        "Development"
        #else
        "Production"
        #endif
    }

    private var environmentColor: Color {
        #if DEBUG
        .orange
        #else
        .green
        #endif
    }

    private var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
    }

    private var buildNumber: String {
        Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "1"
    }
}

/// A single settings row with icon, label, and value.
struct SettingsRow: View {
    let icon: String
    let title: String
    let value: String
    var valueColor: Color = .secondary

    var body: some View {
        HStack {
            Label(title, systemImage: icon)
            Spacer()
            Text(value)
                .foregroundStyle(valueColor)
                .font(.subheadline)
                .lineLimit(1)
        }
    }
}
