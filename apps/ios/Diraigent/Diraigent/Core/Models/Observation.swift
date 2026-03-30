import Foundation

/// A diraigent observation (insight, risk, smell, improvement).
/// Named `DgObservation` to avoid collision with Swift's `Observation` framework.
struct DgObservation: Codable, Identifiable, Sendable {
    let id: UUID
    let title: String
    let description: String?
    let kind: String?
    let severity: String?
    let status: String?
    let evidence: [String: AnyCodable]?
    let agentId: UUID?
    let resolvedTaskId: UUID?
    let source: String?
    let sourceTaskId: UUID?
    let createdAt: String?
    let updatedAt: String?
}
