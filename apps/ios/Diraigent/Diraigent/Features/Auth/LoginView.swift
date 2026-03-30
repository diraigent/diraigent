import SwiftUI
import AuthenticationServices

/// Login screen with OIDC authentication via Authentik.
struct LoginView: View {
    @Environment(AppState.self) private var appState
    @State private var errorMessage: String?
    @State private var isAuthenticating = false

    var body: some View {
        VStack(spacing: 32) {
            Spacer()

            // Logo & title
            VStack(spacing: 16) {
                Image(systemName: "cpu")
                    .font(.system(size: 80))
                    .foregroundStyle(.tint)

                Text("Diraigent")
                    .font(.largeTitle.bold())

                Text("AI Agent Orchestration")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Login button
            VStack(spacing: 16) {
                if appState.authService.isLoading || isAuthenticating {
                    ProgressView("Signing in\u{2026}")
                } else {
                    Button {
                        startAuth()
                    } label: {
                        Label("Sign in with Diraigent", systemImage: "person.circle.fill")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(.blue)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 12))
                    }
                }

                if let error = errorMessage {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
            .padding(.horizontal, 32)

            Spacer()
        }
    }

    // MARK: - Auth Flow

    private func startAuth() {
        let auth = appState.authService
        let codeVerifier = PKCEHelper.generateCodeVerifier()

        guard let url = auth.authorizationURL(codeVerifier: codeVerifier) else {
            errorMessage = "Failed to build authorization URL"
            return
        }

        isAuthenticating = true
        errorMessage = nil

        Task {
            do {
                let callbackURL = try await WebAuthSessionController.shared.authenticate(
                    url: url,
                    callbackURLScheme: "diraigent"
                )

                guard let components = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false),
                      let code = components.queryItems?.first(where: { $0.name == "code" })?.value
                else {
                    errorMessage = "Invalid callback URL"
                    isAuthenticating = false
                    return
                }

                try await auth.exchangeCode(code, codeVerifier: codeVerifier)
            } catch let error as ASWebAuthenticationSessionError where error.code == .canceledLogin {
                // User cancelled — do nothing
            } catch {
                errorMessage = error.localizedDescription
            }
            isAuthenticating = false
        }
    }
}

// MARK: - ASWebAuthenticationSession async wrapper

/// Wraps `ASWebAuthenticationSession` for async/await usage on iOS 17.0+.
@MainActor
private final class WebAuthSessionController: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = WebAuthSessionController()

    /// Retained session to prevent deallocation while the browser is open.
    private var activeSession: ASWebAuthenticationSession?

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                  let window = scene.windows.first
            else {
                return ASPresentationAnchor()
            }
            return window
        }
    }

    func authenticate(url: URL, callbackURLScheme: String) async throws -> URL {
        defer { activeSession = nil }

        return try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: callbackURLScheme
            ) { callbackURL, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let url = callbackURL {
                    continuation.resume(returning: url)
                } else {
                    continuation.resume(throwing: URLError(.badURL))
                }
            }
            activeSession = session
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            session.start()
        }
    }
}
