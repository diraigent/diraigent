import Foundation

/// A single log entry from the Loki query response.
struct LogEntry: Codable, Identifiable, Sendable {
    /// Timestamp as a string (nanoseconds or RFC3339).
    let timestamp: String
    /// The log line content.
    let line: String
    /// Labels attached to this log entry.
    let labels: [String: AnyCodable]?

    /// Stable identity based on timestamp + line hash.
    var id: String { "\(timestamp)-\(line.hashValue)" }

    /// Detected log level from content analysis.
    var detectedLevel: LogLevel {
        let lower = line.lowercased()
        if lower.contains("error") || lower.contains("err=") || lower.contains("panic") || lower.contains("fatal") {
            return .error
        } else if lower.contains("warn") || lower.contains("warning") {
            return .warning
        } else if lower.contains("debug") || lower.contains("trace") {
            return .debug
        } else {
            return .info
        }
    }

    /// Formatted timestamp for display (HH:MM:SS.ms).
    var formattedTimestamp: String {
        // Try parsing as nanosecond epoch
        if let nanos = Int64(timestamp) {
            let seconds = TimeInterval(nanos) / 1_000_000_000
            let date = Date(timeIntervalSince1970: seconds)
            return Self.timeFormatter.string(from: date)
        }
        // Try parsing as ISO8601
        let isoFormatter = ISO8601DateFormatter()
        isoFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = isoFormatter.date(from: timestamp) {
            return Self.timeFormatter.string(from: date)
        }
        isoFormatter.formatOptions = [.withInternetDateTime]
        if let date = isoFormatter.date(from: timestamp) {
            return Self.timeFormatter.string(from: date)
        }
        return timestamp
    }

    private static let timeFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()
}

/// Detected log level.
enum LogLevel: String, Sendable {
    case error
    case warning
    case info
    case debug
}

/// Response from the logs query endpoint.
struct LogsResponse: Codable, Sendable {
    let entries: [LogEntry]
    let total: Int
}

/// Response from the logs labels endpoint.
struct LokiLabelsResponse: Codable, Sendable {
    let status: String?
    let data: [String]?
}
