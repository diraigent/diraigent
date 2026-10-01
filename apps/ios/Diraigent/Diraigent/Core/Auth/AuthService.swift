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
            scopes: String = "openid profile email offline_access"
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

    private let session: URLSession
    private let tokens: AuthTokenStore
    private var generation = UUID()
    private var refreshTask: Task<String, Error>?

    private var accessToken: String? {
        get { tokens.read("access_token") }
        set { tokens.write("access_token", newValue) }
    }

    private var refreshToken: String? {
        get { tokens.read("refresh_token") }
        set { tokens.write("refresh_token", newValue) }
    }

    public convenience init(config: Config, apiClient: APIClient) {
        self.init(config: config, apiClient: apiClient, session: .shared, tokens: KeychainTokenStore())
    }

    init(config: Config, apiClient: APIClient, session: URLSession, tokens: AuthTokenStore) {
        self.config = config
        self.apiClient = apiClient
        self.session = session
        self.tokens = tokens
        isAuthenticated = accessToken != nil
    }

    private func configureClient() async {
        await apiClient.setAuthentication(
            token: { [weak self] in await self?.accessToken },
            refresh: { [weak self] rejected in
                guard let self else { throw APIError.unauthorized }
                return try await self.refresh(rejected: rejected)
            },
            invalidate: { [weak self] rejected in
                await self?.invalidate(rejected: rejected)
            }
        )
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

        // Invalidate any refresh belonging to the previous login.
        generation = UUID()
        refreshTask?.cancel()
        refreshTask = nil
        let loginGeneration = generation
        let response = try await requestTokens([
            "grant_type": "authorization_code", "code": code,
            "redirect_uri": config.redirectURI, "client_id": config.clientId,
            "code_verifier": codeVerifier,
        ])
        guard generation == loginGeneration else { throw CancellationError() }
        accessToken = response.accessToken
        refreshToken = response.refreshToken
        isAuthenticated = true
        await configureClient()
    }

    public func refreshAccessToken() async throws {
        _ = try await refresh(rejected: accessToken)
    }

    private func refresh(rejected: String?) async throws -> String {
        // A late 401 for an old token can use the already refreshed credential.
        if let token = accessToken, token != rejected { return token }
        if let refreshTask { return try await refreshTask.value }
        guard let refreshToken else {
            invalidate(rejected: rejected)
            throw APIError.unauthorized
        }
        let refreshGeneration = generation
        let task = Task { @MainActor in
            do {
                let response = try await self.requestTokens([
                    "grant_type": "refresh_token", "refresh_token": refreshToken,
                    "client_id": self.config.clientId,
                ])
                try Task.checkCancellation()
                guard self.generation == refreshGeneration else { throw CancellationError() }
                self.accessToken = response.accessToken
                if let rotated = response.refreshToken { self.refreshToken = rotated }
                self.isAuthenticated = true
                return response.accessToken
            } catch {
                if case APIError.unauthorized = error, self.generation == refreshGeneration {
                    self.logout()
                }
                throw error
            }
        }
        refreshTask = task
        defer { if generation == refreshGeneration { refreshTask = nil } }
        return try await task.value
    }

    private func invalidate(rejected: String?) {
        guard accessToken == rejected else { return }
        logout()
    }

    public func logout() {
        generation = UUID()
        refreshTask?.cancel()
        refreshTask = nil
        accessToken = nil
        refreshToken = nil
        currentUser = nil
        isAuthenticated = false
    }

    public func restoreSession() async {
        await configureClient()
        guard accessToken != nil, refreshToken != nil else { return }
        do {
            try await refreshAccessToken()
        } catch {
            // Only a definitive OAuth rejection clears credentials (in refresh).
            // Offline launches and temporary provider failures retain the session.
        }
    }

    private func requestTokens(_ fields: [String: String]) async throws -> TokenResponse {
        guard let url = URL(string: config.tokenURL) else { throw APIError.invalidURL }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")
        request.httpBody = fields.sorted { $0.key < $1.key }.map {
            "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: allowed)!)"
        }.joined(separator: "&").data(using: .utf8)
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw APIError.serverError(0, "Invalid token response") }
        guard (200...299).contains(http.statusCode) else {
            let oauth = try? JSONDecoder().decode(OAuthError.self, from: data)
            if http.statusCode == 400 && oauth?.error == "invalid_grant" {
                throw APIError.unauthorized
            }
            throw APIError.serverError(http.statusCode, "Authentication service unavailable")
        }
        return try JSONDecoder().decode(TokenResponse.self, from: data)
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

private struct OAuthError: Decodable { let error: String }

@MainActor
protocol AuthTokenStore {
    func read(_ key: String) -> String?
    func write(_ key: String, _ value: String?)
}

@MainActor
private struct KeychainTokenStore: AuthTokenStore {
    func read(_ key: String) -> String? { KeychainHelper.readString(key: key) }
    func write(_ key: String, _ value: String?) {
        if let value { KeychainHelper.saveString(key: key, value: value) }
        else { KeychainHelper.delete(key: key) }
    }
}
