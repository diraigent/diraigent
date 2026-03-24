import Foundation
import SwiftUI

/// Service for querying Loki logs through the diraigent API.
@Observable
@MainActor
final class LogsService {
    private let apiClient: APIClient

    var entries: [LogEntry] = []
    var labels: [String] = []
    var total: Int = 0
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Query logs with the given parameters.
    func queryLogs(
        query: String,
        start: String?,
        end: String?,
        limit: Int,
        direction: String
    ) async {
        isLoading = true
        error = nil
        do {
            var queryParams: [String: String] = [
                "query": query,
                "limit": String(limit),
                "direction": direction
            ]
            if let start {
                queryParams["start"] = start
            }
            if let end {
                queryParams["end"] = end
            }
            let result: LogsResponse = try await apiClient.get(Endpoints.logs, query: queryParams)
            entries = result.entries
            total = result.total
        } catch {
            self.error = error.localizedDescription
            print("[LogsService] queryLogs failed: \(error)")
        }
        isLoading = false
    }

    /// Fetch available log labels.
    func fetchLabels() async {
        do {
            let result: LokiLabelsResponse = try await apiClient.get(Endpoints.logLabels)
            labels = result.data ?? []
        } catch {
            print("[LogsService] fetchLabels failed: \(error)")
        }
    }
}
