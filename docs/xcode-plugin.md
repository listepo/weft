# Weft in Xcode

`plugins/xcode` brings the [SwiftUI generator and importer](cli.md) into Xcode and SwiftPM, in four parts. They all run the same `weft` program, packed as a SwiftPM artifact bundle for Apple Silicon (arm64 macOS only; Intel Macs are not supported).

| Part | What it does |
| --- | --- |
| Build tool plugin `WeftBuildTool` | Every `.weft` file of a target becomes SwiftUI while the target builds. The generated file is never committed. |
| Command plugin `WeftCommands` | `swift package weft export` and `weft import` convert one screen or view and write the result into the package. |
| Source Editor Extension | Editor menu commands in Xcode: SwiftUI selection to Weft, Weft screen to SwiftUI, validate a Weft screen. |
| Xcode's agents | A script that registers the weft MCP server and the agent guide with the coding agents built into Xcode. |

## Set up

Make the clone ready as in the [tour](tour.md), then build the artifact bundle. It is a build product (`plugins/xcode/Artifacts/`, not committed):

```console
$ moon run xcode-plugin:bundle
wrote plugins/xcode/Artifacts/weft.artifactbundle (weft 0.1.0)
```

The task builds `weft` in release mode for `aarch64-apple-darwin` and writes the bundle's `info.json` (schema 1.0, one `arm64-apple-macosx` variant). `Package.swift` points at it with `binaryTarget(path:)`. Publishing the bundle on GitHub Releases, so that others can use `binaryTarget(url:checksum:)`, waits for the repository to have a remote.

## The build tool plugin

Add the package and the plugin to a target. `Examples/PackageSample` is a complete package:

```swift
dependencies: [.package(path: "../..")],
targets: [
    .target(name: "Screens", plugins: [.plugin(name: "WeftBuildTool", package: "xcode")])
]
```

(`package:` is the package's identity, which for a path dependency is its folder name; a dependency by URL uses the repository name.) With a `login.weft` in `Sources/Screens/`, the target gets `LoginScreen`, `LoginModel` and the rest of what `weft swiftui` writes, and `swift build` runs it. The plugin reads the nearest `weft.json` (in the file's folder or above it, up to the package), so the project's catalog and tokens apply. It declares the screen, the `weft.json` and the catalog and token files that names as inputs, and the generated file as the output, so an unchanged build runs nothing and a change to any of them regenerates that screen alone. A screen that does not validate fails the build with the validator's diagnostics (`login.weft:5:7 W401 …`). `export.swiftui.outDir` has no effect here, because the output goes into the build folder.

An Xcode project uses the same plugin: add the package, then under the target's Build Phases open "Run Build Tool Plug-ins" and add `WeftBuildTool`. `Examples/XcodeSample` does it with an XcodeGen spec (`xcodegen generate` writes the project, which is not committed). The `.weft` file must be in the target, where Xcode copies it as a resource. Xcode asks you to trust a package plugin the first time; a build without the UI passes `-skipPackagePluginValidation`.

## The command plugin

```console
$ swift package --allow-writing-to-package-directory weft export Sources/Screens/login.weft
$ swift package --allow-writing-to-package-directory weft import Sources/Screens/Hosting.swift --out-dir Imported
```

`export` writes `<screen>.swift`, `import` writes `<view>.weft`. Names are relative to the package. The output goes to `--out-dir`, else the project's `export.swiftui.outDir` or `import.swiftui.outDir`, else next to the input. The plugin declares the permission `writeToPackageDirectory`, so SwiftPM refuses without `--allow-writing-to-package-directory` and states why; Xcode asks in a dialog. It refuses to replace a file that exists unless you pass `--force`, so an import cannot silently overwrite the screen an export started from. Losses of an import are printed as the importer prints them.

## The Source Editor Extension

`plugins/xcode/editor-extension` holds a small macOS app that hosts the extension, an XcodeGen spec for it (`project.yml`) and a Swift package, `Core`, with everything the commands do. The Editor menu gets three commands:

| Command | In | What it does |
| --- | --- | --- |
| SwiftUI Selection to Weft | a Swift file | Imports the selection, or the whole file when nothing is selected, and copies the screen to the clipboard. A selection that is only a view expression is wrapped in a view first. The losses are listed in the message. |
| Weft Screen to SwiftUI | a `.weft` file | Copies the generated SwiftUI to the clipboard. |
| Validate Weft Screen | a `.weft` file | Checks the buffer strictly against the core catalog and shows the diagnostics. |

XcodeKit lets a command change only the buffer it runs in, and the only way to show a message is an error, so conversions go to the clipboard and every command reports through that message. Nothing edits your file.

