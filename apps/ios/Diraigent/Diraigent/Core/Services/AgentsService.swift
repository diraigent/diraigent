import Foundation
import SwiftUI

/// Service for fetching agent data.
@Observable
@MainActor
final class AgentsService {
    private let apiClient: APIClient

    var agents: [Agent] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all agents.
    func fetchAgents() async {
        isLoading = true
        error = nil
        do {
            let result: [Agent] = try await apiClient.get(Endpoints.agents)
            agents = result
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    /// Fetch tasks assigned to an agent.
    func fetchAgentTasks(agentId: UUID) async -> [DgTask] {
        do {
            let result: [DgTask] = try await apiClient.get(Endpoints.agentTasks(agentId))
            return result
        } catch {
            return []
        }
    }
}
