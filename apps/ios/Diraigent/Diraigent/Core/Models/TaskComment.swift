import Foundation

/// A comment on a task.
struct TaskComment: Codable, Identifiable, Sendable {
    let id: UUID?
    let taskId: UUID?
    let agentId: UUID?
    let userId: UUID?
    let content: String
    let authorName: String?
    let createdAt: String?
}
