import SwiftUI

/// Reusable colored badge for severity levels.
struct SeverityBadge: View {
    let severity: String

    private var color: Color {
        switch severity.lowercased() {
        case "critical": .red
        case "high": .orange
        case "medium": .yellow
        case "low": .green
        case "info": .blue
        default: .secondary
        }
    }

    private var displayText: String {
        severity.lowercased()
    }

    var body: some View {
        Text(displayText)
            .font(.caption2.weight(.semibold))
            .textCase(.uppercase)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
}

#Preview {
    HStack {
        SeverityBadge(severity: "critical")
        SeverityBadge(severity: "high")
        SeverityBadge(severity: "medium")
        SeverityBadge(severity: "low")
        SeverityBadge(severity: "info")
    }
    .padding()
}
