import SwiftUI

/// Form for creating a new integration.
struct CreateIntegrationView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var kind = "ci"
    @State private var provider = ""
    @State private var baseUrl = ""
    @State private var authType = "none"
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private static let kinds = ["ci", "monitoring", "logging", "vcs", "chat", "custom"]
    private static let authTypes = ["none", "token", "basic", "oauth2"]

    var body: some View {
        NavigationStack {
            Form {
                Section("Integration Info") {
                    TextField("Name", text: $name)

                    Picker("Kind", selection: $kind) {
                        ForEach(Self.kinds, id: \.self) { k in
                            Label(k.capitalized, systemImage: kindIcon(k))
                                .tag(k)
                        }
                    }

                    TextField("Provider", text: $provider)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                Section("Connection") {
                    TextField("Base URL", text: $baseUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)

                    Picker("Auth Type", selection: $authType) {
                        ForEach(Self.authTypes, id: \.self) { t in
                            Text(t.capitalized)
                                .tag(t)
                        }
                    }
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                            .font(DiraigentTheme.captionFont)
                    }
                }
            }
            .navigationTitle("New Integration")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await createIntegration() }
                    }
                    .disabled(
                        name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || provider.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || isSubmitting
                    )
                }
            }
        }
    }

    private func createIntegration() async {
        guard let projectId = appState.selectedProjectId else {
            errorMessage = "No project selected."
            return
        }

        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else {
            errorMessage = "Name is required."
            return
        }

        let trimmedProvider = provider.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedProvider.isEmpty else {
            errorMessage = "Provider is required."
            return
        }

        isSubmitting = true
        errorMessage = nil

        let trimmedBaseUrl = baseUrl.trimmingCharacters(in: .whitespacesAndNewlines)

        let request = CreateIntegrationRequest(
            name: trimmedName,
            kind: kind,
            provider: trimmedProvider,
            baseUrl: trimmedBaseUrl.isEmpty ? nil : trimmedBaseUrl,
            authType: authType
        )

        let result = await appState.integrationsService.createIntegration(
            projectId: projectId,
            request: request
        )

        isSubmitting = false

        if result != nil {
            dismiss()
        } else {
            errorMessage = appState.integrationsService.error ?? "Failed to create integration."
        }
    }

    private func kindIcon(_ kind: String) -> String {
        switch kind.lowercased() {
        case "ci": "gearshape.2"
        case "monitoring": "chart.bar"
        case "logging": "doc.text"
        case "vcs": "arrow.triangle.branch"
        case "chat": "bubble.left.and.bubble.right"
        case "custom": "puzzlepiece.extension"
        default: "puzzlepiece.extension"
        }
    }
}
