import SwiftUI

/// Chat view with streaming AI responses.
struct ChatView: View {
    @Environment(AppState.self) private var appState
    @State private var inputText = ""
    @FocusState private var isInputFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            messageList
            Divider()
            inputArea
        }
        .navigationTitle("Chat")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    appState.chatService.clearMessages()
                } label: {
                    Label("Clear", systemImage: "trash")
                }
                .disabled(appState.chatService.messages.isEmpty)
            }
        }
    }

    // MARK: - Message List

    @ViewBuilder
    private var messageList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: DiraigentTheme.spacingMD) {
                    if appState.chatService.messages.isEmpty {
                        emptyState
                    } else {
                        ForEach(appState.chatService.messages) { message in
                            ChatBubble(message: message)
                                .id(message.id)
                        }

                        if appState.chatService.isStreaming {
                            thinkingIndicator
                                .id("thinking")
                        }
                    }
                }
                .padding(.horizontal, DiraigentTheme.spacingLG)
                .padding(.vertical, DiraigentTheme.spacingMD)
            }
            .onChange(of: appState.chatService.messages.count) {
                scrollToBottom(proxy: proxy)
            }
            .onChange(of: appState.chatService.messages.last?.content) {
                scrollToBottom(proxy: proxy)
            }
        }
    }

    private func scrollToBottom(proxy: ScrollViewProxy) {
        withAnimation(.easeOut(duration: 0.2)) {
            if appState.chatService.isStreaming {
                proxy.scrollTo("thinking", anchor: .bottom)
            } else if let lastMessage = appState.chatService.messages.last {
                proxy.scrollTo(lastMessage.id, anchor: .bottom)
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: DiraigentTheme.spacingLG) {
            Spacer()
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 48))
                .foregroundStyle(.tertiary)
            Text("Start a conversation")
                .font(DiraigentTheme.headlineFont)
                .foregroundStyle(.secondary)
            Text("Ask questions about your project, create tasks, or get help with planning.")
                .font(DiraigentTheme.captionFont)
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, DiraigentTheme.spacingXL)
            Spacer()
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 80)
    }

    private var thinkingIndicator: some View {
        HStack(spacing: DiraigentTheme.spacingSM) {
            ProgressView()
                .controlSize(.small)
            Text("Thinking...")
                .font(DiraigentTheme.captionFont)
                .foregroundStyle(.secondary)
            Spacer()
        }
        .padding(.leading, DiraigentTheme.spacingXS)
    }

    // MARK: - Input Area

    private var inputArea: some View {
        VStack(spacing: DiraigentTheme.spacingSM) {
            if let error = appState.chatService.error {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(DiraigentTheme.warning)
                    Text(error)
                        .font(DiraigentTheme.captionFont)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button("Dismiss") {
                        appState.chatService.error = nil
                    }
                    .font(DiraigentTheme.captionFont)
                }
                .padding(.horizontal, DiraigentTheme.spacingLG)
            }

            HStack(alignment: .bottom, spacing: DiraigentTheme.spacingSM) {
                TextField("Message...", text: $inputText, axis: .vertical)
                    .lineLimit(1...6)
                    .textFieldStyle(.plain)
                    .padding(.horizontal, DiraigentTheme.spacingMD)
                    .padding(.vertical, DiraigentTheme.spacingSM)
                    .background(
                        RoundedRectangle(cornerRadius: 20)
                            .fill(Color(.tertiarySystemBackground))
                    )
                    .focused($isInputFocused)

                sendButton
            }
            .padding(.horizontal, DiraigentTheme.spacingLG)
            .padding(.vertical, DiraigentTheme.spacingSM)
        }
        .background(Color(.secondarySystemBackground))
    }

    @ViewBuilder
    private var sendButton: some View {
        if appState.chatService.isStreaming {
            Button {
                appState.chatService.cancelStreaming()
            } label: {
                Image(systemName: "stop.circle.fill")
                    .font(.title2)
                    .foregroundStyle(DiraigentTheme.error)
            }
        } else {
            Button {
                sendCurrentMessage()
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.title2)
                    .foregroundStyle(canSend ? DiraigentTheme.primary : .gray)
            }
            .disabled(!canSend)
        }
    }

    private var canSend: Bool {
        !inputText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !appState.chatService.isStreaming
            && appState.selectedProjectId != nil
    }

    private func sendCurrentMessage() {
        let text = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, let projectId = appState.selectedProjectId else { return }
        inputText = ""
        appState.chatService.sendMessage(text, projectId: projectId)
    }
}

// MARK: - Chat Bubble

/// A styled message bubble for user or assistant messages.
private struct ChatBubble: View {
    let message: ChatMessage

    private var isUser: Bool { message.role == .user }

    var body: some View {
        HStack {
            if isUser { Spacer(minLength: 60) }

            VStack(alignment: isUser ? .trailing : .leading, spacing: DiraigentTheme.spacingXS) {
                Text(message.content)
                    .font(DiraigentTheme.bodyFont)
                    .foregroundStyle(isUser ? .white : .primary)
                    .textSelection(.enabled)

                Text(message.timestamp, style: .time)
                    .font(.caption2)
                    .foregroundStyle(isUser ? .white.opacity(0.7) : .secondary)
            }
            .padding(.horizontal, DiraigentTheme.spacingMD)
            .padding(.vertical, DiraigentTheme.spacingSM)
            .background(
                RoundedRectangle(cornerRadius: 16)
                    .fill(isUser ? DiraigentTheme.primary : Color(.tertiarySystemBackground))
            )

            if !isUser { Spacer(minLength: 60) }
        }
    }
}
