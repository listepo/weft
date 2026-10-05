import Foundation

/// The three Editor menu commands. The identifiers are the ones in the extension's Info.plist.
public enum EditorCommand: String, CaseIterable, Sendable {
    case importSelection = "dev.weft.editor.import-selection"
    case generateSwiftUI = "dev.weft.editor.generate-swiftui"
    case validate = "dev.weft.editor.validate"
}

/// What a command leaves for the user. XcodeKit lets a command change only the buffer it was run
/// in, so a conversion goes to the clipboard and the message tells what happened.
public struct Outcome: Sendable, Equatable {
    public var clipboard: String?
    public var message: String
    public var isError: Bool
}

public struct WeftEditor: Sendable {
    public let runner: WeftRunner
    /// Longest list of diagnostics or losses in a message; Xcode shows it in a small banner.
    static let listed = 5

    public init(runner: WeftRunner) {
        self.runner = runner
    }

    public func perform(_ command: EditorCommand, on buffer: Buffer) -> Outcome {
        do {
            switch command {
            case .importSelection: return try importSelection(buffer)
            case .generateSwiftUI: return try generateSwiftUI(buffer)
            case .validate: return try validate(buffer)
            }
        } catch {
            return Outcome(clipboard: nil, message: error.localizedDescription, isError: true)
        }
    }

    private func importSelection(_ buffer: Buffer) throws -> Outcome {
        guard buffer.isSwift else { return failure("Open a Swift file: this command reads SwiftUI.") }
        let source = Self.viewSource(buffer.selectedText ?? buffer.text)
        let result = try runner.run("import-swiftui", text: source, fileName: "View.swift")
        guard result.status == 0 else { return failure(result.standardError) }
        let losses = result.standardError.split(separator: "\n").filter { $0.contains(" loss ") }
        var message = "Copied the screen to the clipboard."
        if !losses.isEmpty {
            message += " Weft cannot hold \(losses.count) \(losses.count == 1 ? "thing" : "things"):\n" + Self.list(losses)
        }
        return Outcome(clipboard: result.standardOutput, message: message, isError: false)
    }

    private func generateSwiftUI(_ buffer: Buffer) throws -> Outcome {
        guard !buffer.isSwift else { return failure("Open a .weft file: this command reads a Weft screen.") }
        let result = try runner.run("swiftui", text: buffer.text, fileName: "screen.weft")
        guard result.status == 0 else { return failure(result.standardError) }
        return Outcome(clipboard: result.standardOutput, message: "Copied the SwiftUI view to the clipboard.", isError: false)
    }

    private func validate(_ buffer: Buffer) throws -> Outcome {
        guard !buffer.isSwift else { return failure("Open a .weft file: this command reads a Weft screen.") }
        let extra = runner.catalog.map { ["--strict", "--catalog", $0.path] } ?? []
        let result = try runner.run("validate", text: buffer.text, fileName: "screen.weft", extra: extra)
        let found = result.standardOutput.split(separator: "\n")
        if result.status != 0 { return failure(Self.list(found.isEmpty ? result.standardError.split(separator: "\n") : found)) }
        return Outcome(
            clipboard: nil,
            message: found.isEmpty ? "The screen is valid." : "The screen is valid, with warnings:\n" + Self.list(found),
            isError: false)
    }

    private func failure(_ message: String) -> Outcome {
        Outcome(clipboard: nil, message: message.trimmingCharacters(in: .whitespacesAndNewlines), isError: true)
    }

    private static func list(_ lines: [Substring]) -> String {
        let shown = lines.prefix(listed).joined(separator: "\n")
        return lines.count > listed ? shown + "\n… and \(lines.count - listed) more" : shown
    }

    /// The importer reads a type that conforms to `View`. A selection that is only a view
    /// expression is wrapped in one, so selecting `VStack { … }` works.
    static func viewSource(_ text: String) -> String {
        if text.range(of: #":\s*(?:[\w.]+,\s*)*View\b"#, options: .regularExpression) != nil { return text }
        let body = text.split(separator: "\n", omittingEmptySubsequences: false).map { "        " + $0 }.joined(separator: "\n")
        return "import SwiftUI\n\nstruct Selection: View {\n    var body: some View {\n\(body)\n    }\n}\n"
    }
}

extension Bundle {
    /// The `weft` executable the extension build embeds next to its own, in `Contents/MacOS`.
    public var weftExecutable: URL? { url(forAuxiliaryExecutable: "weft") }
}
