import Foundation
import SwiftUI

/// Service for fetching dashboard data (metrics, events, agents).
@Observable
@MainActor
final class DashboardService {
    private let apiClient: APIClient

    var metrics: ProjectMetrics?
    var recentEvents: [Event] = []
    var agents: [Agent] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all dashboard data for a project in parallel.
    func fetchDashboard(projectId: UUID) async {
        isLoading = true
        error = nil

        async let metricsTask: ProjectMetrics? = fetchMetrics(projectId: projectId)
        async let eventsTask: [Event] = fetchEvents(projectId: projectId)
        async let agentsTask: [Agent] = fetchAgents()

        let (m, e, a) = await (metricsTask, eventsTask, agentsTask)
        metrics = m
        recentEvents = e
        agents = a
        isLoading = false
    }

    // MARK: - Private

    private func fetchMetrics(projectId: UUID) async -> ProjectMetrics? {
        do {
            return try await apiClient.get(Endpoints.projectMetrics(projectId))
        } catch {
            return nil
        }
    }

    private func fetchEvents(projectId: UUID) async -> [Event] {
        do {
            let result: [Event] = try await apiClient.get(
                Endpoints.events(projectId),
                query: ["limit": "10"]
            )
            return result
        } catch {
            return []
        }
    }

    private func fetchAgents() async -> [Agent] {
        do {
            let result: [Agent] = try await apiClient.get(Endpoints.agents)
            return result
        } catch {
            return []
        }
    }
}
