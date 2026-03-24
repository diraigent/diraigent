import Foundation
import SwiftUI

/// Request body for creating an event.
struct CreateEventRequest: Encodable, Sendable {
    let title: String
    let kind: String
    let severity: String
    let description: String?
}

/// Service for managing project events.
@Observable
@MainActor
final class EventsService {
    private let apiClient: APIClient

    var events: [Event] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all events for a project.
    func fetchEvents(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [Event] = try await apiClient.get(Endpoints.events(projectId))
            events = result
        } catch {
            self.error = error.localizedDescription
            print("[EventsService] fetchEvents failed: \(error)")
        }
        isLoading = false
    }

    /// Create a new event.
    func createEvent(projectId: UUID, request: CreateEventRequest) async -> Event? {
        do {
            let result: Event = try await apiClient.post(Endpoints.events(projectId), body: request)
            events.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            print("[EventsService] createEvent failed: \(error)")
            return nil
        }
    }

    /// Delete an event.
    func deleteEvent(projectId: UUID, eventId: UUID) async -> Bool {
        do {
            try await apiClient.delete("/events/\(eventId)")
            events.removeAll { $0.id == eventId }
            return true
        } catch {
            self.error = error.localizedDescription
            print("[EventsService] deleteEvent failed: \(error)")
            return false
        }
    }
}
