import Foundation

/// Flexible type for success_criteria — can be a plain string or a JSON array/object.
enum FlexibleString: Codable, Sendable {
    case string(String)
    case other(AnyCodable)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let str = try? container.decode(String.self) {
            self = .string(str)
        } else {
            let value = try container.decode(AnyCodable.self)
            self = .other(value)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let str): try container.encode(str)
        case .other(let value): try container.encode(value)
        }
    }

    var displayText: String {
        switch self {
        case .string(let str): return str
        case .other(let value): return value.stringValue
        }
    }
}

/// A diraigent work item (epic, feature, milestone, sprint, initiative).
struct Work: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let title: String
    let description: String?
    let workType: String?
    let status: String?
    let priority: Int?
    let parentWorkId: UUID?
    let autoStatus: Bool?
    let successCriteria: FlexibleString?
    let metadata: [String: AnyCodable]?
    let createdBy: String?
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
