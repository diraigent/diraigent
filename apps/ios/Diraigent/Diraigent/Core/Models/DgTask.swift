import Foundation

/// A diraigent task.
struct DgTask: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let number: Int?
    let title: String
    let state: String
    let kind: String?
    let urgent: Bool?
    let flagged: Bool?
    let assignedAgentId: UUID?
    let assignedRoleId: UUID?
    let context: [String: AnyCodable]?
    let costUsd: Double?
    let inputTokens: Int?
    let outputTokens: Int?
    let parentId: UUID?
    let playbookId: UUID?
    let playbookStep: Int?
    let decisionId: UUID?
    let delegatedBy: UUID?
    let createdBy: String?
    let createdAt: String?
    let updatedAt: String?
    let claimedAt: String?
    let completedAt: String?
    let revertedAt: String?
}
