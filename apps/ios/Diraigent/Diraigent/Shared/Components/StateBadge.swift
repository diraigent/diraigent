import SwiftUI

/// Reusable colored badge for task states.
struct StateBadge: View {
    let state: String

    var body: some View {
        Text(state)
            .font(.caption2.bold())
            .textCase(.uppercase)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }

    private var color: Color {
        switch state.lowercased() {
        case "backlog":
            return .gray
        case "ready":
            return .blue
        case "working":
            return .orange
        case "done":
            return .green
        case "cancelled":
            return .red
        case "human_review":
            return .purple
        default:
            return .orange
        }
    }
}

#Preview {
    VStack(spacing: 8) {
        StateBadge(state: "backlog")
        StateBadge(state: "ready")
        StateBadge(state: "working")
        StateBadge(state: "done")
        StateBadge(state: "cancelled")
        StateBadge(state: "human_review")
    }
    .padding()
}
