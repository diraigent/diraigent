import Foundation
import SwiftUI

/// The kind of a source tree entry.
enum EntryKind: String, Codable, Sendable {
    case file
    case dir
}

/// A single entry in the source file tree.
struct TreeEntry: Codable, Identifiable, Sendable {
    let name: String
    let path: String
    let kind: EntryKind

    /// Use the path as a stable identifier.
    var id: String { path.isEmpty ? name : path }

    /// Whether this entry is a directory.
    var isDirectory: Bool { kind == .dir }

    /// SF Symbol name for the entry type.
    var icon: String {
        isDirectory ? "folder.fill" : "doc.fill"
    }

    /// Icon color for the entry type.
    var iconColor: Color {
        isDirectory ? .blue : .secondary
    }
}

/// Response from the source tree endpoint.
struct TreeResponse: Codable, Sendable {
    let entries: [TreeEntry]
}

/// Response from the source blob endpoint.
struct BlobResponse: Codable, Sendable {
    let content: String
    let encoding: String
    let size: Int
}
