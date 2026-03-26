import Foundation

/// Generic paginated response wrapper.
struct PaginatedResponse<T: Codable & Sendable>: Codable, Sendable {
    let data: [T]
    let total: Int
    let limit: Int
    let offset: Int
    let hasMore: Bool
}
