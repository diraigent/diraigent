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
    let createdAt: String?
}
