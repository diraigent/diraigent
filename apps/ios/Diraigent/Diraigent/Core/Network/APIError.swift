import Foundation

/// API error cases.
public enum APIError: Error, LocalizedError, Sendable {
    case unauthorized
    case notFound
    case badRequest(String)
    case conflict(String)
    case serverError(Int, String?)
    case networkError(Error)
    case decodingError(Error)
    case invalidURL

    public var errorDescription: String? {
        switch self {
        case .unauthorized: "Authentication required"
        case .notFound: "Resource not found"
        case .badRequest(let msg): "Bad request: \(msg)"
        case .conflict(let msg): "Conflict: \(msg)"
        case .serverError(let code, let msg): "Server error \(code): \(msg ?? "Unknown")"
        case .networkError(let err): "Network error: \(err.localizedDescription)"
        case .decodingError(let err): "Decoding error: \(err.localizedDescription)"
        case .invalidURL: "Invalid URL"
        }
    }
}

/// Error response body from the API.
struct APIErrorResponse: Codable, Sendable {
    let error: String?
    let errorCode: String?
}
