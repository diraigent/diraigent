import Testing
@testable import Diraigent

struct DiraigentTests {
    @Test func appConfigHasValidURLs() async throws {
        #expect(!AppConfig.development.apiBaseURL.isEmpty)
        #expect(!AppConfig.production.apiBaseURL.isEmpty)
    }
}
