import Foundation

/// An alternative considered in a decision.
struct DecisionAlternative: Codable, Sendable {
    let name: String
    let pros: String?
    let cons: String?
}

/// Alternatives can be a structured array or a plain string (from agent-cli).
enum DecisionAlternatives: Codable, Sendable {
    case structured([DecisionAlternative])
    case plain(String)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let arr = try? container.decode([DecisionAlternative].self) {
            self = .structured(arr)
        } else if let str = try? container.decode(String.self) {
            self = .plain(str)
        } else {
            self = .structured([])
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .structured(let arr): try container.encode(arr)
        case .plain(let str): try container.encode(str)
        }
    }
}

/// A diraigent decision record.
struct Decision: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let title: String
    let status: String?
    let context: String?
    let decision: String?
    let rationale: String?
    let alternatives: DecisionAlternatives?
    let consequences: String?
    let supersededBy: UUID?
    let metadata: [String: AnyCodable]?
    let createdBy: String?
    let createdAt: String?
    let updatedAt: String?
}
