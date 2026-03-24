import Foundation

/// App configuration per environment.
struct AppConfig: Sendable {
    let apiBaseURL: String
    let authIssuer: String
    let authClientId: String
    let authRedirectURI: String

    /// Development config (localhost).
    static let development = AppConfig(
        apiBaseURL: "http://localhost:3000/v1",
        authIssuer: "https://auth.faua.at/application/o/diraigent/",
        authClientId: "PLACEHOLDER_CLIENT_ID",
        authRedirectURI: "diraigent://auth/callback"
    )

    /// Production config.
    static let production = AppConfig(
        apiBaseURL: "https://api.diraigent.dev/v1",
        authIssuer: "https://auth.faua.at/application/o/diraigent/",
        authClientId: "PLACEHOLDER_CLIENT_ID",
        authRedirectURI: "diraigent://auth/callback"
    )

    /// Active configuration — toggle to `development` for localhost testing.
    #if DEBUG
    static let current = development
    #else
    static let current = production
    #endif
}
