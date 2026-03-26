import SwiftUI

/// Maps observation kinds to SF Symbols.
struct KindIcon: View {
    let kind: String

    private var systemName: String {
        switch kind.lowercased() {
        case "insight": "lightbulb"
        case "risk": "exclamationmark.triangle"
        case "smell": "ant"
        case "improvement": "arrow.up.circle"
        default: "questionmark.circle"
        }
    }

    private var color: Color {
        switch kind.lowercased() {
        case "insight": .blue
        case "risk": .red
        case "smell": .orange
        case "improvement": .green
        default: .secondary
        }
    }

    var body: some View {
        Image(systemName: systemName)
            .foregroundStyle(color)
    }
}

#Preview {
    HStack(spacing: 16) {
        KindIcon(kind: "insight")
        KindIcon(kind: "risk")
        KindIcon(kind: "smell")
        KindIcon(kind: "improvement")
    }
    .font(.title2)
    .padding()
}
