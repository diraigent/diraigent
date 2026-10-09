import Foundation

/// Service for managing chat conversations with SSE streaming support.
@Observable
@MainActor
final class ChatService {
    private let apiClient: APIClient

    var messages: [ChatMessage] = []
    var historyLoading = false
    var historyAvailable = true
    private var remoteHistory = false
    private var historyRevision = 0
    private var historyProjectId: UUID?
    private var historyGeneration = UUID()

    func loadHistory(projectId: UUID?) async {
        let generation = UUID()
        historyGeneration = generation
        if historyProjectId != projectId {
            cancelStreaming()
            messages.removeAll()
            remoteHistory = false
            historyProjectId = projectId
        }
        guard let projectId else { historyLoading = false; return }
        historyLoading = true
        defer { if historyGeneration == generation { historyLoading = false } }
        do {
            let history: ChatHistory = try await apiClient.get(Endpoints.chat(projectId) + "/history")
            guard historyGeneration == generation, !Task.isCancelled else { return }
            remoteHistory = history.enabled
            historyRevision = history.revision
            historyAvailable = !history.busy
            if error?.hasPrefix("Conversation storage unavailable.") == true || error?.hasPrefix("A conversation is running on Orchestra.") == true { error = nil }
            if history.enabled { messages = history.messages.compactMap { message in
                guard let role = ChatRole(rawValue: message.role) else { return nil }
                return ChatMessage(role: role, content: message.content)
            } }
            if history.busy { error = "A conversation is running on Orchestra. Reopen chat when it finishes." }
        } catch {
            guard historyGeneration == generation, !Task.isCancelled else { return }
            historyAvailable = false
            self.error = "Conversation storage unavailable. Reopen chat to retry."
        }
    }
    var isStreaming = false
    var error: String?
    var modelCatalog: ChatModelCatalog?
    var modelsLoading = false
    var modelsError: String?
    var selectedModel = ""
    private(set) var modelProvider = "opencode"
    private var modelProjectId: UUID?
    private var modelGeneration = UUID()

    var modelLabel: String { selectedModel.isEmpty ? modelCatalog?.defaultModel ?? "Worker default" : selectedModel }

    func loadModels(projectId: UUID?, provider: String = "opencode", refresh: Bool = false) async {
        let generation = UUID()
        modelGeneration = generation
        if modelProjectId != projectId || modelProvider != provider {
            modelCatalog = nil
            selectedModel = ""
            modelProjectId = projectId
            modelProvider = provider
            if let projectId {
                let stored = UserDefaults.standard.string(forKey: modelStorageKey(projectId, provider)) ?? ""
                if stored.isEmpty || isValidModel(stored) { selectedModel = stored }
            }
        }
        modelsError = nil
        modelsLoading = projectId != nil
        guard let projectId else { return }
        defer { if modelGeneration == generation { modelsLoading = false } }
        do {
            let catalog: ChatModelCatalog = try await apiClient.get(
                Endpoints.chat(projectId) + "/models", query: ["refresh": String(refresh)])
            guard modelGeneration == generation, !Task.isCancelled else { return }
            guard catalog.provider == provider else {
                modelsError = "Chat provider changed. Reload the project to refresh its models."
                modelCatalog = nil
                return
            }
            modelCatalog = catalog
        } catch {
            guard modelGeneration == generation, !Task.isCancelled else { return }
            modelCatalog = nil
            modelsError = "Model list unavailable. Refresh or enter a model manually."
        }
    }

    func setModel(_ model: String) {
        let value = model.trimmingCharacters(in: .whitespacesAndNewlines)
        guard value.isEmpty || isValidModel(value) else { return }
        selectedModel = value
        if let projectId = modelProjectId {
            UserDefaults.standard.set(selectedModel, forKey: modelStorageKey(projectId, modelProvider))
        }
    }

    func isValidModel(_ model: String) -> Bool {
        guard !model.isEmpty, model.count <= 256, !model.contains(where: { $0.isWhitespace }) else { return false }
        if modelProvider != "opencode" { return true }
        return model.range(of: "^[^/\\s]+/\\S+$", options: .regularExpression) != nil
    }

