import Foundation
import SwiftUI

/// Service for searching across project entities.
@Observable
@MainActor
final class SearchService {
    private let apiClient: APIClient

    var results: [SearchResult] = []
    var isLoading = false
    var error: String?

    /// Tracks the current search task so we can cancel stale searches.
    private var searchTask: Task<Void, Never>?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Search across project entities with debouncing.
    /// Cancels any previous in-flight search.
    func search(projectId: UUID, query: String) {
        // Cancel previous search
        searchTask?.cancel()

        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else {
            results = []
            return
        }

        searchTask = Task {
            // Debounce: wait 300ms before actually searching
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }

            isLoading = true
            error = nil

            do {
                let response: SearchResponse = try await apiClient.get(
                    Endpoints.search(projectId),
                    query: ["q": query]
                )
                guard !Task.isCancelled else { return }
                results = response.results
            } catch {
                guard !Task.isCancelled else { return }
                self.error = error.localizedDescription
            }
            isLoading = false
        }
    }

    /// Clear search results.
    func clearResults() {
        searchTask?.cancel()
        results = []
        error = nil
    }
}
