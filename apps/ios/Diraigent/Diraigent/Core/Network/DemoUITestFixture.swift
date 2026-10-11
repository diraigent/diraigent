import Foundation

/// Synthetic transport used only by simulator UI tests in Debug builds.
enum DemoUITestFixture {
    static var isEnabled: Bool {
        #if DEBUG
        ProcessInfo.processInfo.arguments.contains("--ui-test-demo")
        #else
        false
        #endif
    }

    static func client() -> APIClient? {
        #if DEBUG
        guard isEnabled else { return nil }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [DemoFixtureProtocol.self]
        return APIClient(baseURL: "https://demo.invalid/v1", session: URLSession(configuration: configuration),
                         writesAllowedByDefault: false)
        #else
        nil
        #endif
    }
}

#if DEBUG
private final class DemoFixtureProtocol: URLProtocol, @unchecked Sendable {
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "demo.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}

    override func startLoading() {
        let project = "00000000-0000-4000-8000-000000000101"
        let work = "00000000-0000-4000-8000-000000000102"
        let entry = "00000000-0000-4000-8000-000000000103"
        let path = request.url!.path.replacingOccurrences(of: "/v1", with: "")
        let body: String
        switch path {
        case "/account", "/\(project)/people/me": body = #"{"read_only":true}"#
        case "", "/": body = "[{\"id\":\"\(project)\",\"name\":\"Demo workspace\",\"slug\":\"demo\"}]"
        case "/\(project)/work": body = "[{\"id\":\"\(work)\",\"title\":\"Demo work item\",\"status\":\"active\",\"description\":\"Visible description\"}]"
        case "/work/\(work)/tasks", "/agents": body = "[]"
        case "/work/\(work)/progress": body = "{\"work_id\":\"\(work)\",\"total_tasks\":0,\"done_tasks\":0}"
        case "/\(project)/audit": body = "{\"data\":[{\"id\":\"\(entry)\",\"summary\":\"Demo audit [redacted]\",\"action\":\"task.updated\"}],\"total\":1,\"limit\":50,\"offset\":0,\"has_more\":false}"
        case "/\(project)/task-logs": body = "{\"data\":[{\"id\":\"\(entry)\",\"step_name\":\"Demo implementation\"}],\"total\":1,\"limit\":50,\"offset\":0,\"has_more\":false}"
        case "/task-logs/\(entry)": body = "{\"id\":\"\(entry)\",\"content\":\"Demo worker output [redacted]\"}"
        case "/\(project)/source/tree": body = #"{"entries":[{"name":"app.swift","path":"app.swift","kind":"file"}]}"#
        case "/\(project)/source/blob": body = #"{"content":"// Demo source [redacted]","encoding":"utf-8","size":25}"#
        case "/\(project)/metrics": body = "{}"
        default: body = #"{"data":[],"total":0,"limit":50,"offset":0,"has_more":false}"#
        }
        let status = request.httpMethod == "GET" ? 200 : 403
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status,
            httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
}
#endif