    private func modelStorageKey(_ projectId: UUID, _ provider: String) -> String {
        "chat-model:\(projectId.uuidString):\(provider)"
    }

    /// The current streaming task, kept so it can be cancelled.
    private var streamTask: Task<Void, Never>?
    private var streamGeneration = UUID()

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Send a message and stream the assistant response via SSE.
    func sendMessage(_ content: String, projectId: UUID, model: String? = nil) {
        guard historyAvailable, !historyLoading, !isStreaming else { return }
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
        let chosenModel = model ?? (selectedModel.isEmpty ? nil : selectedModel)
        let modelAgentId = chosenModel == nil ? nil : modelCatalog?.agentId

        let generation = UUID()
        streamGeneration = generation
        streamTask = Task { [weak self] in
            guard let self else { return }
            do {
                let bytes = try await apiClient.stream(
                    Endpoints.chat(projectId),
                    body: ChatRequest(messages: requestMessages, model: chosenModel, agentId: modelAgentId, historyRevision: remoteHistory ? historyRevision : nil)
                )

                defer { bytes.task.cancel() }

                var eventType: String?
                var dataBuffer = ""

                for try await line in bytes.lines {
                    if Task.isCancelled || self.streamGeneration != generation { break }

                    if line.hasPrefix("event:") {
                        eventType = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces)
                    } else if line.hasPrefix("data:") {
                        dataBuffer = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)

                        if let type = eventType, !dataBuffer.isEmpty {
                            let event = Self.parseSSEEvent(type: type, data: dataBuffer)
                            self.handleSSEEvent(event, assistantIndex: assistantIndex)
                            if type == "done" || type == "error" { break }
                        }

                        eventType = nil
                        dataBuffer = ""
                    }
                    // Ignore empty lines and comments (SSE spec)
                }
            } catch is CancellationError {
                // Task was cancelled, do nothing
            } catch {
                guard self.streamGeneration == generation else { return }
                self.error = error.localizedDescription
                // Remove empty assistant message if no content was received
                if assistantIndex < self.messages.count,
                   self.messages[assistantIndex].content.isEmpty {
                    self.messages.remove(at: assistantIndex)
                }
                print("[ChatService] streaming failed: \(error)")
            }

            guard self.streamGeneration == generation else { return }
            if self.remoteHistory, self.historyProjectId == projectId {
                await self.loadHistory(projectId: projectId)
            }
            if self.streamGeneration == generation { self.isStreaming = false }
        }
    }

    /// Cancel any active streaming.
    func cancelStreaming() {
        streamGeneration = UUID()
        streamTask?.cancel()
        streamTask = nil
        isStreaming = false
    }

    /// Clear all messages.
    func clearMessages() {
        cancelStreaming()
        if remoteHistory, let projectId = historyProjectId {
            Task { [weak self] in
                guard let self else { return }
                do {
                    let _: ChatHistory = try await apiClient.post(Endpoints.chat(projectId) + "/history/clear", body: ClearChatHistory(revision: historyRevision))
                    if historyProjectId == projectId { await loadHistory(projectId: projectId) }
                } catch { self.error = "Conversation changed or is busy. Reopen chat before clearing." }
            }
        } else {
            messages.removeAll()
            error = nil
        }
    }

    // MARK: - Private Helpers

    private static func parseSSEEvent(type: String, data: String) -> ChatSseEvent {
        guard let jsonData = data.data(using: .utf8) else {
            return .error("Failed to parse SSE data")
        }

        do {
            let json = try JSONSerialization.jsonObject(with: jsonData) as? [String: Any] ?? [:]

            switch type {
            case "thinking":
                return .thinking
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
        case .thinking:
            break
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
        case .error(let message):
            error = message
            if messages[assistantIndex].content.isEmpty {
                messages[assistantIndex].content = "Error: \(message)"
            }
        }
    }
}
