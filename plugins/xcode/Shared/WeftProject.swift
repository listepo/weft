import Foundation

/// The `weft.json` that applies to a document (SPEC §10.1), read only as far as the plugins need:
/// which files the generators will read, and where a tool setting sends its output.
struct WeftProject {
    static let fileName = "weft.json"

    let file: URL
    private let members: [String: Any]

    var directory: URL { file.deletingLastPathComponent() }

    /// The first `weft.json` in the document's directory or above it, but never above `root`: the
    /// build must not depend on files outside the package or project it declared as inputs.
    static func nearest(above document: URL, within root: URL) -> WeftProject? {
        let root = root.standardizedFileURL.resolvingSymlinksInPath()
        var directory = document.standardizedFileURL.resolvingSymlinksInPath().deletingLastPathComponent()
        while directory.path.hasPrefix(root.path) {
            let candidate = directory.appendingPathComponent(fileName)
            if FileManager.default.fileExists(atPath: candidate.path) {
                return WeftProject(file: candidate)
            }
            let parent = directory.deletingLastPathComponent()
            if parent == directory { break }
            directory = parent
        }
        return nil
    }

    private init(file: URL) {
        self.file = file
        // A project the tool cannot parse is still passed to it, which reports the problem.
        let data = try? Data(contentsOf: file)
        members = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]
    }

    /// The catalog and token files `weft.json` names. The generators read them, so a change to one
    /// must rebuild the screens that use the project.
    var resources: [URL] {
        let names = (members["tokens"] as? [String] ?? []) + [members["catalog"] as? String].compactMap { $0 }
        return names.map { directory.appendingPathComponent($0) }
    }

    /// A string setting such as `export.swiftui.outDir`.
    func setting(_ path: [String]) -> String? {
        value(path) as? String
    }

    /// Whether `weft swiftui` reads the tokens from the shared `WeftTokens.swift`: the tool's
    /// default for a project is yes.
    var sharesTokens: Bool {
        // `JSONSerialization` gives a JSON boolean as an `NSNumber`, which `as? Bool` also accepts
        // for 0 and 1: check the type so a number does not pass for a flag.
        guard let flag = value(["export", "swiftui", "sharedTokens"]) as? NSNumber,
            CFGetTypeID(flag) == CFBooleanGetTypeID()
        else { return true }
        return flag.boolValue
    }

    private func value(_ path: [String]) -> Any? {
        var node: Any? = members
        for key in path { node = (node as? [String: Any])?[key] }
        return node
    }
}
