import Foundation

/// A diraigent report (security, component, architecture, performance, custom analysis).
struct Report: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let title: String
    let kind: String?
    let status: String?
    let prompt: String?
    let result: String?
    let taskId: UUID?
    let createdBy: UUID?
    let metadata: [String: AnyCodable]?
    let createdAt: String?
    let updatedAt: String?
}
