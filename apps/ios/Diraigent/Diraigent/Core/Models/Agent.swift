import Foundation

/// A diraigent agent.
struct Agent: Codable, Identifiable, Sendable {
    let id: UUID
    let name: String
    let status: String?
    let capabilities: [String]?
    let lastSeenAt: String?
    let ownerId: UUID?
    let metadata: [String: AnyCodable]?
    let createdAt: String?
    let updatedAt: String?
}
