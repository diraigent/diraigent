import SwiftUI

/// Form for creating a new task.
struct CreateTaskView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var title = ""
    @State private var kind = "feature"
    @State private var spec = ""
    @State private var urgent = false
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private static let kinds = ["feature", "bug", "refactor", "docs", "test"]

    var body: some View {
        NavigationStack {
            Form {
                Section("Task Info") {
                    TextField("Title", text: $title)

                    Picker("Kind", selection: $kind) {
                        ForEach(Self.kinds, id: \.self) { k in
                            Label(k.capitalized, systemImage: kindIcon(k))
                                .tag(k)
                        }
                    }

                    Toggle(isOn: $urgent) {
                        Label("Urgent", systemImage: "exclamationmark.triangle.fill")
                    }
                    .tint(.orange)
                }

                Section("Spec") {
                    TextEditor(text: $spec)
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
            .navigationTitle("New Task")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await createTask() }
                    }
                    .disabled(title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSubmitting)
                }
            }
        }
    }

    private func createTask() async {
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

        let trimmedSpec = spec.trimmingCharacters(in: .whitespacesAndNewlines)
        let context: [String: AnyCodable]? = trimmedSpec.isEmpty
            ? nil
            : ["spec": AnyCodable(trimmedSpec)]

        let request = CreateTaskRequest(
            title: trimmedTitle,
            kind: kind,
            urgent: urgent ? true : nil,
            context: context,
            workId: nil
        )

        let result = await appState.tasksService.createTask(projectId: projectId, request: request)

        isSubmitting = false

        if result != nil {
            dismiss()
        } else {
            errorMessage = appState.tasksService.error ?? "Failed to create task."
        }
    }

    private func kindIcon(_ kind: String) -> String {
        switch kind.lowercased() {
        case "feature": "star.fill"
        case "bug": "ladybug.fill"
        case "refactor": "arrow.triangle.2.circlepath"
        case "docs": "doc.text"
        case "test": "checkmark.shield"
        default: "circle.fill"
        }
    }
}
