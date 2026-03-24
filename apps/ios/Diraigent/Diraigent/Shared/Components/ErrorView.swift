import SwiftUI

/// Reusable error display with an error icon, message, and retry button.
struct ErrorView: View {
    let message: String
    let retryAction: (() -> Void)?

    init(_ message: String, retryAction: (() -> Void)? = nil) {
        self.message = message
        self.retryAction = retryAction
    }

    var body: some View {
        ContentUnavailableView {
            Label("Error", systemImage: "exclamationmark.triangle")
        } description: {
            Text(message)
        } actions: {
            if let retryAction {
                Button("Retry", action: retryAction)
                    .buttonStyle(.bordered)
            }
        }
    }
}

#Preview {
    ErrorView("Something went wrong. Please try again.") {
        print("retry")
    }
}
