import Foundation

/// A webhook subscription.
struct Webhook: Codable, Identifiable, Sendable {
    let id: UUID
    let projectId: UUID?
    let name: String
    let url: String
    let secret: String?
    let events: [String]
    let enabled: Bool
    let metadata: [String: AnyCodable]?
    let createdAt: String?
    let updatedAt: String?
}

/// A single delivery attempt for a webhook.
struct WebhookDelivery: Codable, Identifiable, Sendable {
    let id: UUID
    let webhookId: UUID?
    let eventType: String
    let payload: [String: AnyCodable]?
    let responseStatus: Int?
    let responseBody: String?
    let deliveredAt: String?
    let success: Bool
    let attemptNumber: Int?
}
