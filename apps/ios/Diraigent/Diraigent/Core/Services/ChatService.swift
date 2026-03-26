import Foundation

/// Service for managing chat conversations with SSE streaming support.
@Observable
@MainActor
final class ChatService {
    private let apiClient: APIClient

    var messages: [ChatMessage] = []
    var isStreaming = false
    var error: String?

    /// The current streaming task, kept so it can be cancelled.
    private var streamTask: Task<Void, Never>?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Send a message and stream the assistant response via SSE.
    func sendMessage(_ content: String, projectId: UUID, model: String? = nil) {
        let userMessage = ChatMessage(role: .user, content: content)
        messages.append(userMessage)

        // Build the request messages from conversation history
        let requestMessages = messages.map { msg in
            ChatRequestMessage(role: msg.role.rawValue, content: msg.content)
        }

        // Create a placeholder assistant message to stream content into
        let assistantMessage = ChatMessage(role: .assistant, content: "")
        messages.append(assistantMessage)
        let assistantIndex = messages.count - 1

        isStreaming = true
        error = nil

        streamTask = Task { [weak self] in
            guard let self else { return }
            do {
                let baseURL = AppConfig.current.apiBaseURL
                let token = KeychainHelper.readString(key: "access_token")
                let url = try self.buildChatURL(baseURL: baseURL, projectId: projectId)
                let request = try self.buildStreamRequest(
                    url: url,
                    token: token,
                    messages: requestMessages,
                    model: model
                )

                let (bytes, response) = try await URLSession.shared.bytes(for: request)

                guard let httpResponse = response as? HTTPURLResponse else {
                    throw APIError.serverError(0, "Invalid response")
                }
                guard (200...299).contains(httpResponse.statusCode) else {
                    if httpResponse.statusCode == 401 {
                        throw APIError.unauthorized
                    }
                    throw APIError.serverError(httpResponse.statusCode, "Chat request failed")
                }

                var eventType: String?
                var dataBuffer = ""

                for try await line in bytes.lines {
                    if Task.isCancelled { break }

                    if line.hasPrefix("event:") {
                        eventType = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces)
                    } else if line.hasPrefix("data:") {
                        dataBuffer = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)

                        if let type = eventType, !dataBuffer.isEmpty {
                            let event = Self.parseSSEEvent(type: type, data: dataBuffer)
                            self.handleSSEEvent(event, assistantIndex: assistantIndex)
                        }

                        eventType = nil
                        dataBuffer = ""
                    }
                    // Ignore empty lines and comments (SSE spec)
                }
            } catch is CancellationError {
                // Task was cancelled, do nothing
            } catch {
                self.error = error.localizedDescription
                // Remove empty assistant message if no content was received
                if assistantIndex < self.messages.count,
                   self.messages[assistantIndex].content.isEmpty {
                    self.messages.remove(at: assistantIndex)
                }
                print("[ChatService] streaming failed: \(error)")
            }

            self.isStreaming = false
        }
    }

    /// Cancel any active streaming.
    func cancelStreaming() {
        streamTask?.cancel()
        streamTask = nil
        isStreaming = false
    }

    /// Clear all messages.
    func clearMessages() {
        cancelStreaming()
        messages.removeAll()
        error = nil
    }

    // MARK: - Private Helpers

    private func buildChatURL(baseURL: String, projectId: UUID) throws -> URL {
        let path = Endpoints.chat(projectId)
        guard let url = URL(string: baseURL + path) else {
            throw APIError.invalidURL
        }
        return url
    }

    private func buildStreamRequest(
        url: URL,
        token: String?,
        messages: [ChatRequestMessage],
        model: String?
    ) throws -> URLRequest {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        if let token {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        let body = ChatRequest(messages: messages, model: model)
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        request.httpBody = try encoder.encode(body)

        return request
    }

    private static func parseSSEEvent(type: String, data: String) -> ChatSseEvent {
        guard let jsonData = data.data(using: .utf8) else {
            return .error("Failed to parse SSE data")
        }

        do {
            let json = try JSONSerialization.jsonObject(with: jsonData) as? [String: Any] ?? [:]

            switch type {
            case "text":
                let content = json["content"] as? String ?? ""
                return .text(content)
            case "tool_start":
                let toolName = json["tool_name"] as? String ?? ""
                let toolId = json["tool_id"] as? String ?? ""
                return .toolStart(toolName: toolName, toolId: toolId)
            case "tool_end":
                let toolId = json["tool_id"] as? String ?? ""
                let success = json["success"] as? Bool ?? false
                return .toolEnd(toolId: toolId, success: success)
            case "done":
                if let message = json["message"] as? [String: Any] {
                    let role = message["role"] as? String ?? "assistant"
                    let content = message["content"] as? String ?? ""
                    return .done(role: role, content: content)
                }
                return .done(role: "assistant", content: "")
            case "error":
                let message = json["message"] as? String ?? "Unknown error"
                return .error(message)
            default:
                return .error("Unknown event type: \(type)")
            }
        } catch {
            return .error("Failed to parse SSE JSON: \(error.localizedDescription)")
        }
    }

    private func handleSSEEvent(_ event: ChatSseEvent, assistantIndex: Int) {
        guard assistantIndex < messages.count else { return }

        switch event {
        case .text(let content):
            messages[assistantIndex].content += content
        case .toolStart(let toolName, _):
            if !messages[assistantIndex].content.isEmpty {
                messages[assistantIndex].content += "\n"
            }
            messages[assistantIndex].content += "[Using \(toolName)...]"
        case .toolEnd(_, let success):
            if let range = messages[assistantIndex].content.range(
                of: "\\[Using .*?\\.\\.\\.]",
                options: .regularExpression
            ) {
                let toolText = String(messages[assistantIndex].content[range])
                let toolName = toolText
                    .replacingOccurrences(of: "[Using ", with: "")
                    .replacingOccurrences(of: "...]", with: "")
                let status = success ? "done" : "failed"
                messages[assistantIndex].content.replaceSubrange(
                    range,
                    with: "[\(toolName) \(status)]\n"
                )
            }
        case .done(_, let content):
            if !content.isEmpty {
                messages[assistantIndex].content = content
            }
            isStreaming = false
        case .error(let message):
            error = message
            if messages[assistantIndex].content.isEmpty {
                messages[assistantIndex].content = "Error: \(message)"
            }
            isStreaming = false
        }
    }
}
