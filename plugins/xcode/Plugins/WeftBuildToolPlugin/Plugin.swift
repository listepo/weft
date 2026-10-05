import Foundation
import PackagePlugin

/// Generates SwiftUI from every `.weft` file of a target while it builds. The output goes to the
/// plugin's work directory, so it is never committed and always matches its screen.
@main
struct WeftBuildToolPlugin: BuildToolPlugin {
    func createBuildCommands(context: PluginContext, target: Target) async throws -> [Command] {
        guard let target = target as? SourceModuleTarget else { return [] }
        let documents = target.sourceFiles.map(\.url).filter { $0.pathExtension == "weft" }
        return documents.map {
            weftCommand(
                for: $0, tool: try? context.tool(named: "weft").url,
                root: context.package.directoryURL, workDirectory: context.pluginWorkDirectoryURL)
        }.compactMap { $0 }
    }
}

#if canImport(XcodeProjectPlugin)
    import XcodeProjectPlugin

    extension WeftBuildToolPlugin: XcodeBuildToolPlugin {
        func createBuildCommands(context: XcodePluginContext, target: XcodeTarget) throws -> [Command] {
            let documents = target.inputFiles.map(\.url).filter { $0.pathExtension == "weft" }
            return documents.map {
                weftCommand(
                    for: $0, tool: try? context.tool(named: "weft").url,
                    root: context.xcodeProject.directoryURL, workDirectory: context.pluginWorkDirectoryURL)
            }.compactMap { $0 }
        }
    }
#endif

/// One command per screen. The inputs are the screen, the `weft.json` that applies to it and the
/// catalog and token files that names, and the output is the one file `weft swiftui` writes, so a
/// change to any input regenerates that screen and nothing else.
private func weftCommand(for document: URL, tool: URL?, root: URL, workDirectory: URL) -> Command? {
    guard let tool else { return nil }
    let project = WeftProject.nearest(above: document, within: root)
    // One folder per source directory: two screens with the same file name in different
    // directories must not write the same output.
    let relative = relativePath(of: document.deletingLastPathComponent(), from: root)
    let outDirectory = relative.isEmpty ? workDirectory : workDirectory.appendingPathComponent(relative)
    let output = outDirectory.appendingPathComponent(document.deletingPathExtension().lastPathComponent + ".swift")
    // The project is always explicit: without it the tool would search above the package and find
    // a `weft.json` that was not declared as an input.
    let projectArguments = project.map { ["--project", $0.file.path] } ?? ["--no-project"]
    return .buildCommand(
        displayName: "Weft: generate SwiftUI from \(document.lastPathComponent)",
        executable: tool,
        arguments: ["swiftui", document.path] + projectArguments + ["--out-dir", outDirectory.path],
        inputFiles: [document] + (project.map { [$0.file] + $0.resources } ?? []),
        outputFiles: [output]
    )
}

private func relativePath(of directory: URL, from root: URL) -> String {
    let directory = directory.standardizedFileURL.resolvingSymlinksInPath().path
    let root = root.standardizedFileURL.resolvingSymlinksInPath().path
    guard directory.hasPrefix(root) else { return "" }
    return String(directory.dropFirst(root.count)).trimmingCharacters(in: CharacterSet(charactersIn: "/"))
}
