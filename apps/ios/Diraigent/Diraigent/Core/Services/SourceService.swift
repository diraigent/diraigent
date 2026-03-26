import Foundation
import SwiftUI

/// Service for browsing source files via the diraigent API.
@Observable
@MainActor
final class SourceService {
    private let apiClient: APIClient

    var entries: [TreeEntry] = []
    var blobContent: String?
    var blobSize: Int?
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch the file tree for a directory path.
    func fetchTree(projectId: UUID, path: String = "", ref: String? = nil) async {
        isLoading = true
        error = nil
        do {
            var query: [String: String] = ["path": path]
            if let ref {
                query["ref"] = ref
            }
            let result: TreeResponse = try await apiClient.get(
                Endpoints.sourceTree(projectId),
                query: query
            )
            entries = result.entries.sorted { lhs, rhs in
                // Directories first, then alphabetical
                if lhs.isDirectory != rhs.isDirectory {
                    return lhs.isDirectory
                }
                return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
            }
        } catch {
            self.error = error.localizedDescription
            entries = []
            print("[SourceService] fetchTree failed: \(error)")
        }
        isLoading = false
    }

    /// Fetch the content of a single file.
    func fetchBlob(projectId: UUID, path: String, ref: String? = nil) async {
        isLoading = true
        error = nil
        blobContent = nil
        blobSize = nil
        do {
            var query: [String: String] = ["path": path]
            if let ref {
                query["ref"] = ref
            }
            let result: BlobResponse = try await apiClient.get(
                Endpoints.sourceBlob(projectId),
                query: query
            )
            blobContent = result.content
            blobSize = result.size
        } catch {
            self.error = error.localizedDescription
            print("[SourceService] fetchBlob failed: \(error)")
        }
        isLoading = false
    }
}
