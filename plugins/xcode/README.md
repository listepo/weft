# Weft for Xcode and SwiftPM

A build tool plugin, a command plugin and a Source Editor Extension that bring `weft swiftui` and `weft import-swiftui` into Xcode, and a script that registers the weft MCP server with Xcode's agents. The guide is [docs/xcode-plugin.md](../../docs/xcode-plugin.md).

| Path | What it is |
| --- | --- |
| `Package.swift`, `Plugins/`, `Shared/` | The SwiftPM package: plugins `WeftBuildTool` and `WeftCommands`. `Shared/WeftProject.swift` is linked into both plugin targets, because a plugin cannot depend on a library target. |
| `Artifacts/` | The `weft` artifact bundle for Apple Silicon, built by `moon run xcode-plugin:bundle` and not committed. |
| `Examples/` | `PackageSample` (SwiftPM) and `XcodeSample` (an XcodeGen spec), each building the corpus login screen and a review screen through the plugin, with one shared `WeftTokens.swift` and `RatingView`, the view the app writes for the project's own `rating` kind. |
| `editor-extension/` | The Source Editor Extension: `project.yml` (XcodeGen), `App/`, `Extension/`, and `Core/`, a Swift package with the testable part. |
| `scripts/` | `artifactbundle.ts` builds the bundle; `xcode-agents.ts` registers the MCP server and the guide with Xcode's agents. |

`moon run xcode-plugin:test` runs the tests; they skip with a message without Xcode.
