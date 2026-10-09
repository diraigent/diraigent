import Foundation
import Testing
@testable import Diraigent

@MainActor
private final class MemoryTokens: AuthTokenStore {
    var values: [String: String] = ["access_token": "old"]
    func read(_ key: String) -> String? { values[key] }
    func write(_ key: String, _ value: String?) { values[key] = value }
}

private final class AuthURLProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var handler: (@Sendable (URLRequest) throws -> (Int, String))?
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        do {
            var normalized = request
            if normalized.httpBody == nil, let stream = normalized.httpBodyStream {
                stream.open()
                defer { stream.close() }
                var data = Data()
                var buffer = [UInt8](repeating: 0, count: 1024)
                while stream.hasBytesAvailable {
                    let count = stream.read(&buffer, maxLength: buffer.count)
                    guard count > 0 else { break }
                    data.append(contentsOf: buffer.prefix(count))
                }
                normalized.httpBody = data
            }
            let (status, body) = try Self.handler!(normalized)
            client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status,
                httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Data(body.utf8))
            client?.urlProtocolDidFinishLoading(self)
        } catch { client?.urlProtocol(self, didFailWithError: error) }
    }
    override func stopLoading() {}
}

private final class Requests: @unchecked Sendable {
    private let lock = NSLock()
    private var requests: [URLRequest] = []
    func add(_ request: URLRequest) { lock.withLock { requests.append(request) } }
    var all: [URLRequest] { lock.withLock { requests } }
}

@Suite(.serialized)
@MainActor
struct AuthenticationTests {
    private let config = AuthService.Config(issuer: "https://auth.example/application/o/diraigent/",
        clientId: "ios", redirectURI: "diraigent://auth/callback")

    private func fixture() -> (AuthService, APIClient, MemoryTokens) {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AuthURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let api = APIClient(baseURL: "https://api.example", session: session)
        let tokens = MemoryTokens()
        let auth = AuthService(config: config, apiClient: api, session: session, tokens: tokens)
        return (auth, api, tokens)
    }

