import AppKit
import WeftEditorCore
import XcodeKit

/// One class for the three Editor menu commands; Xcode tells them apart by identifier. The work is
/// in WeftEditorCore, which the tests cover; this file only maps XcodeKit's types onto its.
@objc(WeftSourceEditorCommand)
final class SourceEditorCommand: NSObject, XCSourceEditorCommand {
    static let domain = "dev.weft.editor"

    func perform(with invocation: XCSourceEditorCommandInvocation, completionHandler: @escaping (Error?) -> Void) {
        guard let command = EditorCommand(rawValue: invocation.commandIdentifier),
            let weft = Bundle.main.weftExecutable
        else {
            return completionHandler(Self.error("The Weft extension is incomplete: its weft executable is missing."))
        }
        let catalog = Bundle.main.url(forResource: "catalog", withExtension: "json")
        let buffer = Buffer(
            lines: invocation.buffer.lines.compactMap { $0 as? String },
            selections: invocation.buffer.selections.compactMap { $0 as? XCSourceTextRange }.map {
                TextRange(
                    start: TextPosition(line: $0.start.line, column: $0.start.column),
                    end: TextPosition(line: $0.end.line, column: $0.end.column))
            },
            isSwift: invocation.buffer.contentUTI == "public.swift-source")
        let outcome = WeftEditor(runner: WeftRunner(executable: weft, catalog: catalog)).perform(command, on: buffer)
        if let text = outcome.clipboard {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        // XcodeKit has no other way to show a message than an error, so success is reported with
        // one too (code 0); the command is not undone either way, because it never edits the buffer.
        completionHandler(Self.error(outcome.message, code: outcome.isError ? 1 : 0))
    }

    private static func error(_ message: String, code: Int = 1) -> NSError {
        NSError(domain: domain, code: code, userInfo: [NSLocalizedDescriptionKey: message])
    }
}
