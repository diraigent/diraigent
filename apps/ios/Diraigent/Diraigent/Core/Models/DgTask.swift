import Foundation

/// Task context — spec, files, test command, acceptance criteria.
struct TaskContext: Codable, Sendable {
    let spec: String?
    let files: [String]?
    let testCmd: String?
    let acceptanceCriteria: [String]?
    let notes: String?
}

/// A diraigent task.
struct DgTask: Codable, Identifiable, Sendable {
    let id: UUID
    let number: Int?
    let title: String
    let state: String
    let kind: String?
    let priority: Int?
    let urgent: Bool?
    let flagged: Bool?
    let assignedAgentId: UUID?
    let assignedRoleId: UUID?
    let context: TaskContext?
    let costUsd: Double?
    let inputTokens: Int?
    let outputTokens: Int?
    let parentId: UUID?
    let playbookId: UUID?
    let playbookStep: Int?
    let projectId: UUID?
    let createdAt: String?
    let updatedAt: String?
    let claimedAt: String?
    let completedAt: String?
}
