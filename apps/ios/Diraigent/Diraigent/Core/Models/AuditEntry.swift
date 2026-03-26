import Foundation

/// An audit log entry.
struct AuditEntry: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let actorAgentId: UUID?
    let actorUserId: UUID?
    let action: String?
    let entityType: String?
    let entityId: UUID?
    let summary: String?
    let beforeState: AnyCodable?
    let afterState: AnyCodable?
    let metadata: [String: AnyCodable]?
    let createdAt: String?
}
