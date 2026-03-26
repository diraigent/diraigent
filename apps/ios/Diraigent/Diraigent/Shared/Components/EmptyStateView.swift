import SwiftUI

/// Reusable empty state view with an SF Symbol icon, title, subtitle, and optional action button.
struct EmptyStateView: View {
    let icon: String
    let title: String
    let subtitle: String?
    let actionTitle: String?
    let action: (() -> Void)?

    init(
        icon: String,
        title: String,
        subtitle: String? = nil,
        actionTitle: String? = nil,
        action: (() -> Void)? = nil
    ) {
        self.icon = icon
        self.title = title
        self.subtitle = subtitle
        self.actionTitle = actionTitle
        self.action = action
    }

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: icon)
        } description: {
            if let subtitle {
                Text(subtitle)
            }
        } actions: {
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .buttonStyle(.bordered)
            }
        }
    }
}

#Preview {
    EmptyStateView(
        icon: "tray",
        title: "No Items",
        subtitle: "Nothing to show here yet.",
        actionTitle: "Refresh"
    ) {
        print("refresh")
    }
}
