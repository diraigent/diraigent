import SwiftUI

/// Form for creating a new report.
struct CreateReportView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var title = ""
    @State private var kind = "architecture"
    @State private var prompt = ""
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private static let kinds = ["security", "component", "architecture", "performance", "custom"]

    var body: some View {
        NavigationStack {
            Form {
                Section("Report Info") {
                    TextField("Title", text: $title)

                    Picker("Kind", selection: $kind) {
                        ForEach(Self.kinds, id: \.self) { k in
                            Label(k.capitalized, systemImage: kindIcon(k))
                                .tag(k)
                        }
                    }
                }

                Section("Prompt") {
                    TextEditor(text: $prompt)
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
            .navigationTitle("New Report")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await createReport() }
                    }
                    .disabled(
                        title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || isSubmitting
                    )
                }
            }
        }
    }

    private func createReport() async {
        guard let projectId = appState.selectedProjectId else {
            errorMessage = "No project selected."
            return
        }

        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTitle.isEmpty else {
            errorMessage = "Title is required."
            return
        }

        let trimmedPrompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedPrompt.isEmpty else {
            errorMessage = "Prompt is required."
            return
        }

        isSubmitting = true
        errorMessage = nil

        let request = CreateReportRequest(
            title: trimmedTitle,
            kind: kind,
            prompt: trimmedPrompt
        )

        let result = await appState.reportsService.createReport(projectId: projectId, request: request)

        isSubmitting = false

        if result != nil {
            dismiss()
        } else {
            errorMessage = appState.reportsService.error ?? "Failed to create report."
        }
    }

    private func kindIcon(_ kind: String) -> String {
        switch kind.lowercased() {
        case "security": "lock.shield"
        case "component": "square.stack.3d.up"
        case "architecture": "building.columns"
        case "performance": "gauge.with.dots.needle.33percent"
        case "custom": "doc.text"
        default: "doc.text"
        }
    }
}
