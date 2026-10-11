import SwiftUI

private struct WriteAccess: ViewModifier {
    @Environment(AppState.self) private var appState

    func body(content: Content) -> some View {
        if appState.accountAccess.canWrite { content }
    }
}

extension View {
    /// Hide mutation controls while preserving navigation and read views.
    func requiresWriteAccess() -> some View { modifier(WriteAccess()) }
}
