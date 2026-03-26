import Foundation

/// A task update (progress, artifact, blocker, etc.).
struct TaskUpdate: Codable, Identifiable, Sendable {
    let id: UUID?
    let taskId: UUID?
    let kind: String
    let content: String
    let createdAt: String?
}
