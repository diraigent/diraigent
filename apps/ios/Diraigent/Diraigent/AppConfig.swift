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
        authIssuer: "http://localhost:9000/application/o/diraigent/",
        authClientId: "change-me",
        authRedirectURI: "diraigent://auth/callback"
    )

    /// Production defaults. Supply deployment-specific authentication locally.
    static let production = AppConfig(
        apiBaseURL: "https://api.diraigent.com/v1",
        authIssuer: "",
        authClientId: "",
        authRedirectURI: "diraigent://auth/callback"
    )

    /// LocalConfig.plist is bundled by Xcode but excluded from version control.
    static let current: AppConfig = {
        if let url = Bundle.main.url(forResource: "LocalConfig", withExtension: "plist"),
           let data = try? Data(contentsOf: url),
           let values = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: String],
           let apiBaseURL = values["apiBaseURL"], !apiBaseURL.isEmpty,
           let authIssuer = values["authIssuer"], !authIssuer.isEmpty,
           let authClientId = values["authClientId"], !authClientId.isEmpty,
           let authRedirectURI = values["authRedirectURI"], !authRedirectURI.isEmpty {
            return AppConfig(
                apiBaseURL: apiBaseURL,
                authIssuer: authIssuer,
                authClientId: authClientId,
                authRedirectURI: authRedirectURI
            )
        }
        #if DEBUG
        return development
        #else
        return production
        #endif
    }()
}
