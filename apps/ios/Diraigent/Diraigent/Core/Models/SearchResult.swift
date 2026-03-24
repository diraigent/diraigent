import Foundation

/// A single search result.
struct SearchResult: Codable, Identifiable, Sendable {
    var id: UUID { entityId }
    let entityType: String
    let entityId: UUID
    let title: String
    let snippet: String?
    let relevance: Float?
    let createdAt: String?
}

/// Response from the search endpoint.
struct SearchResponse: Codable, Sendable {
    let results: [SearchResult]
    let total: Int?
    let query: String?
}
