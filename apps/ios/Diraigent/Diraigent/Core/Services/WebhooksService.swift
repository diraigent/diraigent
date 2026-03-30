import Foundation
import SwiftUI

/// Request body for creating a webhook.
struct CreateWebhookRequest: Encodable, Sendable {
    let name: String
    let url: String
    let secret: String?
    let events: [String]
}

/// Request body for toggling a webhook's enabled status.
struct UpdateWebhookRequest: Encodable, Sendable {
    let enabled: Bool
}

/// Service for managing webhooks.
@Observable
@MainActor
final class WebhooksService {
    private let apiClient: APIClient

    var webhooks: [Webhook] = []
    var deliveries: [WebhookDelivery] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all webhooks for a project.
    func fetchWebhooks(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [Webhook] = try await apiClient.get(Endpoints.webhooks(projectId))
            webhooks = result
        } catch {
            self.error = error.localizedDescription
            print("[WebhooksService] fetchWebhooks failed: \(error)")
        }
        isLoading = false
    }

    /// Create a new webhook.
    func createWebhook(projectId: UUID, request: CreateWebhookRequest) async -> Webhook? {
        do {
            let result: Webhook = try await apiClient.post(Endpoints.webhooks(projectId), body: request)
            webhooks.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            print("[WebhooksService] createWebhook failed: \(error)")
            return nil
        }
    }

    /// Delete a webhook.
    func deleteWebhook(webhookId: UUID) async -> Bool {
        do {
            try await apiClient.delete(Endpoints.webhook(webhookId))
            webhooks.removeAll { $0.id == webhookId }
            return true
        } catch {
            self.error = error.localizedDescription
            print("[WebhooksService] deleteWebhook failed: \(error)")
            return false
        }
    }

    /// Toggle a webhook's enabled status.
    func toggleWebhook(webhookId: UUID, enabled: Bool) async -> Bool {
        do {
            let body = UpdateWebhookRequest(enabled: enabled)
            let updated: Webhook = try await apiClient.put(Endpoints.webhook(webhookId), body: body)
            if let index = webhooks.firstIndex(where: { $0.id == webhookId }) {
                webhooks[index] = updated
            }
            return true
        } catch {
            self.error = error.localizedDescription
            print("[WebhooksService] toggleWebhook failed: \(error)")
            return false
        }
    }

    /// Test a webhook by triggering a test delivery.
    func testWebhook(webhookId: UUID) async -> Bool {
        do {
            try await apiClient.post(Endpoints.webhookTest(webhookId))
            return true
        } catch {
            self.error = error.localizedDescription
            print("[WebhooksService] testWebhook failed: \(error)")
            return false
        }
    }

    /// Fetch recent deliveries for a webhook.
    func listDeliveries(webhookId: UUID) async {
        do {
            let result: [WebhookDelivery] = try await apiClient.get(Endpoints.webhookDeliveries(webhookId))
            deliveries = result
        } catch {
            self.error = error.localizedDescription
            print("[WebhooksService] listDeliveries failed: \(error)")
        }
    }
}
