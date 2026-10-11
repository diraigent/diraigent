import XCTest

@MainActor
final class DiraigentUITests: XCTestCase {
    func testDemoCanBrowseWithoutChatOrExecutionControls() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-test-demo"]
        app.launch()
        XCTAssertTrue(app.staticTexts["Read-only access"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.tabBars.buttons["Chat"].exists)
        app.tabBars.buttons["Work"].tap()
        XCTAssertTrue(app.staticTexts["Demo work item"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["Add"].exists)
        app.staticTexts["Demo work item"].tap()
        XCTAssertTrue(app.staticTexts["Visible description"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["Execute"].exists)
        XCTAssertFalse(app.buttons["Plan & Execute"].exists)
        app.tabBars.buttons["More"].tap()
        XCTAssertFalse(app.buttons["Chat"].exists)
        app.buttons["Logs"].tap()
        XCTAssertTrue(app.buttons["Demo implementation"].waitForExistence(timeout: 5))
        app.buttons["Demo implementation"].tap()
        XCTAssertTrue(app.staticTexts["Demo worker output [redacted]"].waitForExistence(timeout: 5))
        app.navigationBars.buttons["Logs"].tap()
        XCTAssertTrue(app.navigationBars.buttons["More"].waitForExistence(timeout: 5))
        app.navigationBars.buttons["More"].tap()
        XCTAssertTrue(app.buttons["Source Browser"].waitForExistence(timeout: 5))
        app.buttons["Source Browser"].tap()
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "app.swift")).firstMatch.waitForExistence(timeout: 5))
        app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "app.swift")).firstMatch.tap()
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Demo source [redacted]")).firstMatch.waitForExistence(timeout: 5))
    }

    func testAppLaunches() throws {
        let app = XCUIApplication()
        app.launch()
    }
}
