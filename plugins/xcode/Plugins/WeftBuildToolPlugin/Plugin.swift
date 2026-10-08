import Foundation
import PackagePlugin

/// Generates SwiftUI from every `.weft` file of a target while it builds. The output goes to the
/// plugin's work directory, so it is never committed and always matches its screen.
@main
struct WeftBuildToolPlugin: BuildToolPlugin {
    func createBuildCommands(context: PluginContext, target: Target) async throws -> [Command] {
        guard let target = target as? SourceModuleTarget else { return [] }
        let documents = target.sourceFiles.map(\.url).filter { $0.pathExtension == "weft" }
        return try weftCommands(
            for: documents, target: target.name, tool: try? context.tool(named: "weft").url,
            root: context.package.directoryURL, workDirectory: context.pluginWorkDirectoryURL)
    }
}

#if canImport(XcodeProjectPlugin)
    import XcodeProjectPlugin

    extension WeftBuildToolPlugin: XcodeBuildToolPlugin {
        func createBuildCommands(context: XcodePluginContext, target: XcodeTarget) throws -> [Command] {
            let documents = target.inputFiles.map(\.url).filter { $0.pathExtension == "weft" }
            return try weftCommands(
                for: documents, target: target.displayName, tool: try? context.tool(named: "weft").url,
                root: context.xcodeProject.directoryURL, workDirectory: context.pluginWorkDirectoryURL)
        }
    }
#endif

struct WeftPluginError: Error, CustomStringConvertible {
    let description: String
}

/// One command per screen, and one for the `WeftTokens.swift` the screens of a project share. A
/// target is one Swift module, so it can hold the shared tokens of one project only.
private func weftCommands(
    for documents: [URL], target: String, tool: URL?, root: URL, workDirectory: URL
) throws -> [Command] {
    guard let tool else { return [] }
    let screens = documents.map { ($0, WeftProject.nearest(above: $0, within: root)) }
    var shared: [URL: WeftProject] = [:]
    for case let (_, project?) in screens where project.sharesTokens {
        shared[project.file] = project
    }
    if shared.count > 1 {
        let files = shared.keys.map(\.path).sorted().joined(separator: ", ")
        throw WeftPluginError(
            description:
                "Weft: the screens of target \(target) use \(shared.count) projects with shared tokens (\(files)), "
                + "but one target holds one WeftTokens. Set \"export\": { \"swiftui\": { \"sharedTokens\": false } } "
                + "in all but one weft.json, or move their screens to separate targets.")
    }
    var commands = screens.map { weftCommand(for: $0.0, project: $0.1, tool: tool, root: root, workDirectory: workDirectory) }
    if let project = shared.values.first {
        commands.append(tokensCommand(for: project, tool: tool, workDirectory: workDirectory))
    }
    return commands
}

/// The tokens file goes to the top of the work directory, once per target: only the project and the
/// files it names are inputs, so a screen edit does not regenerate it.
private func tokensCommand(for project: WeftProject, tool: URL, workDirectory: URL) -> Command {
    .buildCommand(
        displayName: "Weft: generate WeftTokens.swift from \(project.file.lastPathComponent)",
        executable: tool,
        // The work directory holds only this plugin's outputs, which every rebuild replaces.
        arguments: ["swiftui-tokens", "--project", project.file.path, "--out-dir", workDirectory.path, "--force"],
        inputFiles: [project.file] + project.resources,
        outputFiles: [workDirectory.appendingPathComponent("WeftTokens.swift")]
    )
}

/// One command per screen. The inputs are the screen, the `weft.json` that applies to it and the
/// catalog and token files that names, and the output is the one file `weft swiftui` writes, so a
/// change to any input regenerates that screen and nothing else.
private func weftCommand(for document: URL, project: WeftProject?, tool: URL, root: URL, workDirectory: URL)
    -> Command
{
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
        arguments: ["swiftui", document.path] + projectArguments + ["--out-dir", outDirectory.path, "--force"],
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