**Why the extension runs the `weft` program.** The extension is sandboxed (Apple's template turns the sandbox on). Two ways to run the conversion in it were considered. The WebAssembly core cannot do it: it has no SwiftUI generator or importer, because the importer's grammar is C and does not build for `wasm32`. So the extension embeds the same `weft` binary the plugins use, in its bundle next to its executable, and starts it as a child process. Apple's rule for a helper tool in a sandboxed app is that it carries exactly two entitlements, `com.apple.security.app-sandbox` and `com.apple.security.inherit`, and the build signs it that way (`scripts/embed-weft.sh`). A test starts it from the sandboxed host app and converts a screen. See [research §12](../research.md) for the sources.

**What it cannot do.** XcodeKit gives a command the buffer's text and type but no file path, so the extension cannot find a `weft.json`: it uses the core catalog and the default tokens, and cannot use your project's own components or tokens. It cannot read your files at all.

Build it, signed for this Mac only (ad hoc):

```console
$ moon run xcode-plugin:bundle
$ cd plugins/xcode/editor-extension && xcodegen generate
$ xcodebuild -project WeftEditor.xcodeproj -scheme WeftEditor -configuration Release -derivedDataPath build
```

What you do by hand, because it needs Xcode's UI or an Apple account:

1. Copy `build/Build/Products/Release/WeftEditor.app` to `/Applications` and open it once.
2. Turn the extension on in System Settings > General > Login Items & Extensions > Xcode Source Editor, then restart Xcode.
3. Look for the three commands in the Editor menu. Apple's page on creating an extension says development signing is required, and forum posts report that an ad-hoc ("Sign to Run Locally") extension may not be listed. If Xcode does not show the commands, set `DEVELOPMENT_TEAM` and `CODE_SIGN_IDENTITY` to your own development identity in `project.yml`.
4. To give it to others: sign the app and the extension with a Developer ID, enable the hardened runtime (already on), and notarize. None of that is done here.

## Xcode's agents

Xcode 26.3 and later run coding agents (Claude Agent, Codex) inside Xcode, and Apple documents that they can use MCP servers: you put the agent's configuration files in its folder under `~/Library/Developer/Xcode/CodingAssistant/` (`ClaudeAgentConfig`, `codex`, `gemini`), and those files affect the agent only when Xcode runs it. Xcode starts the agents with `CLAUDE_CONFIG_DIR` and `CODEX_HOME` pointing there, so registering Weft in your home folder (`claude mcp add`) does not reach them. This script runs the agents' own `mcp add` commands with the folder pointed at Xcode's, and copies the agent guide as a skill:

```console
$ node plugins/xcode/scripts/xcode-agents.ts            # Claude and Codex, whichever are installed
$ node plugins/xcode/scripts/xcode-agents.ts --dry-run  # print what it would do
```

It registers `node plugins/claude-code/dist/server.js` as `weft` (with the absolute path of the `node` that ran the script, because Xcode's `PATH` may not have one; Node 24.2 or later is required) and copies `plugins/claude-code/skills/spec` (the skill and `AGENT-SPEC.md`) to `skills/weft-spec` in each folder. Run it again after moving the clone. Gemini is not handled: its command line is not used here.

For project instructions, Apple names `AGENTS.md` and `CLAUDE.md` as the files to add hints to. In a project with Weft screens, add:

```markdown
Screens are Weft files (`*.weft`). Before writing or changing one, read AGENT-SPEC.md of the Weft repository
and check every change with the weft MCP tools (`weft_validate`, `weft_patch`). The SwiftUI is generated from
the screens at build time: change the screen, not the generated file.
```

Agents you run outside Xcode (Claude Code in a terminal, with the [Claude Code plugin](claude-code-plugin.md)) need nothing from this page, apart from Xcode's own MCP server, which gives them build and preview tools: turn on "Allow external agents to use Xcode tools" in Xcode > Settings > Intelligence, then `claude mcp add --transport stdio xcode -- xcrun mcpbridge`.

What is not known: whether an in-Xcode agent reads a project's `.mcp.json`, and whether the folder's `skills` directory is used exactly as Claude Code and Codex use theirs. Both are in the manual checks below. Xcode also has a Plug-ins row (Intelligence settings, Agents) that installs plug-ins bundling MCP servers and skills; the repository has no remote to install this one from yet.

## Tests

`moon run xcode-plugin:test` (also part of `moon ci`) runs, and skips with a message when `xcrun`, Xcode or the bundle is missing:

- the bundle's `info.json` and its arm64 binary;
- the SwiftPM sample builds through the build tool plugin, the generated file typechecks for iOS 17 and macOS 14, an unchanged build runs nothing, a token file change regenerates, and an invalid screen fails the build;
- the Xcode project sample builds for iOS 17 and macOS 14 through the plugin;
- the command plugin refuses without permission, round-trips a corpus screen, refuses to overwrite, and honours `weft.json`;
- the editor extension's core passes `swift test`, the project builds with `xcodebuild`, the extension and its helper carry the right entitlements, and the sandboxed host app runs the embedded `weft`;
- the agent script registers the server and the guide in a temporary folder.

## Manual checks

These need Xcode's UI and have not been run:

1. In Xcode, open `Examples/XcodeSample` (after `xcodegen generate`) and press Run. Xcode asks to trust the plugin; the app shows the login screen.
2. Edit `login.weft` in the project and build: the screen changes without a Swift file being touched.
3. Right-click the package in the project navigator and run the WeftCommands plugin; Xcode asks for permission to write.
4. Enable the extension as described above. In a Swift file with a view, select the view and run the SwiftUI Selection to Weft command; paste into a new file. Open a `.weft` file and run the other two commands. Check that the message appears and what Xcode shows for a command that succeeds.
5. Check that Xcode treats `.weft` as the type `dev.weft.screen` (the host app declares it), or whether the commands are offered only for Swift files.
6. Run the agent script, restart Xcode, and in the coding assistant check that the `weft` MCP server is listed and that the agent finds the `weft-spec` skill. Try a project `.mcp.json` as well.

## Follow-ups

- Publish `weft.artifactbundle` on GitHub Releases and switch `Package.swift` to `binaryTarget(url:checksum:)` when the repository has a remote.
- Sign and notarize the editor extension for distribution.
- Make diagnostics of the build tool plugin appear as Xcode issues (the CLI prints `file:line:col code message`, which Xcode does not parse as an error line).
