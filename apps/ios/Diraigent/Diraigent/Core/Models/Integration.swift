import Foundation

/// An external integration (CI, monitoring, logging, VCS, chat, custom).
struct Integration: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let name: String
    let kind: String?
    let provider: String?
    let baseUrl: String?
    let authType: String?
    let config: [String: AnyCodable]?
    let capabilities: [String]?
    let enabled: Bool
    let createdAt: String?
    let updatedAt: String?
}
