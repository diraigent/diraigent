import SwiftUI

/// Sheet form for creating a new work item.
struct CreateWorkView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var title = ""
    @State private var workType = "feature"
    @State private var status = "active"
    @State private var priority = 5
    @State private var description = ""
    @State private var isCreating = false
    @State private var errorMessage: String?

    private static let workTypes = ["epic", "feature", "milestone", "sprint", "initiative"]
    private static let statuses = ["active", "achieved", "paused", "abandoned"]

    private var workService: WorkService { appState.workService }

    private var isValid: Bool {
        !title.trimmingCharacters(in: .whitespaces).isEmpty
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Details") {
                    TextField("Title", text: $title)

                    Picker("Type", selection: $workType) {
                        ForEach(Self.workTypes, id: \.self) { type in
                            Text(type.capitalized).tag(type)
                        }
                    }

                    Picker("Status", selection: $status) {
                        ForEach(Self.statuses, id: \.self) { s in
                            Text(s.capitalized).tag(s)
                        }
                    }

                    Stepper("Priority: \(priority)", value: $priority, in: 1...10)
                }

                Section("Description") {
                    TextEditor(text: $description)
                        .frame(minHeight: 100)
                }

                if let errorMessage {
                    Section {
                        Label(errorMessage, systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(DiraigentTheme.error)
                            .font(DiraigentTheme.captionFont)
                    }
                }
            }
            .navigationTitle("New Work Item")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await createItem() }
                    }
                    .disabled(!isValid || isCreating)
                }
            }
            .interactiveDismissDisabled(isCreating)
        }
    }

    private func createItem() async {
        guard let projectId = appState.selectedProjectId else { return }
        isCreating = true
        errorMessage = nil

        let request = CreateWorkRequest(
            title: title.trimmingCharacters(in: .whitespaces),
            workType: workType,
            status: status,
            priority: priority,
            description: description.isEmpty ? nil : description
        )

        let result = await workService.createWork(projectId: projectId, request: request)
        isCreating = false

        if result != nil {
            dismiss()
        } else {
            errorMessage = workService.error ?? "Failed to create work item."
        }
    }
}
