import SwiftUI

/// Form for creating a new event.
struct CreateEventView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var title = ""
    @State private var kind = "custom"
    @State private var severity = "info"
    @State private var description = ""
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private static let kinds = ["ci", "deploy", "error", "merge", "release", "alert", "custom"]
    private static let severities = ["info", "warning", "error", "critical"]

    var body: some View {
        NavigationStack {
            Form {
                Section("Event Info") {
                    TextField("Title", text: $title)

                    Picker("Kind", selection: $kind) {
                        ForEach(Self.kinds, id: \.self) { k in
                            Label(k.capitalized, systemImage: kindIcon(k))
                                .tag(k)
                        }
                    }

                    Picker("Severity", selection: $severity) {
                        ForEach(Self.severities, id: \.self) { s in
                            Label(s.capitalized, systemImage: severityIcon(s))
                                .tag(s)
                        }
                    }
                }

                Section("Description") {
                    TextEditor(text: $description)
                        .frame(minHeight: 120)
                        .font(DiraigentTheme.bodyFont)
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                            .font(DiraigentTheme.captionFont)
                    }
                }
            }
            .navigationTitle("New Event")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await createEvent() }
                    }
                    .disabled(title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSubmitting)
                }
            }
        }
    }

    private func createEvent() async {
        guard let projectId = appState.selectedProjectId else {
            errorMessage = "No project selected."
            return
        }

        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTitle.isEmpty else {
            errorMessage = "Title is required."
            return
        }

        isSubmitting = true
        errorMessage = nil

        let trimmedDescription = description.trimmingCharacters(in: .whitespacesAndNewlines)

        let request = CreateEventRequest(
            title: trimmedTitle,
            kind: kind,
            severity: severity,
            description: trimmedDescription.isEmpty ? nil : trimmedDescription
        )

        let result = await appState.eventsService.createEvent(projectId: projectId, request: request)

        isSubmitting = false

        if result != nil {
            dismiss()
        } else {
            errorMessage = appState.eventsService.error ?? "Failed to create event."
        }
    }

    private func kindIcon(_ kind: String) -> String {
        switch kind.lowercased() {
        case "ci": "gearshape.2"
        case "deploy": "arrow.up.circle"
        case "error": "exclamationmark.triangle"
        case "merge": "arrow.triangle.merge"
        case "release": "tag"
        case "alert": "bell.badge"
        case "custom": "circle.fill"
        default: "circle.fill"
        }
    }

    private func severityIcon(_ severity: String) -> String {
        switch severity.lowercased() {
        case "critical": "exclamationmark.octagon.fill"
        case "error": "xmark.circle.fill"
        case "warning": "exclamationmark.triangle.fill"
        case "info": "info.circle.fill"
        default: "circle.fill"
        }
    }
}
