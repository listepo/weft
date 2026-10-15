// The app as a user sees it: launched with a corpus screen, its window, the drawn surface, and
// Save As through the launch-environment picker (the system panels are a manual check).

import AppKit
import XCTest

final class WeftStudioUITests: XCTestCase {
    private let login = URL(filePath: #filePath)
        .deletingLastPathComponent()
        .appending(path: "../../../corpus/login/screen.weft")
        .standardizedFileURL

    @MainActor
    func testACorpusScreenOpensDrawsAndSavesAs() throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: "weft-studio-ui-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let copy = dir.appending(path: "copy.weft")

        let app = XCUIApplication()
        // No restored windows from an earlier run.
        app.launchArguments = ["-WeftStudioScreen", login.path, "-ApplePersistenceIgnoreState", "YES"]
        app.launchEnvironment = ["WEFT_STUDIO_SAVE_PATH": copy.path]
        app.launch()
        defer { app.terminate() }

        let window = app.windows.matching(NSPredicate(format: "title == %@", "screen.weft")).firstMatch
        // The first surface of a freshly linked library takes a few seconds.
        XCTAssertTrue(window.waitForExistence(timeout: 60), "no window titled screen.weft")
        let surface = window.images["surface"]
        XCTAssertTrue(surface.waitForExistence(timeout: 30), "no surface in the window")
        XCTAssertTrue(waitUntil(timeout: 30) { colours(in: surface.screenshot().image) > 1 }, "the surface is one colour")

        window.typeKey("s", modifierFlags: [.command, .shift])
        XCTAssertTrue(waitUntil(timeout: 30) { FileManager.default.fileExists(atPath: copy.path) }, "Save As wrote nothing")
        XCTAssertEqual(try Data(contentsOf: copy), try Data(contentsOf: login))
        let renamed = app.windows.matching(NSPredicate(format: "title == %@", "copy.weft")).firstMatch
        XCTAssertTrue(renamed.waitForExistence(timeout: 10), "the window kept its old title")
    }

    private func waitUntil(timeout: TimeInterval, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        return condition()
    }
}

/// Distinct colours in the image, stopping at two.
private func colours(in image: NSImage) -> Int {
    guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return 0 }
    let rep = NSBitmapImageRep(cgImage: cg)
    guard let first = rep.colorAt(x: 0, y: 0) else { return 0 }
    for y in stride(from: 0, to: rep.pixelsHigh, by: 2) {
        for x in stride(from: 0, to: rep.pixelsWide, by: 2) where rep.colorAt(x: x, y: y) != first {
            return 2
        }
    }
    return 1
}
