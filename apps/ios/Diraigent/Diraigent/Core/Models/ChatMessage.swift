import Foundation

/// Role of a chat message sender.
enum ChatRole: String, Codable, Sendable {
    case user
    case assistant
}

/// A single message in a chat conversation.
struct ChatMessage: Identifiable, Sendable {
    let id: UUID
    let role: ChatRole
    var content: String
    let timestamp: Date

    init(id: UUID = UUID(), role: ChatRole, content: String, timestamp: Date = Date()) {
        self.id = id
        self.role = role
        self.content = content
        self.timestamp = timestamp
    }
}

/// Request body sent to the chat endpoint.
struct ChatRequest: Encodable, Sendable {
    let messages: [ChatRequestMessage]
    let model: String?
}

/// A message in the chat request payload.
struct ChatRequestMessage: Codable, Sendable {
    let role: String
    let content: String
}

/// SSE event types returned by the chat endpoint.
enum ChatSseEvent {
    case text(String)
    case toolStart(toolName: String, toolId: String)
    case toolEnd(toolId: String, success: Bool)
    case done(role: String, content: String)
    case error(String)
}
