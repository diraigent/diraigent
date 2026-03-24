import Foundation
import SwiftUI

/// Service for fetching and caching project data.
@Observable
@MainActor
final class ProjectService {
    private let apiClient: APIClient

    var projects: [Project] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// The currently selected project, derived from AppState's selectedProjectId.
    func selectedProject(id: UUID?) -> Project? {
        guard let id else { return nil }
        return projects.first { $0.id == id }
    }

    /// Fetch all projects the user has access to.
    func fetchProjects() async {
        isLoading = true
        error = nil
        do {
            let result: [Project] = try await apiClient.get(Endpoints.projects)
            projects = result
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    /// Fetch a single project by ID.
    func fetchProject(id: UUID) async -> Project? {
        do {
            let result: Project = try await apiClient.get(Endpoints.project(id))
            return result
        } catch {
            return nil
        }
    }

    /// Fetch metrics for a project.
    func fetchMetrics(projectId: UUID) async -> ProjectMetrics? {
        do {
            let result: ProjectMetrics = try await apiClient.get(Endpoints.projectMetrics(projectId))
            return result
        } catch {
            return nil
        }
    }
}
