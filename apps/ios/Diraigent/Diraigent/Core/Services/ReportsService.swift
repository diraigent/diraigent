import Foundation
import SwiftUI

/// Request body for creating a report.
struct CreateReportRequest: Encodable, Sendable {
    let title: String
    let kind: String
    let prompt: String
}

/// Service for managing reports.
@Observable
@MainActor
final class ReportsService {
    private let apiClient: APIClient

    var reports: [Report] = []
    var isLoading = false
    var error: String?

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    /// Fetch all reports for a project.
    func fetchReports(projectId: UUID) async {
        isLoading = true
        error = nil
        do {
            let result: [Report] = try await apiClient.get(Endpoints.reports(projectId))
            reports = result
        } catch {
            self.error = error.localizedDescription
            print("[ReportsService] fetchReports failed: \(error)")
        }
        isLoading = false
    }

    /// Create a new report.
    func createReport(projectId: UUID, request: CreateReportRequest) async -> Report? {
        do {
            let result: Report = try await apiClient.post(Endpoints.reports(projectId), body: request)
            reports.insert(result, at: 0)
            return result
        } catch {
            self.error = error.localizedDescription
            print("[ReportsService] createReport failed: \(error)")
            return nil
        }
    }

    /// Delete a report.
    func deleteReport(projectId: UUID, reportId: UUID) async -> Bool {
        do {
            try await apiClient.delete("/reports/\(reportId)")
            reports.removeAll { $0.id == reportId }
            return true
        } catch {
            self.error = error.localizedDescription
            print("[ReportsService] deleteReport failed: \(error)")
            return false
        }
    }
}
