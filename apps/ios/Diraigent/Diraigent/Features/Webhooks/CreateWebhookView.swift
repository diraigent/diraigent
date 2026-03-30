import SwiftUI

/// Form for creating a new webhook.
struct CreateWebhookView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var url = ""
    @State private var secret = ""
    @State private var selectedEvents: Set<String> = []
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private static let availableEvents = [
        "task.created",
        "task.updated",
        "task.transitioned",
        "task.completed",
        "task.commented",
        "work.created",
        "work.updated",
        "decision.created",
        "decision.updated",
        "observation.created",
        "knowledge.created",
        "verification.created",
    ]

    var body: some View {
        NavigationStack {
            Form {
                Section("Webhook Info") {
                    TextField("Name", text: $name)

                    TextField("URL", text: $url)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)

                    TextField("Secret (optional)", text: $secret)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                Section("Event Subscriptions") {
                    ForEach(Self.availableEvents, id: \.self) { event in
                        Toggle(event, isOn: Binding(
                            get: { selectedEvents.contains(event) },
                            set: { isOn in
                                if isOn {
                                    selectedEvents.insert(event)
                                } else {
                                    selectedEvents.remove(event)
                                }
                            }
                        ))
                        .font(DiraigentTheme.bodyFont)
                    }

                    Button("Select All") {
                        selectedEvents = Set(Self.availableEvents)
                    }
                    .disabled(selectedEvents.count == Self.availableEvents.count)
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                            .font(DiraigentTheme.captionFont)
                    }
                }
            }
            .navigationTitle("New Webhook")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await createWebhook() }
                    }
                    .disabled(
                        url.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || selectedEvents.isEmpty
                        || isSubmitting
                    )
                }
            }
        }
    }

    private func createWebhook() async {
        guard let projectId = appState.selectedProjectId else {
            errorMessage = "No project selected."
            return
        }

        let trimmedUrl = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedUrl.isEmpty else {
            errorMessage = "URL is required."
            return
        }

        guard !selectedEvents.isEmpty else {
            errorMessage = "Select at least one event."
            return
        }

        isSubmitting = true
        errorMessage = nil

        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedSecret = secret.trimmingCharacters(in: .whitespacesAndNewlines)

        let request = CreateWebhookRequest(
            name: trimmedName.isEmpty ? trimmedUrl : trimmedName,
            url: trimmedUrl,
            secret: trimmedSecret.isEmpty ? nil : trimmedSecret,
            events: Array(selectedEvents).sorted()
        )

        let result = await appState.webhooksService.createWebhook(
            projectId: projectId,
            request: request
        )

        isSubmitting = false

        if result != nil {
            dismiss()
        } else {
            errorMessage = appState.webhooksService.error ?? "Failed to create webhook."
        }
    }
}
