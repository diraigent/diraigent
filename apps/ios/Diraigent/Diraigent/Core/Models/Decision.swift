import Foundation

/// An alternative considered in a decision.
struct DecisionAlternative: Codable, Sendable {
    let name: String
    let pros: String?
    let cons: String?
}

/// A diraigent decision record.
struct Decision: Codable, Identifiable, Sendable {
    let id: UUID
    let title: String
    let description: String?
    let status: String?
    let context: String?
    let decision: String?
    let rationale: String?
    let alternatives: [DecisionAlternative]?
    let consequences: String?
    let supersededBy: UUID?
    let tags: [String]?
    let createdAt: String?
    let updatedAt: String?
}
