import SwiftUI

/// Sheet for creating a new task.
struct CreateTaskSheet: View {
    let projectId: UUID
    let tasksService: TasksService
    @Binding var isPresented: Bool

    @State private var title = ""
    @State private var kind = "feature"
    @State private var priority: Double = 5
    @State private var urgent = false
    @State private var spec = ""
    @State private var filesText = ""
    @State private var testCmd = ""
    @State private var acceptanceCriteria: [String] = []
    @State private var newCriterion = ""
    @State private var isSubmitting = false

    private let kinds = ["feature", "bug", "chore", "research", "spike", "refactor", "docs", "test"]

    var body: some View {
        NavigationStack {
            Form {
                // Title
                Section("Title") {
                    TextField("Task title", text: $title)
                }

                // Kind & Priority
                Section("Classification") {
                    Picker("Kind", selection: $kind) {
                        ForEach(kinds, id: \.self) { k in
                            Text(k.capitalized).tag(k)
                        }
                    }

                    VStack(alignment: .leading) {
                        Text("Priority: \(Int(priority))")
                        Slider(value: $priority, in: 1...10, step: 1)
                    }

                    Toggle("Urgent", isOn: $urgent)
                }

                // Spec
                Section("Specification") {
                    TextEditor(text: $spec)
                        .frame(minHeight: 100)
                }

                // Files
                Section("Files") {
                    TextField("Comma-separated file paths", text: $filesText)
                        .font(.callout.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                // Test Command
                Section("Test Command") {
                    TextField("e.g. npm test", text: $testCmd)
                        .font(.callout.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                // Acceptance Criteria
                Section("Acceptance Criteria") {
                    ForEach(Array(acceptanceCriteria.enumerated()), id: \.offset) { idx, criterion in
                        HStack {
                            Text(criterion)
                                .font(.callout)
                            Spacer()
                            Button {
                                acceptanceCriteria.remove(at: idx)
                            } label: {
                                Image(systemName: "minus.circle.fill")
                                    .foregroundStyle(.red)
                            }
                            .buttonStyle(.plain)
                        }
                    }

                    HStack {
                        TextField("Add criterion", text: $newCriterion)
                        Button {
                            let trimmed = newCriterion.trimmingCharacters(in: .whitespaces)
                            if !trimmed.isEmpty {
                                acceptanceCriteria.append(trimmed)
                                newCriterion = ""
                            }
                        } label: {
                            Image(systemName: "plus.circle.fill")
                        }
                        .disabled(newCriterion.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
            }
            .navigationTitle("New Task")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { isPresented = false }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await submit() }
                    }
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty || isSubmitting)
                }
            }
        }
    }

    private func submit() async {
        isSubmitting = true
        let files = filesText
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        var context: [String: AnyCodable] = [:]
        if !spec.isEmpty { context["spec"] = AnyCodable(spec) }
        if !files.isEmpty { context["files"] = AnyCodable(files) }
        if !testCmd.isEmpty { context["test_cmd"] = AnyCodable(testCmd) }
        if !acceptanceCriteria.isEmpty { context["acceptance_criteria"] = AnyCodable(acceptanceCriteria) }

        let request = CreateTaskRequest(
            title: title.trimmingCharacters(in: .whitespaces),
            kind: kind,
            urgent: urgent,
            context: context.isEmpty ? nil : context,
            workId: nil
        )

        let _ = await tasksService.createTask(projectId: projectId, request: request)
        isSubmitting = false
        isPresented = false
    }
}
