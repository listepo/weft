import Foundation
import Testing

@testable import WeftEditorCore

/// The `weft` of the artifact bundle (`moon run xcode-plugin:bundle`), or WEFT_BINARY.
private let weft: URL? = {
    if let path = ProcessInfo.processInfo.environment["WEFT_BINARY"] { return URL(fileURLWithPath: path) }
    let bundle = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("Artifacts/weft.artifactbundle")
    let variants = (try? FileManager.default.contentsOfDirectory(atPath: bundle.path)) ?? []
    return variants.first { $0.hasPrefix("weft-") }.map { bundle.appendingPathComponent($0 + "/bin/weft") }
}()

private let repository = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

private func corpus(_ name: String) throws -> String {
    try String(contentsOf: repository.appendingPathComponent("corpus/\(name)/screen.weft"), encoding: .utf8)
}

private func lines(_ text: String) -> [String] {
    text.split(separator: "\n", omittingEmptySubsequences: false).enumerated().map { index, line in
        index == text.split(separator: "\n", omittingEmptySubsequences: false).count - 1 ? String(line) : line + "\n"
    }.filter { !$0.isEmpty }
}

private func editor() throws -> WeftEditor {
    let executable = try #require(weft, "no weft: run `moon run xcode-plugin:bundle` or set WEFT_BINARY")
    let catalog = repository.appendingPathComponent("packages/catalog/catalog.json")
    return WeftEditor(runner: WeftRunner(executable: executable, catalog: catalog))
}

@Suite struct SelectionTests {
    @Test func takesTheTextBetweenTheSelectionEnds() {
        let buffer = Buffer(
            lines: ["struct A {\n", "    var x = 1\n", "}\n"],
            selections: [TextRange(start: .init(line: 0, column: 7), end: .init(line: 1, column: 7))],
            isSwift: true)
        #expect(buffer.selectedText == "A {\n    var")
    }

    @Test func anEmptyOrBlankSelectionSelectsNothing() {
        let empty = Buffer(lines: ["abc\n"], selections: [TextRange(start: .init(line: 0, column: 1), end: .init(line: 0, column: 1))], isSwift: true)
        #expect(empty.selectedText == nil)
        let blank = Buffer(lines: ["a  \n"], selections: [TextRange(start: .init(line: 0, column: 1), end: .init(line: 0, column: 3))], isSwift: true)
        #expect(blank.selectedText == nil)
        #expect(Buffer(lines: ["abc\n"], isSwift: true).selectedText == nil)
    }

    @Test func positionsPastTheEndAreClamped() {
        let buffer = Buffer(
            lines: ["ab\n", "cd"],
            selections: [TextRange(start: .init(line: 0, column: 1), end: .init(line: 5, column: 99))],
            isSwift: true)
        #expect(buffer.selectedText == "b\ncd")
    }

    @Test func viewExpressionsAreWrappedAndViewsAreNot() {
        #expect(WeftEditor.viewSource("VStack {}").contains("struct Selection: View"))
        let own = "struct Hello: View { var body: some View { Text(\"x\") } }"
        #expect(WeftEditor.viewSource(own) == own)
        let several = "struct Hello: Identifiable, View { var id = 1; var body: some View { Text(\"x\") } }"
        #expect(WeftEditor.viewSource(several) == several)
    }
}

@Suite struct CommandTests {
    @Test func aWeftBufferBecomesSwiftUIAndComesBack() throws {
        let editor = try editor()
        let screen = try corpus("login")
        let generated = editor.perform(.generateSwiftUI, on: Buffer(lines: lines(screen), isSwift: false))
        #expect(!generated.isError, Comment(rawValue: generated.message))
        let swift = try #require(generated.clipboard)
        #expect(swift.contains("struct LoginScreen: View"))

        // The importer gives back what the generator printed (SPEC §9): the canonical screen,
        // stamped with the current version (the corpus stays at 0.1, which 0.3 reads unchanged).
        let imported = editor.perform(.importSelection, on: Buffer(lines: lines(swift), isSwift: true))
        #expect(!imported.isError, Comment(rawValue: imported.message))
        let formatted = try editor.runner.run("fmt", text: screen, fileName: "screen.weft")
        let stamped = formatted.standardOutput.replacingOccurrences(of: "weft=\"0.1\"", with: "weft=\"0.3\"")
        #expect(imported.clipboard == stamped)
    }

