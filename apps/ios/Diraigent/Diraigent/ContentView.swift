import SwiftUI

/// Root view — placeholder until auth and features are wired.
struct ContentView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Image(systemName: "cpu")
                    .font(.system(size: 64))
                    .foregroundStyle(.tint)

                Text("Diraigent")
                    .font(.largeTitle.bold())

                Text("AI Agent Orchestration")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .navigationTitle("Diraigent")
        }
    }
}

#Preview {
    ContentView()
        .environment(AppState())
}
