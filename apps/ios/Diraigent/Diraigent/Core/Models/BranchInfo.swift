import Foundation

/// Git branch information.
struct BranchInfo: Codable, Sendable, Identifiable {
    var id: String { name }
    let name: String
    let commit: String?
    let isPushed: Bool?
    let aheadRemote: Int?
    let behindRemote: Int?
    let taskIdPrefix: String?
}

/// Response for the branch list endpoint.
struct BranchListResponse: Codable, Sendable {
    let currentBranch: String
    let branches: [BranchInfo]
}

/// Git task status response.
struct GitTaskStatus: Codable, Sendable {
    let branch: String?
    let exists: Bool?
    let ahead: Int?
    let behind: Int?
    let changedFilesCount: Int?
}

/// Main push status.
struct MainPushStatus: Codable, Sendable {
    let ahead: Int?
    let behind: Int?
    let lastCommit: String?
    let lastCommitMessage: String?
}