    @Test func aSelectionIsImportedAndItsLossesAreListed() throws {
        let source = "struct Hello: View {\n    var body: some View { Text(\"Hi\").padding() }\n}\n"
        let all = lines("// header\n" + source)
        let selection = TextRange(start: .init(line: 1, column: 0), end: .init(line: 3, column: 0))
        let outcome = try editor().perform(.importSelection, on: Buffer(lines: all, selections: [selection], isSwift: true))
        #expect(!outcome.isError, Comment(rawValue: outcome.message))
        #expect(outcome.clipboard?.contains("<text id=\"text-hi\">Hi</text>") == true)
        #expect(outcome.message.contains("Weft cannot hold"))
        #expect(outcome.message.contains("`.padding` not kept"))
        #expect(!outcome.message.contains("View.swift"))
    }

    @Test func aViewExpressionSelectionIsWrappedToImport() throws {
        let selection = TextRange(start: .init(line: 0, column: 0), end: .init(line: 0, column: 40))
        let outcome = try editor().perform(
            .importSelection,
            on: Buffer(lines: ["Text(\"Hi\").accessibilityIdentifier(\"hi\")\n"], selections: [selection], isSwift: true))
        #expect(!outcome.isError, Comment(rawValue: outcome.message))
        #expect(outcome.clipboard?.contains("<text") == true)
    }

    @Test func validateAcceptsACorpusScreenAndRejectsABrokenOne() throws {
        let editor = try editor()
        let good = editor.perform(.validate, on: Buffer(lines: lines(try corpus("login")), isSwift: false))
        #expect(!good.isError, Comment(rawValue: good.message))
        #expect(good.message.hasPrefix("The screen is valid"))

        let broken = try corpus("login").replacingOccurrences(of: "<heading", with: "<headline")
        let bad = editor.perform(.validate, on: Buffer(lines: lines(broken), isSwift: false))
        #expect(bad.isError)
        // The temporary file's name never reaches the user: the message starts at line:column.
        #expect(bad.message.range(of: #"^\d+:\d+ W\d+"#, options: .regularExpression) != nil, Comment(rawValue: bad.message))
    }

    @Test func aSyntaxErrorIsReportedByValidateAndGenerate() throws {
        let editor = try editor()
        let buffer = Buffer(lines: ["<screen\n"], isSwift: false)
        #expect(editor.perform(.validate, on: buffer).message.contains("W110"))
        #expect(editor.perform(.generateSwiftUI, on: buffer).isError)
    }

    @Test func commandsRefuseTheWrongKindOfBuffer() throws {
        let editor = try editor()
        #expect(editor.perform(.importSelection, on: Buffer(lines: ["<screen/>\n"], isSwift: false)).isError)
        #expect(editor.perform(.generateSwiftUI, on: Buffer(lines: ["let x = 1\n"], isSwift: true)).isError)
        #expect(editor.perform(.validate, on: Buffer(lines: ["let x = 1\n"], isSwift: true)).isError)
    }

    @Test func aMissingExecutableIsAnErrorNotACrash() {
        let editor = WeftEditor(runner: WeftRunner(executable: URL(fileURLWithPath: "/nonexistent/weft")))
        let outcome = editor.perform(.validate, on: Buffer(lines: ["<screen/>\n"], isSwift: false))
        #expect(outcome.isError)
        #expect(outcome.message.contains("could not be started"))
    }
}
