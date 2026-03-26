import SwiftUI

/// Design tokens for the Diraigent app.
enum DiraigentTheme {

    // MARK: - Color Palette

    static let primary = Color.blue
    static let secondary = Color.indigo
    static let success = Color.green
    static let warning = Color.orange
    static let error = Color.red
    static let background = Color(.systemBackground)
    static let surface = Color(.secondarySystemBackground)
    static let surfaceGrouped = Color(.systemGroupedBackground)

    // MARK: - Task State Colors

    static func taskStateColor(_ state: String) -> Color {
        switch state {
        case "ready": .blue
        case "backlog": .gray
        case "done": .green
        case "cancelled": .red
        case "human_review": .purple
        default: .orange // working/implement/review/etc.
        }
    }

    // MARK: - Agent Status Colors

    static func agentStatusColor(_ status: String?) -> Color {
        switch status {
        case "idle": .green
        case "working": .orange
        case "offline": .gray
        default: .gray
        }
    }

    // MARK: - Severity Colors

    static func severityColor(_ severity: String?) -> Color {
        switch severity {
        case "critical", "high": .red
        case "medium": .orange
        case "low": .blue
        default: .secondary
        }
    }

    // MARK: - Typography

    static let titleFont: Font = .title.bold()
    static let headlineFont: Font = .headline
    static let bodyFont: Font = .body
    static let captionFont: Font = .caption

    // MARK: - Spacing

    static let spacingXS: CGFloat = 4
    static let spacingSM: CGFloat = 8
    static let spacingMD: CGFloat = 12
    static let spacingLG: CGFloat = 16
    static let spacingXL: CGFloat = 24
}
