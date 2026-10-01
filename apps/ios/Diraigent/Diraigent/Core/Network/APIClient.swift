import Foundation
import SwiftUI

/// HTTP method.
public enum HTTPMethod: String, Sendable {
    case get = "GET"
    case post = "POST"
    case put = "PUT"
    case delete = "DELETE"
    case patch = "PATCH"
}

/// Core API client with token management.
public actor APIClient {
    private let baseURL: String
    private let session: URLSession
    private let streamSession: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    private var accessToken: String?
    private var onUnauthorized: (@Sendable () async -> Void)?

    public init(
        baseURL: String,
        session: URLSession = .shared,
        streamSession: URLSession? = nil,
        onUnauthorized: (@Sendable () async -> Void)? = nil
    ) {
        self.baseURL = baseURL
        self.session = session
        let streamConfig = session.configuration
        // SSE keep-alives arrive every 15 seconds. Allow long tool/thinking gaps;
        // the server owns the worker idle timeout, rather than a total HTTP limit.
        streamConfig.timeoutIntervalForRequest = 660
        streamConfig.timeoutIntervalForResource = 7 * 24 * 60 * 60
        self.streamSession = streamSession ?? URLSession(configuration: streamConfig)
        self.onUnauthorized = onUnauthorized

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .iso8601
        self.decoder = decoder

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.dateEncodingStrategy = .iso8601
        self.encoder = encoder
    }

    public func setToken(_ token: String?) {
        self.accessToken = token
    }

    public func setOnUnauthorized(_ handler: @Sendable @escaping () async -> Void) {
        self.onUnauthorized = handler
    }

    private var tokenProvider: (@Sendable () async -> String?)?
    private var tokenRefresh: (@Sendable (String?) async throws -> String)?
    private var invalidateToken: (@Sendable (String?) async -> Void)?

    func setAuthentication(
        token: @Sendable @escaping () async -> String?,
        refresh: @Sendable @escaping (String?) async throws -> String,
        invalidate: @Sendable @escaping (String?) async -> Void
    ) {
        tokenProvider = token
        tokenRefresh = refresh
        invalidateToken = invalidate
    }

    private func authorize(_ request: URLRequest) async -> (URLRequest, String?) {
        let token: String?
        if let tokenProvider { token = await tokenProvider() }
        else { token = accessToken }
        var request = request
        request.setValue(token.map { "Bearer \($0)" }, forHTTPHeaderField: "Authorization")
        return (request, token)
    }

    private func recover(_ rejected: String?, attempt: Int) async throws {
        guard attempt == 0, let tokenRefresh else {
            await invalidateToken?(rejected)
            await onUnauthorized?()
            throw APIError.unauthorized
        }
        _ = try await tokenRefresh(rejected)
        try Task.checkCancellation()
    }

    /// Retry only an initial authentication rejection, never an accepted stream.
    func stream<B: Encodable & Sendable>(_ path: String, body: B) async throws -> URLSession.AsyncBytes {
        guard let url = URL(string: baseURL + path) else { throw APIError.invalidURL }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.httpBody = try encoder.encode(body)
        for attempt in 0...1 {
            let (authorized, token) = await authorize(request)
            let (bytes, response) = try await streamSession.bytes(for: authorized)
            guard let http = response as? HTTPURLResponse else { throw APIError.serverError(0, "Invalid response") }
            if http.statusCode == 401 {
                bytes.task.cancel()
                try await recover(token, attempt: attempt)
                continue
            }
            guard (200...299).contains(http.statusCode) else {
                bytes.task.cancel()
                throw APIError.serverError(http.statusCode, "Chat request failed")
            }
            return bytes
        }
        throw APIError.unauthorized
    }

    // MARK: - Request builders

    public func get<T: Decodable & Sendable>(_ path: String, query: [String: String] = [:]) async throws -> T {
        try await request(method: .get, path: path, query: query)
    }

    public func post<T: Decodable & Sendable, B: Encodable & Sendable>(_ path: String, body: B) async throws -> T {
        try await request(method: .post, path: path, body: body)
    }

    public func post(_ path: String) async throws {
        let _: EmptyResponse = try await request(method: .post, path: path)
    }

    public func put<T: Decodable & Sendable, B: Encodable & Sendable>(_ path: String, body: B) async throws -> T {
        try await request(method: .put, path: path, body: body)
    }

    public func delete(_ path: String) async throws {
        let _: EmptyResponse = try await request(method: .delete, path: path)
    }

    public func patch<T: Decodable & Sendable, B: Encodable & Sendable>(_ path: String, body: B) async throws -> T {
        try await request(method: .patch, path: path, body: body)
    }

    // MARK: - Core request

    private func request<T: Decodable>(
        method: HTTPMethod,
        path: String,
        query: [String: String] = [:],
        body: (any Encodable)? = nil
    ) async throws -> T {
        guard var components = URLComponents(string: baseURL + path) else {
            throw APIError.invalidURL
        }

        if !query.isEmpty {
            components.queryItems = query.map { URLQueryItem(name: $0.key, value: $0.value) }
        }

        guard let url = components.url else {
            throw APIError.invalidURL
        }

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = method.rawValue
        urlRequest.setValue("application/json", forHTTPHeaderField: "Accept")

        #if DEBUG
        print("[APIClient] \(method.rawValue) \(url.absoluteString)")
        #endif

        if let body {
            urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
            urlRequest.httpBody = try encoder.encode(body)
        }

        var data = Data()
        var httpResponse: HTTPURLResponse!
        for attempt in 0...1 {
            let (authorized, token) = await authorize(urlRequest)
            let response: URLResponse
            do {
                (data, response) = try await session.data(for: authorized)
            } catch {
                throw APIError.networkError(error)
            }
            guard let http = response as? HTTPURLResponse else {
                throw APIError.serverError(0, "Invalid response")
            }
            httpResponse = http
            if http.statusCode != 401 { break }
            try await recover(token, attempt: attempt)
        }

        #if DEBUG
        if httpResponse.statusCode >= 400 {
            let body = String(data: data, encoding: .utf8) ?? "<non-utf8>"
            print("[APIClient] \(method.rawValue) \(path) → \(httpResponse.statusCode): \(body)")
        }
        #endif

        switch httpResponse.statusCode {
        case 200...299:
            break
        case 401:
            await onUnauthorized?()
            throw APIError.unauthorized
        case 404:
            throw APIError.notFound
        case 400:
            let errorBody = try? decoder.decode(APIErrorResponse.self, from: data)
            throw APIError.badRequest(errorBody?.error ?? "Bad request")
        case 409:
            let errorBody = try? decoder.decode(APIErrorResponse.self, from: data)
            throw APIError.conflict(errorBody?.error ?? "Conflict")
        default:
            let errorBody = try? decoder.decode(APIErrorResponse.self, from: data)
            throw APIError.serverError(httpResponse.statusCode, errorBody?.error)
        }

        // Handle empty responses (204 No Content or empty body)
        if data.isEmpty || httpResponse.statusCode == 204 {
            if let empty = EmptyResponse() as? T {
                return empty
            }
        }

        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            #if DEBUG
            print("[APIClient] decode \(T.self) failed: \(error)")
            #endif
            throw APIError.decodingError(error)
        }
    }
}

/// Placeholder for endpoints that return no body.
struct EmptyResponse: Codable, Sendable {
    init() {}
}
