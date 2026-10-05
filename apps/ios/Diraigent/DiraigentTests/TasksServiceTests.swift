import Foundation
import Testing
@testable import Diraigent

private final class TasksURLProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var handler: (@Sendable (URLRequest) throws -> (Int, String))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        do {
            let (status, body) = try Self.handler!(request)
            client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status,
                httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Data(body.utf8))
            client?.urlProtocolDidFinishLoading(self)
        } catch { client?.urlProtocol(self, didFailWithError: error) }
    }
    override func stopLoading() {}
}

@Suite(.serialized)
@MainActor
struct TasksServiceTests {
    private let projectId = UUID()
    private let backlog = #"{"id":"00000000-0000-0000-0000-000000000001","title":"Backlog task","state":"backlog"}"#
    private let ready = #"{"id":"00000000-0000-0000-0000-000000000002","title":"Ready task","state":"ready"}"#

    private func fixture() -> TasksService {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [TasksURLProtocol.self]
        return TasksService(apiClient: APIClient(baseURL: "https://api.example",
            session: URLSession(configuration: configuration)))
    }

    private func page(_ tasks: String, offset: Int = 0, hasMore: Bool = false) -> String {
        "{\"data\":[\(tasks)],\"total\":2,\"limit\":100,\"offset\":\(offset),\"has_more\":\(hasMore)}"
    }

    @Test func loadsPaginatedTasksIncludingBacklog() async {
        let body = page("\(backlog),\(ready)")
        let expectedPath = Endpoints.tasks(projectId)
        TasksURLProtocol.handler = { request in
            #expect(request.httpMethod == "GET")
            #expect(request.url?.path == expectedPath)
            let query = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
            #expect(Set(query.map(\.name)) == ["limit", "offset"])
            return (200, body)
        }
        let service = fixture()
        await service.fetchTasks(projectId: projectId)
        #expect(service.tasks.map(\.state) == ["backlog", "ready"])
        #expect(service.error == nil)
        #expect(!service.isLoading)
    }

    @Test func followsPaginationUntilLastPage() async {
        let first = page(ready, hasMore: true)
        let last = page(backlog, offset: 1)
        TasksURLProtocol.handler = { request in
            let query = Dictionary(uniqueKeysWithValues: URLComponents(
                url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!.map { ($0.name, $0.value!) })
            #expect(query["limit"] == "100")
            let offset = query["offset"]
            #expect(["0", "1"].contains(offset ?? "<missing>"))
            return (200, offset == "0" ? first : last)
        }
        let service = fixture()
        await service.fetchTasks(projectId: projectId)
        #expect(service.tasks.map(\.title) == ["Ready task", "Backlog task"])
        #expect(service.error == nil)
    }

    @Test func emptyRefreshClearsPreviousTasks() async {
        let service = fixture()
        let body = page(backlog)
        TasksURLProtocol.handler = { _ in (200, body) }
        await service.fetchTasks(projectId: projectId)
        #expect(service.tasks.count == 1)
        TasksURLProtocol.handler = { _ in
            (200, #"{"data":[],"total":0,"limit":100,"offset":0,"has_more":false}"#)
        }
        await service.fetchTasks(projectId: projectId)
        #expect(service.tasks.isEmpty)
        #expect(service.error == nil)
        #expect(!service.isLoading)
    }

    @Test func failedLaterPagePreservesPreviousListAndReportsError() async {
        let service = fixture()
        let initial = page(backlog)
        TasksURLProtocol.handler = { _ in (200, initial) }
        await service.fetchTasks(projectId: projectId)
        let first = page(ready, hasMore: true)
        TasksURLProtocol.handler = { request in
            let query = Dictionary(uniqueKeysWithValues: URLComponents(
                url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!.map { ($0.name, $0.value!) })
            return query["offset"] == "0"
                ? (200, first) : (503, "{}")
        }
        await service.fetchTasks(projectId: projectId)
        #expect(service.tasks.map(\.state) == ["backlog"])
        #expect(service.error != nil)
        #expect(!service.isLoading)
    }
}
