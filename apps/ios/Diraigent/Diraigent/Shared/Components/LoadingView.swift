import SwiftUI

/// Reusable centered loading indicator with an optional message.
struct LoadingView: View {
    let message: String?

    init(_ message: String? = nil) {
        self.message = message
    }

    var body: some View {
        VStack(spacing: DiraigentTheme.spacingMD) {
            ProgressView()
                .controlSize(.large)
            if let message {
                Text(message)
                    .font(DiraigentTheme.captionFont)
                    .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    LoadingView("Loading data...")
}
