// The document model over the real weft-studio-ffi, with a fake picker in place of the panels.
// Slint takes the first thread that draws as its UI thread, so everything runs on the main actor.

import Foundation
import Testing
@testable import WeftStudio

private let corpus = URL(filePath: #filePath)
    .deletingLastPathComponent()
    .appending(path: "../../../corpus")
    .standardizedFileURL

/// A folder of its own, removed by the test that made it.
private func scratch() throws -> URL {
    let dir = FileManager.default.temporaryDirectory.appending(path: "weft-studio-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return dir
}

/// `corpus/<name>/screen.weft` copied into `dir`, so saving never touches the corpus.
private func screen(_ name: String, in dir: URL) throws -> URL {
    let copy = dir.appending(path: "screen.weft")
    try FileManager.default.copyItem(at: corpus.appending(path: "\(name)/screen.weft"), to: copy)
    return copy
}

@MainActor
private final class FakePicker: FilePicking {
    var save: URL?
    private(set) var suggestedNames: [String] = []

    func pickFileToOpen() -> URL? { nil }

    func pickSaveLocation(suggestedName: String) -> URL? {
        suggestedNames.append(suggestedName)
        return save
    }
}

@MainActor
@Suite(.serialized)
struct StudioDocumentTests {
    @Test func aCorpusScreenOpensWithItsTitleAndASurface() throws {
        let dir = try scratch()
        defer { try? FileManager.default.removeItem(at: dir) }
        let document = StudioDocument()
        #expect(document.open(try screen("login", in: dir)))
        #expect(document.error == nil)
        #expect(document.state?.title == "screen.weft")
        #expect(document.state?.screen == "login")
        let surface = try #require(document.surface)
        #expect(surface.pixelWidth == 1 && surface.pixelHeight == 1)
    }

    @Test func saveAsWritesTheSameBytesWhereThePickerSays() throws {
        let dir = try scratch()
        defer { try? FileManager.default.removeItem(at: dir) }
        let original = try screen("login", in: dir)
        let document = StudioDocument()
        #expect(document.open(original))
        let picker = FakePicker()
        picker.save = dir.appending(path: "copy.weft")
        #expect(document.saveAs(using: picker))
        #expect(picker.suggestedNames == ["screen.weft"])
        #expect(try Data(contentsOf: dir.appending(path: "copy.weft")) == Data(contentsOf: original))
        #expect(document.state?.title == "copy.weft")
    }

    @Test func aCancelledSaveAsChangesNothing() throws {
        let dir = try scratch()
        defer { try? FileManager.default.removeItem(at: dir) }
        let document = StudioDocument()
        #expect(document.open(try screen("login", in: dir)))
        #expect(!document.saveAs(using: FakePicker()))
        #expect(document.error == nil)
        #expect(document.state?.title == "screen.weft")
    }

    @Test func saveWritesBackToTheDocumentsPath() throws {
        let dir = try scratch()
        defer { try? FileManager.default.removeItem(at: dir) }
        let path = try screen("account", in: dir)
        let original = try Data(contentsOf: path)
        let document = StudioDocument()
        #expect(document.open(path))
        try FileManager.default.removeItem(at: path)
        #expect(document.save())
        #expect(try Data(contentsOf: path) == original)
    }

    @Test func nothingSavesBeforeAnOpen() {
        let document = StudioDocument()
        #expect(!document.save())
        #expect(!document.saveAs(using: FakePicker()))
        #expect(document.state == nil)
    }

    @Test func aSyntaxErrorIsAnErrorWithItsDiagnosticsAndKeepsTheOpenDocument() throws {
        let dir = try scratch()
        defer { try? FileManager.default.removeItem(at: dir) }
        let document = StudioDocument()
        #expect(document.open(try screen("login", in: dir)))
        let broken = dir.appending(path: "broken.weft")
        try Data(#"<screen id="s" weft="0.1"><heading"#.utf8).write(to: broken)
        #expect(!document.open(broken))
        let error = try #require(document.error)
        #expect(error.summary.contains("syntax"))
        #expect(error.details.hasPrefix("1:"), "\(error.message)")
        #expect(document.state?.title == "screen.weft")
    }

    @Test func aRefusedScreenOpensWithoutASurfaceAndSaysWhy() throws {
        let dir = try scratch()
        defer { try? FileManager.default.removeItem(at: dir) }
        let refused = dir.appending(path: "refused.weft")
        try Data(#"<screen id="s" weft="0.1"><heading id="t" level="1" shade="dark">Hi</heading></screen>"#.utf8)
            .write(to: refused)
        let document = StudioDocument()
        #expect(document.open(refused))
        #expect(document.state?.title == "refused.weft")
        #expect(document.surface == nil)
        let error = try #require(document.error)
        #expect(error.summary.hasPrefix("the document has "), "\(error.message)")
        #expect(error.details.contains("shade"), "\(error.message)")
    }

    @Test func theLaunchScreenComesFromTheArgumentDomain() throws {
        let suite = "weft-studio-tests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        #expect(LaunchOptions.screen(in: defaults) == nil)
        defaults.set("/tmp/a/screen.weft", forKey: LaunchOptions.screenKey)
        #expect(LaunchOptions.screen(in: defaults) == URL(filePath: "/tmp/a/screen.weft"))
    }

    @Test func theEnvironmentCanReplaceThePanels() {
        #expect(Pickers.make(environment: [:]) is PanelPicker)
        let picker = Pickers.make(environment: [Pickers.saveKey: "/tmp/copy.weft"])
        #expect(picker.pickSaveLocation(suggestedName: "screen.weft") == URL(filePath: "/tmp/copy.weft"))
        #expect(picker.pickFileToOpen() == nil)
    }
}
