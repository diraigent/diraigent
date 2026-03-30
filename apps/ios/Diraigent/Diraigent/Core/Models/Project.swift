import Foundation

/// A diraigent project.
struct Project: Codable, Identifiable, Sendable {
    let id: UUID
    let name: String
    let slug: String
    let description: String?
    let defaultBranch: String?
    let gitMode: String?
    let repoUrl: String?
    let repoPath: String?
    let serviceName: String?
    let defaultPlaybookId: UUID?
    let metadata: [String: AnyCodable]?
    let createdAt: String?
    let updatedAt: String?
}
