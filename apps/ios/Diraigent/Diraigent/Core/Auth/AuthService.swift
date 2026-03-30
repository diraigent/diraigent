import Foundation

/// OIDC / PKCE authentication service for Authentik.
///
/// Manages the full authentication lifecycle: authorization URL generation,
/// code exchange, token refresh, and logout. Tokens are persisted in the
/// Keychain via ``KeychainHelper``.
@Observable
@MainActor
public final class AuthService {

    // MARK: - Configuration

    public struct Config: Sendable {
        public let issuer: String
        public let authBase: String
        public let clientId: String
        public let redirectURI: String
        public let scopes: String

        public init(
            issuer: String,
            clientId: String,
            redirectURI: String,
            scopes: String = "openid profile email"
        ) {
            self.issuer = issuer
            // Authentik's authorize/token endpoints live at /application/o/
            // not under the per-app issuer path /application/o/{slug}/
            let base = issuer
                .replacingOccurrences(of: "/application/o/diraigent/", with: "/application/o/")
            self.authBase = base.hasSuffix("/") ? base : base + "/"
            self.clientId = clientId
            self.redirectURI = redirectURI
            self.scopes = scopes
        }

        var authorizeURL: String { "\(authBase)authorize/" }
        var tokenURL: String { "\(authBase)token/" }
        var userinfoURL: String { "\(authBase)userinfo/" }
    }

    // MARK: - Public state

    /// Whether the user is currently authenticated (has a valid access token).
    public private(set) var isAuthenticated = false

    /// Whether an authentication operation is in progress.
    public private(set) var isLoading = false

    /// The current user info, if available.
    public private(set) var currentUser: UserInfo?

    // MARK: - Private

    private let config: Config
    private let apiClient: APIClient

    private var accessToken: String? {
        get { KeychainHelper.readString(key: "access_token") }
        set {
            if let value = newValue {
                KeychainHelper.saveString(key: "access_token", value: value)
            } else {
                KeychainHelper.delete(key: "access_token")
            }
        }
    }

    private var refreshToken: String? {
        get { KeychainHelper.readString(key: "refresh_token") }
        set {
            if let value = newValue {
                KeychainHelper.saveString(key: "refresh_token", value: value)
            } else {
                KeychainHelper.delete(key: "refresh_token")
            }
        }
    }

    // MARK: - Init

    public init(config: Config, apiClient: APIClient) {
        self.config = config
        self.apiClient = apiClient

        // Restore session if we have a stored token
        if let token = accessToken {
            isAuthenticated = true
            Task {
                await apiClient.setToken(token)
            }
        }
    }

    // MARK: - Auth Flow

    /// Build the OIDC authorization URL for web-based login with PKCE.
    public func authorizationURL(codeVerifier: String) -> URL? {
        let codeChallenge = PKCEHelper.codeChallenge(for: codeVerifier)

        var components = URLComponents(string: config.authorizeURL)
        components?.queryItems = [
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "client_id", value: config.clientId),
            URLQueryItem(name: "redirect_uri", value: config.redirectURI),
            URLQueryItem(name: "scope", value: config.scopes),
            URLQueryItem(name: "code_challenge", value: codeChallenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
        ]
        return components?.url
    }

    /// Exchange an authorization code for access and refresh tokens.
    public func exchangeCode(_ code: String, codeVerifier: String) async throws {
        isLoading = true
        defer { isLoading = false }

        guard let tokenURL = URL(string: config.tokenURL) else { return }

        var request = URLRequest(url: tokenURL)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")

        let body: [String: String] = [
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": config.redirectURI,
            "client_id": config.clientId,
            "code_verifier": codeVerifier,
        ]
        request.httpBody = body
            .map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? $0.value)" }
            .joined(separator: "&")
            .data(using: .utf8)

        let (data, _) = try await URLSession.shared.data(for: request)
        let tokenResponse = try JSONDecoder().decode(TokenResponse.self, from: data)

        accessToken = tokenResponse.accessToken
        refreshToken = tokenResponse.refreshToken
        isAuthenticated = true

        await apiClient.setToken(tokenResponse.accessToken)
    }

    /// Refresh the access token using the stored refresh token.
    public func refreshAccessToken() async throws {
        guard let refresh = refreshToken else {
            logout()
            return
        }

        guard let tokenURL = URL(string: config.tokenURL) else { return }

        var request = URLRequest(url: tokenURL)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")

        let body: [String: String] = [
            "grant_type": "refresh_token",
            "refresh_token": refresh,
            "client_id": config.clientId,
        ]
        request.httpBody = body
            .map { "\($0.key)=\($0.value)" }
            .joined(separator: "&")
            .data(using: .utf8)

        let (data, _) = try await URLSession.shared.data(for: request)
        let tokenResponse = try JSONDecoder().decode(TokenResponse.self, from: data)

        accessToken = tokenResponse.accessToken
        if let newRefresh = tokenResponse.refreshToken {
            refreshToken = newRefresh
        }

        await apiClient.setToken(tokenResponse.accessToken)
    }

    /// Clear all tokens and reset authentication state.
    public func logout() {
        accessToken = nil
        refreshToken = nil
        currentUser = nil
        isAuthenticated = false
        Task {
            await apiClient.setToken(nil)
        }
    }

    /// Attempt to restore session on launch by refreshing the token.
    public func restoreSession() async {
        guard accessToken != nil else { return }
        do {
            try await refreshAccessToken()
        } catch {
            // Refresh failed — session is stale, log out
            logout()
        }
    }
}

// MARK: - Token Response

private struct TokenResponse: Codable, Sendable {
    let accessToken: String
    let tokenType: String?
    let expiresIn: Int?
    let refreshToken: String?
    let scope: String?

    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case tokenType = "token_type"
        case expiresIn = "expires_in"
        case refreshToken = "refresh_token"
        case scope
    }
}

// MARK: - User Info

/// Basic user information from the OIDC provider.
public struct UserInfo: Codable, Sendable {
    public let sub: String
    public let email: String?
    public let name: String?
    public let preferredUsername: String?

    enum CodingKeys: String, CodingKey {
        case sub
        case email
        case name
        case preferredUsername = "preferred_username"
    }
}
