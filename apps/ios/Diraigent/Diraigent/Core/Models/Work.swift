import Foundation

/// A diraigent work item (epic, feature, milestone, sprint, initiative).
struct Work: Codable, Identifiable, Sendable {
    let id: UUID
    let title: String
    let description: String?
    let workType: String?
    let status: String?
    let priority: Int?
    let parentWorkId: UUID?
    let autoStatus: Bool?
    let successCriteria: [String: AnyCodable]?
    let metadata: [String: AnyCodable]?
    let createdAt: String?
    let updatedAt: String?
}

/// Progress summary for a work item.
struct WorkProgress: Codable, Sendable {
    let workId: UUID
    let totalTasks: Int
    let doneTasks: Int
}

/// Detailed stats for a work item.
struct WorkStats: Codable, Sendable {
    let workId: UUID
    let backlogCount: Int
    let readyCount: Int
    let workingCount: Int
    let doneCount: Int
    let cancelledCount: Int
    let totalCount: Int
    let totalCostUsd: Double
    let totalInputTokens: Int
    let totalOutputTokens: Int
    let blockedCount: Int
    let avgCompletionHours: Double?
    let oldestOpenTaskDate: String?
}
