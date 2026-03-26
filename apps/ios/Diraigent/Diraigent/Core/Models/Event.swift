import Foundation

/// A project event.
struct Event: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let kind: String?
    let source: String?
    let title: String?
    let description: String?
    let severity: String?
    let metadata: [String: AnyCodable]?
    let relatedTaskId: UUID?
    let agentId: UUID?
    let createdAt: String?
}
