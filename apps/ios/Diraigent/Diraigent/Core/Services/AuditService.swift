import Foundation
import SwiftUI

/// Service for fetching audit log entries.
@Observable
@MainActor
final class AuditService {
    private let apiClient: APIClient

    var entries: [AuditEntry] = []
    var isLoading = false
    var isLoadingMore = false
    var error: String?
    var hasMore = false
    var total = 0

    private let pageSize = 50

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch the first page of audit entries for a project.
    func fetchAuditEntries(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: PaginatedResponse<AuditEntry> = try await apiClient.get(
                Endpoints.audit(projectId),
                query: ["limit": "\(pageSize)", "offset": "0"]
            )
            entries = result.data
            total = result.total
            hasMore = result.hasMore
        } catch {
            self.error = error.localizedDescription
            print("[AuditService] fetchAuditEntries failed: \(error)")
        }
        isLoading = false
    }

    /// Load more audit entries (next page).
    func loadMore(projectId: UUID) async {
        guard !isLoadingMore, hasMore else { return }
        isLoadingMore = true
        do {
            let result: PaginatedResponse<AuditEntry> = try await apiClient.get(
                Endpoints.audit(projectId),
                query: ["limit": "\(pageSize)", "offset": "\(entries.count)"]
            )
            entries.append(contentsOf: result.data)
            total = result.total
            hasMore = result.hasMore
        } catch {
            self.error = error.localizedDescription
            print("[AuditService] loadMore failed: \(error)")
        }
        isLoadingMore = false
    }

    /// Fetch audit entries for a specific entity.
    func fetchEntityHistory(projectId: UUID, entityType: String, entityId: UUID) async -> [AuditEntry] {
        do {
            let result: PaginatedResponse<AuditEntry> = try await apiClient.get(
                Endpoints.audit(projectId),
                query: [
                    "entity_type": entityType,
                    "entity_id": entityId.uuidString,
                    "limit": "100",
                    "offset": "0",
                ]
            )
            return result.data
        } catch {
            print("[AuditService] fetchEntityHistory failed: \(error)")
            return []
        }
    }
}
