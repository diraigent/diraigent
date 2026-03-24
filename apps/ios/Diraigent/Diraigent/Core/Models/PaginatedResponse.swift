import Foundation

/// Generic paginated response wrapper.
struct PaginatedResponse<T: Codable & Sendable>: Codable, Sendable {
    let data: [T]
    let hasMore: Bool?
}
