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
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    private var accessToken: String?
    private var onUnauthorized: (@Sendable () async -> Void)?

    public init(
        baseURL: String,
        session: URLSession = .shared,
        onUnauthorized: (@Sendable () async -> Void)? = nil
    ) {
        self.baseURL = baseURL
        self.session = session
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

        if let token = accessToken {
            urlRequest.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        if let body {
            urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
            urlRequest.httpBody = try encoder.encode(body)
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: urlRequest)
        } catch {
            throw APIError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.serverError(0, "Invalid response")
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