    @Test func loginRequestsRefreshTokensAndEncodesFormFields() async throws {
        let requests = Requests()
        AuthURLProtocol.handler = { request in
            requests.add(request)
            return (200, #"{"access_token":"new","refresh_token":"rotated"}"#)
        }
        let (auth, _, tokens) = fixture()
        let url = auth.authorizationURL(codeVerifier: "verifier")!
        let scope = URLComponents(url: url, resolvingAgainstBaseURL: false)!.queryItems!.first { $0.name == "scope" }!.value!
        #expect(scope.contains("offline_access"))
        try await auth.exchangeCode("code&+=", codeVerifier: "verifier")
        #expect(tokens.values["refresh_token"] == "rotated")
        #expect(String(data: requests.all[0].httpBody!, encoding: .utf8)!.contains("code=code%26%2B%3D"))
    }

    @Test func concurrentUnauthorizedRequestsShareRefreshAndRetry() async throws {
        let requests = Requests()
        AuthURLProtocol.handler = { request in
            requests.add(request)
            if request.url!.path == "/application/o/token" {
                return (200, #"{"access_token":"new","refresh_token":"rotated"}"#)
            }
            return request.value(forHTTPHeaderField: "Authorization") == "Bearer new" ? (200, "{}") : (401, "{}")
        }
        let (auth, api, tokens) = fixture()
        await auth.restoreSession() // Configure without refreshing a legacy session.
        tokens.values["refresh_token"] = "refresh&+=token"
        try await withThrowingTaskGroup(of: Void.self) { group in
            for _ in 0..<12 {
                group.addTask { let _: EmptyResponse = try await api.get("/projects") }
            }
            try await group.waitForAll()
        }
        let refreshes = requests.all.filter { $0.url!.path == "/application/o/token" }
        #expect(refreshes.count == 1)
        #expect(String(data: refreshes[0].httpBody!, encoding: .utf8)!.contains("refresh_token=refresh%26%2B%3Dtoken"))
        #expect(auth.isAuthenticated)
        #expect(tokens.values["refresh_token"] == "rotated")
    }

    @Test func retryIsBoundedAndSecondUnauthorizedLogsOut() async {
        let requests = Requests()
        AuthURLProtocol.handler = { request in
            requests.add(request)
            return request.url!.path == "/application/o/token" ? (200, #"{"access_token":"new"}"#) : (401, "{}")
        }
        let (auth, api, tokens) = fixture()
        await auth.restoreSession()
        tokens.values["refresh_token"] = "refresh"
        do { let _: EmptyResponse = try await api.get("/projects"); Issue.record("Expected rejection") }
        catch { #expect(error as? APIError != nil) }
        #expect(requests.all.count == 3)
        #expect(!auth.isAuthenticated)
        #expect(tokens.values.isEmpty)
    }

    @Test(arguments: [400, 503, -1]) func refreshFailuresOnlyDiscardDefinitivelyExpiredSession(status: Int) async {
        AuthURLProtocol.handler = { _ in
            if status == -1 { throw URLError(.notConnectedToInternet) }
            return (status, #"{"error":"invalid_grant"}"#)
        }
        let (auth, _, tokens) = fixture()
        tokens.values["refresh_token"] = "refresh"
        await auth.restoreSession()
        #expect(auth.isAuthenticated == (status != 400))
        #expect((tokens.values["refresh_token"] != nil) == (status != 400))
    }

    @Test func expiredLegacySessionRequiresLogin() async {
        AuthURLProtocol.handler = { _ in (401, "{}") }
        let (auth, api, _) = fixture()
        await auth.restoreSession()
        #expect(auth.isAuthenticated)
        do { let _: EmptyResponse = try await api.get("/projects"); Issue.record("Expected rejection") }
        catch { }
        #expect(!auth.isAuthenticated)
    }

    @Test func logoutDuringRefreshCannotRestoreCredentials() async {
        let (auth, _, tokens) = fixture()
        tokens.values["refresh_token"] = "refresh"
        AuthURLProtocol.handler = { _ in
            let completed = DispatchSemaphore(value: 0)
            Task { @MainActor in
                auth.logout()
                completed.signal()
            }
            completed.wait()
            return (200, #"{"access_token":"late","refresh_token":"late"}"#)
        }
        await auth.restoreSession()
        #expect(!auth.isAuthenticated)
        #expect(tokens.values.isEmpty)
    }

    @Test func providerOutageDuringApiRefreshPreservesSessionWithoutReplay() async {
        let requests = Requests()
        AuthURLProtocol.handler = { request in
            requests.add(request)
            return request.url!.path == "/application/o/token" ? (503, "{}") : (401, "{}")
        }
        let (auth, api, tokens) = fixture()
        await auth.restoreSession()
        tokens.values["refresh_token"] = "refresh"
        do { let _: EmptyResponse = try await api.get("/projects"); Issue.record("Expected provider outage") }
        catch { }
        #expect(auth.isAuthenticated)
        #expect(tokens.values["refresh_token"] == "refresh")
        #expect(requests.all.count == 2)
    }

    @Test func orchestraHistoryFailureBlocksSendingAndRecoversWithoutLogout() async throws {
        AuthURLProtocol.handler = { _ in (503, #"{"error":"offline"}"#) }
        let (_, api, _) = fixture()
        let chat = ChatService(apiClient: api)
        let project = UUID()
        await chat.loadHistory(projectId: project)
        #expect(!chat.historyAvailable)
        chat.sendMessage("must not send", projectId: project)
        #expect(chat.messages.isEmpty)
        AuthURLProtocol.handler = { _ in (200, #"{"enabled":true,"revision":2,"busy":false,"messages":[{"role":"user","content":"saved"}]}"#) }
        await chat.loadHistory(projectId: project)
        #expect(chat.historyAvailable)
        #expect(chat.messages.map(\.content) == ["saved"])
        AuthURLProtocol.handler = { _ in (200, #"{"enabled":true,"revision":0,"busy":false,"messages":[]}"#) }
        await chat.loadHistory(projectId: UUID())
        #expect(chat.messages.isEmpty)
    }

    @Test func chatRefreshesBeforeOpeningStream() async throws {
        let requests = Requests()
        AuthURLProtocol.handler = { request in
            requests.add(request)
            if request.url!.path == "/application/o/token" { return (200, #"{"access_token":"new"}"#) }
            return request.value(forHTTPHeaderField: "Authorization") == "Bearer new" ? (200, "data: done\n\n") : (401, "{}")
        }
        let (auth, api, tokens) = fixture()
        await auth.restoreSession()
        tokens.values["refresh_token"] = "refresh"
        let bytes = try await api.stream("/chat", body: ["message": "hello"])
        var text = ""
        for try await line in bytes.lines { text += line }
        #expect(text == "data: done")
        #expect(requests.all.filter { $0.url!.path == "/chat" }.allSatisfy { $0.timeoutInterval == 660 })
        #expect(requests.all.count == 3)
        #expect(requests.all.filter { $0.url!.path == "/chat" }.allSatisfy { $0.httpBody == requests.all[0].httpBody })
        #expect(auth.isAuthenticated)
    }
}
