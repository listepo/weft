import Foundation

/// A position in the editor, zero-based like XcodeKit's `XCSourceTextPosition`.
public struct TextPosition: Sendable, Equatable {
    public var line: Int
    public var column: Int

    public init(line: Int, column: Int) {
        self.line = line
        self.column = column
    }
}

public struct TextRange: Sendable, Equatable {
    public var start: TextPosition
    public var end: TextPosition

    public init(start: TextPosition, end: TextPosition) {
        self.start = start
        self.end = end
    }
}

/// What a command needs of an `XCSourceTextBuffer`, so the commands can be tested without Xcode.
public struct Buffer: Sendable, Equatable {
    public var lines: [String]
    public var selections: [TextRange]
    /// Whether Xcode calls the buffer Swift source (`public.swift-source`).
    public var isSwift: Bool

    public init(lines: [String], selections: [TextRange] = [], isSwift: Bool) {
        self.lines = lines
        self.selections = selections
        self.isSwift = isSwift
    }

    public var text: String { lines.joined() }

    /// The text inside the selections, or nil when nothing is selected. The end of a range is
    /// exclusive, columns count UTF-16 units as NSString does, and positions past the end are clamped.
    public var selectedText: String? {
        let pieces = selections.compactMap(piece)
        let text = pieces.joined()
        return text.allSatisfy(\.isWhitespace) ? nil : text
    }

    private func piece(_ range: TextRange) -> String? {
        guard !lines.isEmpty, range.start.line < lines.count else { return nil }
        let lastLine = min(range.end.line, lines.count - 1)
        guard range.start.line <= lastLine else { return nil }
        var out = ""
        for index in range.start.line...lastLine {
            let line = lines[index] as NSString
            let from = index == range.start.line ? min(max(range.start.column, 0), line.length) : 0
            let to = index == range.end.line ? min(max(range.end.column, from), line.length) : line.length
            out += line.substring(with: NSRange(location: from, length: to - from))
        }
        return out.isEmpty ? nil : out
    }
}
