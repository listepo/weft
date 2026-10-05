# Research

The question: what open format should describe user interfaces so that AI agents can read, write, validate and edit them — something between JSON and HTML. This file holds the prior art, the comparison, the chosen direction, and what the prototype has measured so far.

Every external fact carries its primary source and the date it was checked. Statements about what suits language models are design hypotheses; the benchmark (`bench/`, task T8) is what confirms or refutes them. Measurements quoted here come from this repository and name the file they come from.

## 1. Existing formats

| Format | What it describes | Fact | Source (checked) |
| --- | --- | --- | --- |
| Design Tokens (DTCG) | Design values: color, dimension, typography, themes, aliases | First stable version 2025.10, published 2025-10-28 | https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/ (2026-10-03) |
| WAI-ARIA 1.2 | Roles, states and properties of accessible UI | W3C Recommendation of 2023-06-06; "an ontology of roles, states, and properties" | https://www.w3.org/TR/wai-aria-1.2/ (2026-10-03) |
| ARIA Authoring Practices Guide | Keyboard interaction and ARIA semantics per widget | About 30 patterns (tabs, dialog, combobox, grid, menu, …), each with keyboard and role/state guidance | https://www.w3.org/WAI/ARIA/apg/patterns/ (2026-10-03) |
| JSX | Markup inside JavaScript | "A syntax extension for JavaScript"; JSX turns into JavaScript | https://react.dev/learn/writing-markup-with-jsx (2026-10-03) |
| Lit templates | Web component templates | Written as JavaScript template literals tagged with `html`, with JavaScript expressions | https://lit.dev/docs/templates/overview/ (2026-10-03) |
| Svelte | Component files | A compiler turns components written in HTML, CSS and JavaScript into JavaScript | https://svelte.dev/docs/svelte/overview (2026-10-03) |
| MDX | Prose with components | Markdown plus JSX, JavaScript expressions and ESM import/export; compiled to JavaScript | https://mdxjs.com/docs/what-is-mdx/ (2026-10-03) |
| Storybook CSF | Named component states as examples | CSF 3 is current: default export holds metadata, each named export is a story, `args` are named inputs; "an open standard based on ES6 modules" | https://storybook.js.org/docs/api/csf (2026-10-03) |
| Custom Elements Manifest | Contract of a web component | Schema 2.1.0, released 2024-05-06; covers tag name, attributes, properties, events, CSS variables and parts | https://github.com/webcomponents/custom-elements-manifest (2026-10-03) |
| A2UI | Agent-generated UI as JSON | Flat component list with id references, client-side catalog of trusted components, streaming messages; v0.9.1 current, v1.0 release candidate | https://a2ui.org/specification/v0.9-a2ui/ , https://a2ui.org/specification/v1.0-a2ui/ (2026-10-03) |
| json-render | Agent-generated UI as JSON or YAML | Apache-2.0; catalog of components and actions defined with Zod (`defineCatalog`); flat spec `{ root, elements }` with child id lists; YAML wire format with a streaming parser; renderers for React, Vue, Svelte, Solid, React Native and others | https://github.com/vercel-labs/json-render (2026-10-03) |
| MCP Apps | Delivery of UI from MCP servers | SEP-1865: `ui://` resources carrying sandboxed HTML (`text/html;profile=mcp-app`); accepted 2026-01-26 | https://modelcontextprotocol.io/seps/1865-mcp-apps-interactive-user-interfaces-for-mcp (2026-10-03) |

Two observations follow from the table. JSX, Lit, Svelte and MDX are code: a document in them is a program, so it cannot be validated against a closed vocabulary or rendered from an untrusted source without executing it. A2UI and json-render are data constrained by a catalog, which is exactly the trust boundary an agent-facing format needs, but both encode the tree as a flat map of ids.

## 2. What makes a format convenient for language models

These are the properties Weft was designed for. Each is a hypothesis; the column on the right says how the prototype tests it.

| Property | Why it should help a model | How it is tested |
| --- | --- | --- |
| Closed vocabulary, one meaning per element | No guessing what a `div` with classes means | Strict validation rejects unknown kinds (`W401`, `W2xx`) |
| One way to say a thing | Fewer variants to learn and to compare | Canonical form: `serialize(parse(x))` is byte-stable; tested on 10 000 generated documents |
| Explicit hierarchy | Nesting is visible locally instead of reconstructed from id references | Token cost and edit success against A2UI (T8) |
| Enumerated states | `state="loading"` instead of flag combinations | Catalog declares states; `W203` on undeclared ones |
| Named slots | The model sees where content may go, the validator can check it | `W207`, `W208` |
| Stable ids on every element | Edits and actions can address an element | Patches by id (SPEC §7) |
| No code | Bindings and named actions only; safe to render from an untrusted source | Injection tests in the renderer and JSX generator |
| Diagnostics written for repair | Path, expectation and the nearest valid value make one repair cycle enough | Repair-cycle validity target in T8 |
| Low token cost | Cheaper context, more room for the task | `bench/REPORT.md` |

## 3. Validation approaches

| Approach | For | Against | Source (checked) |
| --- | --- | --- | --- |
| JSON Schema (current version 2020-12) | Standard; accepted by provider structured-output modes | Context rules ("`tab` only inside `tabs`") are awkward; error messages are generic | https://json-schema.org/specification (2026-10-03) |
| Custom schema language | Exact nesting rules | Its own tooling; nobody knows it | — |
| Type system (Zod, TypeScript) | One source for types, runtime checks and JSON Schema export | Tied to the TypeScript ecosystem | json-render uses it: https://github.com/vercel-labs/json-render (2026-10-03) |

Chosen: Zod as the source of truth (`packages/core/src/model.ts`), JSON Schema as an export (`catalogJsonSchema`, `documentJsonSchema`), and a separate semantic layer for what neither expresses well — unique ids, parent and child rules, loop scope, tokens, actions, numeric ranges. Diagnostics are an API with stable codes (SPEC §6.2).

## 4. How agents interact with UI today

| Channel | For agents | Against | Source (checked) |
| --- | --- | --- | --- |
| Screenshots | Works on anything; sees the real rendering | High token cost (image tokens); coordinates are approximate and break on layout changes | https://playwright.dev/mcp/snapshots (2026-10-03) |
| DOM and selectors | Exact and complete | Large and noisy; selectors break | — |
| Accessibility tree (Playwright MCP snapshot) | "A structured tree of accessible elements with refs for interaction"; text only, low cost, deterministic | Depends on the quality of the page's markup; read-only | https://playwright.dev/mcp/snapshots (2026-10-03) |
| Figma API | `GET /v1/files/:key` and `GET /v1/files/:key/nodes` return the node tree with layout and auto-layout properties, component metadata and annotations | Describes design intent, not a running UI; layers are not semantics | https://developers.figma.com/docs/rest-api/file-endpoints/ (2026-10-03) |

Agents read UI through the accessibility tree and write it as JSX or HTML: two vocabularies for one object. Weft declares ARIA roles in the document, so what an agent writes can be compared with what a user of assistive technology — or a browsing agent — reads back. The prototype does this comparison in Chromium (`packages/render-react`).

## 5. Compared approaches

| Approach | For agents | Against |
| --- | --- | --- |
| A. Raw HTML or JSX | Most familiar syntax; full expressiveness | Cannot be validated or rendered safely; ambiguous semantics |
| B. Catalog JSON (A2UI, json-render) | Strict validation, streaming, id-addressed patches, trust boundary | Verbose; hierarchy is reconstructed from references |
| C. Accessibility snapshot | Compact; carries actions | Read-only: no layout, tokens or slots; no way back to code |
| D. Component manifest (Custom Elements Manifest, CSF) | Exact component contract | Describes components, not screens |
| E. Strict markup with a canonical JSON form (Weft) | Reads like HTML, validates like JSON, patches by id | New syntax that has to prove its benefit |

## 6. Directions considered

1. **A profile on top of A2UI** — A2UI as is, plus ARIA semantics and DTCG token references. Low risk; verbosity and the flat graph remain.
2. **Strict markup with canonical JSON** — one semantic model, two serializations: markup for models, JSON for tools.
3. **A writable accessibility snapshot** — extend the snapshot format so it can be written as well as read. Reading and writing coincide, but it is web-only and tied to one tool's format.

## 7. Decision

Direction 2, named Weft. It takes the catalog and trust boundary from A2UI and json-render, the component contract shape from Custom Elements Manifest, roles and states from WAI-ARIA and the APG patterns, design values from DTCG, and nested markup as the model-facing surface. Because roles live in the document, an agent reads and writes one vocabulary.

The main risk was that markup is not measurably better than catalog JSON for models. The stop criterion: if Weft gives neither 25% fewer tokens nor 10 points more successful edits than A2UI JSON, the creator decides whether to continue.

## 8. What the prototype has shown

Implemented (details in `done.md`): specification 0.1 (`SPEC.md`), parser, serializer, validator and CLI (`packages/core`), the core catalog of 29 components and a DTCG token loader (`packages/catalog`), a React renderer checked in Chromium (`packages/render-react`), importers from accessibility snapshots and DOM (`packages/from-aria`), a JSX generator (`packages/to-jsx`), patches and an MCP server with six tools (`packages/mcp`), a forward-compatibility suite (`compat/`), and a corpus of 12 screens in four formats with a benchmark harness (`corpus/`, `bench/`).

Measured (`bench/REPORT.md`, proxy tokenizer o200k_base, 12 screens):

| Format | Tokens | Weft as a share |
| --- | ---: | ---: |
| Weft | 2529 | — |
| HTML | 2536 | 100% |
| JSX | 2980 | 85% |
| A2UI v0.9 | 7734 | 33% |

- Weft needs 67% fewer tokens than A2UI JSON, so the token half of the stop criterion is met. Against HTML there is no size advantage; any advantage there has to come from validation and edit accuracy.
- The tokenizer is a proxy, not Claude's. Exact counts need an API key.
- A2UI's basic catalog v0.9 has no table, switch, menu, link, alert or form submit; the corpus approximates them, so the comparison is partly approximate (`corpus/README.md`).
- Writing the corpus exposed gaps in the first draft of the spec (bound text on buttons, submit buttons, empty states, numeric ranges); they were closed in T9.
- Round trips hold: document → render → accessibility snapshot or DOM → document keeps structure, roles and states on all 12 screens, and ids through DOM. Generated JSX renders the same HTML as the reference renderer on 12 screens and 29 catalog examples.

Not yet measured: how well real models generate and edit Weft compared with the baselines — first-try validity, validity after one repair cycle, and edit success. That is T8 in `plan.md`.

## 9. Open questions

| Question | State | Where |
| --- | --- | --- |
| Format versioning | Answered for 0.x: minor versions only add; lenient readers warn and keep unknown content; catalog changes are classified major, minor or none | SPEC §8, `compat/`, `diffCatalogs` |
| Extensibility without breaking agents | Answered: `x-<vendor>-` extensions with an ARIA fallback role; unknown elements render as `group` | SPEC §8 |
| Mapping to code and back | Partly: React renderer and JSX generator; SwiftUI generator and importer (T34); importers from DOM and snapshots with a documented loss table | SPEC §9, `roadmap.md` |
| Streaming and incremental generation | Open. Flat id lists in A2UI and json-render exist for progressive rendering; nested markup has to show it can do the same | `roadmap.md` |
| Host capabilities | Specified (a host advertises its catalogs), not implemented | SPEC §8, `roadmap.md` |
| Layout without becoming CSS | Open: only `stack` and `grid` | `roadmap.md` |
| Who keeps a registry of extension catalogs | Open | `roadmap.md` |
| Figma as a source | Open | `roadmap.md` |

## 10. Project files (T31)

| Decision | Basis | Source (checked) |
| --- | --- | --- |
| `weft.json` is found by walking up from the screen; an explicit argument wins | The `tsconfig.json` lookup: the compiler searches the current directory, then each parent, unless a project is named | https://www.typescriptlang.org/docs/handbook/tsconfig-json.html (2026-10-05) |
| Token files merge in order and aliases resolve after the merge | The DTCG resolver module merges token sets in order, later sets override earlier ones, and references are resolved on the merged result | https://www.designtokens.org/tr/2025.10/resolver/ (2026-10-05) |
| The data schema is a JSON Schema 2020-12 subset (`type`, `properties`, `additionalProperties`, `items`, boolean schemas) | The schema format providers and editors already accept; the unsupported keywords are reported, not ignored | https://json-schema.org/draft/2020-12/json-schema-core (2026-10-05) |
| Not JSON Type Definition | JTD (RFC 8927) is closed by default and simpler, but fewer tools and models know it | https://www.rfc-editor.org/rfc/rfc8927 (2026-10-05) |
| An object schema with `properties` and no `additionalProperties` is closed | A deliberate deviation from JSON Schema, where objects are open: a misspelled binding path must be an error, not an allowed extra member (SPEC §10.5) | — |
| Fragments (part B, proposal) | `docs/fragments-design.md`, with its own sources | — |

## 11. SwiftUI (T34)

| Decision | Basis | Source (checked) |
| --- | --- | --- |
| Swift source is parsed with tree-sitter 0.27.0 | The maintained Rust bindings of the incremental parser; the crate's latest release | https://crates.io/crates/tree-sitter, https://github.com/tree-sitter/tree-sitter (2026-10-05) |
| The grammar is tree-sitter-swift 0.7.4 | Released 2026-10-04, repository active and not archived; the grammar the tree-sitter ecosystem uses for Swift | https://crates.io/crates/tree-sitter-swift, https://github.com/alex-pinkus/tree-sitter-swift (2026-10-05) |
| Not oak-swift or devgen-tree-sitter-swift | oak-swift is at 0.0.11; devgen-tree-sitter-swift is a 2024 fork at 0.21.0 | https://crates.io/crates/oak-swift, https://crates.io/crates/devgen-tree-sitter-swift (2026-10-05) |
| The importer is not in WebAssembly builds | The grammar is C; `cargo build --target wasm32-unknown-unknown` fails in its `parser.c` (`stdlib.h` not found), so the importer sits behind the crate's default `import` feature and the generator builds alone | Built in this repository (2026-10-05) |
| The generated code targets iOS 17 and macOS 14 | `@Observable` is the model; `swiftc -typecheck` for iOS 16 rejects it ("'Observable()' is only available in iOS 17.0 or newer"), so there is no lower target to configure | https://developer.apple.com/documentation/observation/observable(), Xcode 27.0 / Swift 6.4 (2026-10-05) |

## 12. Xcode (T42)

Checked on 2026-10-05 with Xcode 27.0 (27A266a), Swift 6.4 and macOS 27.0.1. "Local" means read from that Xcode installation; the docs are Apple's JSON endpoints under `https://developer.apple.com/tutorials/data/documentation/<path>.json`.

| Decision or fact | Basis | Source (checked) |
| --- | --- | --- |
| SwiftPM plugins run a binary tool through an artifact bundle | `info.json` has `schemaVersion` `"1.0"` and `artifacts.<name>` with `type` `executable`, `version` and `variants[]` of `path` and `supportedTriples`; Apple Silicon macOS is `arm64-apple-macosx`; a plugin target lists the `binaryTarget` as a dependency and calls `context.tool(named:)` | swift-package-manager at tag `swift-6.4.0-RELEASE` (18da3eb): `Sources/PackageModel/ArtifactsArchiveMetadata.swift`, `Fixtures/Miscellaneous/Plugins/MyBinaryToolPlugin` (2026-10-05) |
| The plugin API is the `URL` one | With tools version 6.0 `Command.buildCommand(displayName:executable:arguments:environment:inputFiles:outputFiles:)` takes URLs; the `Path` overloads are deprecated | swift-package-manager `Sources/Runtimes/PackagePlugin/Protocols.swift` at the same tag (2026-10-05) |
| Xcode projects get the same plugins through `XcodeProjectPlugin` | `XcodeBuildToolPlugin.createBuildCommands(context: XcodePluginContext, target: XcodeTarget)` and `XcodeCommandPlugin.performCommand(context:arguments:)`; imported under `#if canImport(XcodeProjectPlugin)` | `XcodeProjectPlugin.swiftinterface` of Xcode 27.0 (local, 2026-10-05) |
| A command plugin writes to the package only with permission | `PluginPermission.writeToPackageDirectory(reason:)`; `swift package --allow-writing-to-package-directory`; built and run in this repository | swift-package-manager `Sources/Runtimes/PackageDescription/Target.swift`, `Sources/PackageManagerDocs/Documentation.docc/Plugins/EnableCommandPlugin.md` (2026-10-05) |
| The editor extension runs the `weft` binary, not the WebAssembly core | The WebAssembly build has no SwiftUI generator or importer (§11: the importer's grammar is C and does not build for `wasm32-unknown-unknown`; `weft-wasm` does not include `weft-swiftui`), so only the binary can do all three commands. WebAssembly does run in a bare `JSContext` on macOS 27 (a hand-written module returned 5), which does not change that | `crates/weft-wasm/Cargo.toml`; `JSContext` run locally (2026-10-05) |
| A helper tool in a sandboxed extension needs exactly two entitlements | "a child target must use exactly two App Sandbox entitlement keys: `com.apple.security.app-sandbox` and `com.apple.security.inherit`"; any other and "the system aborts the child process"; the parent is spawned with `posix_spawn` or `NSTask` and never has inherit set. The tool is embedded with a Copy Files phase to Executables (`Contents/MacOS`), with `CODE_SIGN_INJECT_BASE_ENTITLEMENTS` off | https://developer.apple.com/library/archive/documentation/Miscellaneous/Reference/EntitlementKeyReference/Chapters/EnablingAppSandbox.html, https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app (2026-10-05) |
| An ad-hoc signed helper with those two entitlements starts inside the sandbox | The host app, ad hoc signed, ran `weft` from the extension bundle as a child while `APP_SANDBOX_CONTAINER_ID` was set; the test is `editor-extension.test.ts` | Built and run in this repository (2026-10-05) |
| XcodeKit gives a command no file path | `XCSourceEditorCommandInvocation` has `commandIdentifier`, `buffer`, `cancellationHandler`; `XCSourceTextBuffer` has `contentUTI`, `tabWidth`, `indentationWidth`, `usesTabsForIndentation`, `lines`, `selections`, `completeBuffer`. So the extension cannot find a `weft.json` and converts with the core catalog and the default tokens | XcodeKit headers of the Xcode 27.0 SDK (local, 2026-10-05) |
| Commands are declared in `Info.plist` | `NSExtension` > `NSExtensionAttributes` > `XCSourceEditorExtensionPrincipalClass` and `XCSourceEditorCommandDefinitions` (identifier, name, class name); extension point `com.apple.dt.Xcode.extension.source-editor`; the Xcode template turns the sandbox on | `Xcode Source Editor Extension.xctemplate` of Xcode 27.0 (local); https://developer.apple.com/documentation/xcodekit/creating-a-source-editor-extension (2026-10-05) |
| Whether Xcode loads an ad-hoc signed extension | Apple's page says development signing is required for each target; forum posts say "Sign to Run Locally" extensions do not appear. **Unverified**: not run through Xcode's UI | https://developer.apple.com/documentation/xcodekit/creating-a-source-editor-extension; https://developer.apple.com/forums/thread/750811 (secondary) (2026-10-05) |
| Xcode 26.3 introduced agentic coding and MCP | The release notes say Xcode exposes its capabilities through the Model Context Protocol and add Claude Agent and Codex | https://developer.apple.com/documentation/xcode-release-notes/xcode-26_3-release-notes (2026-10-05) |
| Agents built into Xcode can use MCP servers, through their own configuration folders | "you can set a default model, add additional Model Context Protocol (MCP) servers, and create your own skills" by placing configuration files in `~/Library/Developer/Xcode/CodingAssistant/ClaudeAgentConfig`, `.../codex` or `.../gemini`; these "only affect agents when you launch them in Xcode". A Plug-ins row (Add Plug-in) installs plug-ins that bundle subagents, MCP servers and skills | https://developer.apple.com/documentation/xcode/extending-and-customizing-agents (2026-10-05) |
| Xcode points the agents at those folders with the agents' own variables | The framework holds "Set CLAUDE_CONFIG_DIR to Claude Agent config directory", "Override CODEX_HOME", and the file names `config.toml`, `settings.json`, `mcp-servers.json`. So the agents' own `mcp add` commands, run with the variable set, write what Xcode's agent reads (the formats are Claude Code's and Codex's). Inferred from strings, not from running Xcode's agents | `IDEIntelligenceAgents.framework` of Xcode 27.0 (local, `strings`, 2026-10-05) |
| Xcode's own MCP server is for agents run outside Xcode | Turn on Intelligence > Model Context Protocol > "Allow external agents to use Xcode tools", then `claude mcp add --transport stdio xcode -- xcrun mcpbridge` or `codex mcp add xcode -- xcrun mcpbridge`; the project must be open in Xcode | https://developer.apple.com/documentation/xcode/giving-external-agents-access-to-xcode (2026-10-05) |
| Project instructions are `AGENTS.md` and `CLAUDE.md` | The docs say to add hints about Xcode and the project to those files "in the location that the external agent uses". No Xcode-specific instructions file and no Cursor rules were found | https://developer.apple.com/documentation/xcode/giving-external-agents-access-to-xcode (2026-10-05) |
| Not found | A documented command for registering an MCP server with the in-Xcode agents; whether the in-Xcode agents read a project's `.mcp.json`; the schema of `~/Library/Developer/Xcode/CodingAssistant/mcp-servers.json`, which exists and holds an empty `mcpServers` object | Searched the pages above and the local framework (2026-10-05) |

## 13. Token modes and typography (T45)

Checked on 2026-10-05. The resolver module is the published report, not the editor's draft on the repository's `main` branch (that draft says "preview draft … do not implement").

| Decision or fact | Basis | Source (checked) |
| --- | --- | --- |
| The Resolver Module exists in 2025.10 | "Design Tokens Resolver Module 2025.10", a Final Community Group Report dated 28 October 2025, published with the Format 2025.10 Weft already reads | https://www.designtokens.org/TR/2025.10/resolver/ (2026-10-05) |
| Root of a resolver document | `version` (required, must be `2025.10`), `resolutionOrder` (required), `sets`, `modifiers`, `name`, `description`, `$schema`; the JSON Schema has `"version": { "const": "2025.10" }` and `"required": ["version", "resolutionOrder"]` | Resolver 2025.10 §Syntax; https://www.designtokens.org/schemas/2025.10/resolver.json (2026-10-05) |
| Sets | A set has a `sources` array of reference objects and inline token trees, merged in array order: the last occurrence of a token wins | Resolver 2025.10 §Sets |
| Modifiers and contexts | A modifier has a `contexts` map of name to an array of sources (merged like a set's) and an optional `default` that must be one of the contexts ("Tools MUST throw an error if the value is not present"); 0 contexts is an error, 1 context "SHOULD" be one; a context may reference a set but never a modifier | Resolver 2025.10 §Modifiers, §Contexts, §Default |
| Resolution order | `resolutionOrder` lists sets and modifiers, by reference (`#/sets/…`, `#/modifiers/…`) or inline with `name` and `type` (`set` or `modifier`, names unique); later entries override earlier ones; a modifier contributes only the context the input selects | Resolver 2025.10 §Resolution order, §Inline sets and modifiers |
| Aliases resolve after flattening | "Aliases MUST NOT be resolved until this step"; the flattened tree "behaves the same as if it were one source", so the existing loader runs on it unchanged | Resolver 2025.10 §Resolution logic |
| Inputs | An input is an object of modifier name to context name; a modifier without `default` needs one; inputs "SHOULD be case-insensitive". Weft has no input of its own, so it takes each modifier's `default`, else its first context (a Weft rule, SPEC §10.3) | Resolver 2025.10 §Inputs, §Input validation |
| Reference objects | `{ "$ref": … }` with a JSON Pointer; same-document pointers are required, files relative to the resolver and remote URLs optional; no circular references; nothing may point into `resolutionOrder`, and only `resolutionOrder` may point at a modifier; keys beside `$ref` override shallowly. Weft supports same-document and project-file references, not URLs | Resolver 2025.10 §Reference objects, §Invalid pointers, §Extending |
| File extension | "Users SHOULD use the `.resolver.json` file extension" | Resolver 2025.10 §File extension |
| Typography `letterSpacing` and `lineHeight` | `letterSpacing` is a dimension; `lineHeight` is a number, "a multiplier of the fontSize" | https://www.designtokens.org/TR/2025.10/format/ §Typography (2026-10-05) |
| SwiftUI letter spacing is `tracking(_:)` | View `tracking(_:)`, iOS 16 / macOS 13: "additional space, in points, that the view should add to each character cluster after layout", like CSS `letter-spacing`; `kerning(_:)` changes character offsets instead, and tracking wins when both are set | https://developer.apple.com/documentation/swiftui/view/tracking(_:), https://developer.apple.com/documentation/swiftui/text/tracking(_:) (2026-10-05) |
| SwiftUI line height on iOS 17 is `lineSpacing(_:)` | `lineSpacing(_:)` sets "the amount of space between the bottom of one line and the top of the next"; View `lineHeight(_:)` is iOS 26 / macOS 26 only, above the iOS 17 / macOS 14 target. So the spacing is the CSS line box (size × multiplier) minus the font's own line height, with half of it as vertical padding, the half-leading CSS puts above the first and below the last line | https://developer.apple.com/documentation/swiftui/view/linespacing(_:), https://developer.apple.com/documentation/swiftui/view/lineheight(_:) (2026-10-05) |
| A dynamic colour on iOS 17 / macOS 14 | SwiftUI has no light-and-dark `Color` initializer at this target; `UIColor(dynamicProvider:)` and `NSColor(name:dynamicProvider:)` give one that follows the appearance, wrapped by `Color(uiColor:)` / `Color(nsColor:)` | https://developer.apple.com/documentation/uikit/uicolor/init(dynamicprovider:), https://developer.apple.com/documentation/appkit/nscolor/init(name:dynamicprovider:) (2026-10-05) |
| The web follows the appearance with `prefers-color-scheme` | `@media (prefers-color-scheme: dark)` works for every token type, not only colours; `color-scheme` is Baseline widely available (since 2024-08-03); `light-dark()` is Baseline newly available (since 2024-05-13) and takes colours only, so it is not used | web-features 3.40.1 `color-scheme`, `light-dark` (https://unpkg.com/web-features/data.json); https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/light-dark (2026-10-05) |
| Figma modes | `VariableCollection.modes`, `defaultModeId`, `addMode(name)` (returns the mode id; throws "Limited to N modes only" on a plan that limits modes), `renameMode`, `removeMode`; `Variable.setValueForMode(modeId, value)` and `valuesByMode`. Modes need an Education, Professional, Organization or Enterprise plan | https://www.figma.com/plugin-docs/api/VariableCollection/, https://www.figma.com/plugin-docs/api/Variable/, https://help.figma.com/hc/en-us/articles/15343816063383-Modes-for-variables (2026-10-05); `@figma/plugin-typings` 1.140.0 |
| Penpot token themes | `TokenCatalog.themes`, `addTheme({ group, name })`, `addSet({ name, active })`; a `TokenTheme` has `group`, `name`, `active`, `activeSets`, `addSet`, `removeSet`; "At any time only one of the themes in a group may be active", so a group is a modifier and its themes its contexts. Among active sets "the latter has precedence" | `@penpot/plugin-types` 1.5.0 `index.d.ts` (`TokenCatalog`, `TokenSet`, `TokenTheme`) (2026-10-05) |
| Penpot theme activation | "When a TokenSet is activated or deactivated directly, all themes are disabled"; `TokenTheme.addSet` takes "a `TokenSet` or the id of a token set"; `TokenCatalog.themes` is "in creation order", so the first theme of a group is the one Weft reads as the default context; a set name "may contain a group path, separated by `/`" | `@penpot/plugin-types` 1.5.0 `index.d.ts` (2026-10-05) |
| Penpot marks no default theme | `TokenTheme` has `id`, `externalId` ("may exist if the theme was imported from an external tool"), `group`, `name`, `active`, `activeSets` and methods, and no plugin data or order field; `Library extends PluginData`, which has `getSharedPluginData(namespace, key)` and `setSharedPluginData`, so Weft records the default context there | `@penpot/plugin-types` 1.5.0 `index.d.ts` lines 5321-5380 (`TokenTheme`), 2606 (`Library`), 3409-3477 (`PluginData`) (2026-10-05) |
| Token names in a written resolver | A token or group name must not start with `$` or contain `{`, `}` or `.`; a path that breaks this is left out of the document the plugins return | https://www.designtokens.org/TR/2025.10/format/ §5.1.1 Character restrictions (2026-10-05) |
| Modifier names in `$ref` | A JSON Pointer writes `~` as `~0` and `/` as `~1`, in that order | RFC 6901 §3, §4, https://www.rfc-editor.org/rfc/rfc6901 (2026-10-05) |

## 14. Running only the checks a change affects (T54)

Checked on 2026-10-05 against the pinned tools: moon 2.5.6 (`moon run --help`, run in the repository), cargo-nextest 0.9.143, Vitest 5.0.3. "Measured" rows are experiments in the T54 worktree, reproducible with the commands given; the other rows cite the vendor's documentation.

| Decision or fact | Basis | Source (checked) |
| --- | --- | --- |
| moon's affected mode selects a task when a changed file matches its declared `inputs`, and can add graph relations with `--upstream` and `--downstream` (`none`, `direct`, `deep`) | "Any modified file matching the task's `inputs`" makes a task affected; relations are controlled by `--upstream` and `--downstream`; `moon run` pre-fills `--upstream=deep` and no downstream | https://moonrepo.dev/docs/concepts/affected, https://moonrepo.dev/docs/commands/run (2026-10-05); options listed by `moon run --help` of moon 2.5.6 |
| moon's affected mode cannot narrow this workspace | Measured: with one appended line in `packages/mcp/src/index.ts`, `moon query affected` lists all 21 projects and every `test` task, because the inherited `test` task declares `/packages/**/*` as an input (`.moon/tasks/all.yml`); with one line in `crates/weft-swiftui/src/lib.rs` and `--downstream deep` it lists every task that depends on `root:wasm`, though the module does not contain that crate | `moon query affected` and `moon query affected --downstream deep` of moon 2.5.6 in the worktree |
| Narrowing the declared inputs to fix that would break the cache | The packages' tests read sibling packages by path as well as by import (for example `packages/core/test/cases.ts` reads `packages/from-aria`, `bench/test/agent-spec.test.ts` reads `packages/catalog`), and a task hash covers only its own inputs, so dropping the coarse glob would serve stale results in the full check | `grep` of the test sources in the worktree; hash contents in `.moon/cache/hashes/*.json` |
| nextest selects tests by reverse dependency | `rdeps(name-matcher)` "includes tests in matching crates and all the crates that (possibly transitively) depend on" them; `=name` is the equality matcher, and a bare name is a glob; union is `\|`; `-E` takes the filterset | https://nexte.st/docs/filtersets/reference/ (2026-10-05) |
| `-E 'rdeps(=weft-swiftui)'` runs the crate and its dependents | Measured through `moon run root:rust-test -- -E "'rdeps(=weft-swiftui)'"`: 74 tests: 20 in `weft-swiftui`, 40 in `weft-cli` and 14 in `weft-snapshots`, the two crates that depend on it | cargo-nextest 0.9.143 in the worktree |
| moon passes the arguments after `--` to a shell unquoted | Measured: `moon run root:rust-test -- -E 'rdeps(=x)'` failed with a bash syntax error at `(`; the filterset therefore carries its own quotes | moon 2.5.6 in the worktree |
| Arguments after `--` are part of the task hash | Measured: `.moon/cache/hashes/*.json` of `mcp:test -- --changed HEAD` lists `--changed` and `HEAD` among the arguments, and its hash differs from the plain run's, so a narrowed run is never served as the full run | moon 2.5.6 in the worktree |
| Vitest runs the tests related to changed files | `--changed [since]` takes a commit or branch, compares against it (uncommitted changes when empty) and runs the tests affected; the module graph finds them across workspace packages (measured: a changed `packages/render-react/src/index.ts` ran the `@weft/mcp` files that import it); a data file read by a test without an import is not followed | https://vitest.dev/guide/cli.html, https://vitest.dev/config/changed (2026-10-05); measured in the worktree |
| The determinator maps changed files to cargo packages but is a library | The crate "figures out what packages in a Rust workspace changed between two commits", picks "the package nearest to the file", treats a file outside every package as a full rebuild, has no command-line program; latest 0.12.0 | https://docs.rs/determinator/latest/determinator/ (2026-10-05), https://github.com/guppy-rs/guppy |
| Rejected: wrapping the determinator in a Rust program | It would add a crate and a build for a mapping of about ten lines (`cargo metadata --no-deps` gives each package's directory and workspace dependencies); its extra value, comparing dependency versions between two lockfiles, is replaced by running the full check when a lockfile or root manifest changes | decision, T54 |

## 15. Richer controls (T50)

Checked on 2026-10-05. Apple pages were read as their documentation JSON (`developer.apple.com/tutorials/data/documentation/...`); the Playwright row is a measurement with `playwright-core` 1.63.0 on the pinned Chromium, not a document.

| Decision or fact | Basis | Source (checked) |
| --- | --- | --- |
| `slider` is the ARIA `slider` role, drawn on the web by `<input type="range">` | The slider pattern requires `aria-valuenow`, `aria-valuemin`, `aria-valuemax` (and `aria-valuetext` when the number is not friendly) and the arrow, Home, End and Page keys; HTML-AAM maps `input type=range` to `slider`, so the native element carries all of it | https://www.w3.org/WAI/ARIA/apg/patterns/slider/, https://www.w3.org/TR/html-aam-1.0/ (2026-10-05) |
| Range defaults and clamping | `min` 0, `max` 100, `step` 1; a value outside the range is clamped to the nearest bound; with `max` below `min` the control cannot be used and takes `min`. Weft writes the sanitized value itself so every target shows the same number | https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/range (2026-10-05) |
| A range input rounds an off-step value | `<input type="range" min="0" max="10" step="2" value="3">` reports value `4` and the snapshot line `slider "Vol": "4"` | Measured with Playwright 1.63.0 `ariaSnapshot()` on the pinned Chromium (2026-10-05) |
| `stepper` is the ARIA `spinbutton` role, drawn by `<input type="number">` with two pointer-only buttons | The spinbutton pattern requires `aria-valuenow` (and min/max when there are bounds), arrow keys step, Home and End jump to the bounds, and "the text field is usually the only focusable component", so the buttons are visual aids: `tabindex="-1"` and `aria-hidden="true"`; HTML-AAM maps `input type=number` to `spinbutton` | https://www.w3.org/WAI/ARIA/apg/patterns/spinbutton/, https://www.w3.org/TR/html-aam-1.0/ (2026-10-05) |
| `date-picker`: one kind for a date, a time or both | HTML has three input types with fixed value formats: `date` is `yyyy-mm-dd`, `datetime-local` is a date and time without a time zone, `time` is a time; each takes `min`, `max` and `step`. SwiftUI's `DatePicker` shows any of them through `displayedComponents` | https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/date, https://developer.apple.com/documentation/swiftui/datepicker (2026-10-05) |
| A date or colour input has no ARIA role | HTML-AAM gives `input type=date`, `datetime-local` and `color` "No corresponding role"; Playwright's accessibility snapshot reports all of `date`, `time`, `datetime-local` and `color` as `textbox` (`textbox "Due": 2026-10-05`, `textbox "Accent": "#3b82f6"`). Because SPEC §9 requires the rendered tree to carry the declared role, the catalog declares `textbox` for both | https://www.w3.org/TR/html-aam-1.0/ (2026-10-05); measured with Playwright 1.63.0 (2026-10-05) |
| `color-picker` value is `#rrggbb` | `input type=color` takes a CSS colour and falls back to `#000000` when the value is invalid; the browser's own sanitization wants seven-character lowercase hex, and `alpha` is opt-in, so Weft writes lowercase `#rrggbb` and no alpha. SwiftUI's `ColorPicker(selection:supportsOpacity:)` is told `supportsOpacity: false` | https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/color, https://developer.apple.com/documentation/swiftui/colorpicker (2026-10-05) |
| `segmented-control` is a radio group | APG has no segmented-control pattern: it says radio buttons "in a toolbar are frequently styled in a manner that appears more like toggle buttons" and keeps the radio group roles and keys (arrows move and check). So a segmented control is `radiogroup` of `radio`, as `radio-group`, and only the look differs | https://www.w3.org/WAI/ARIA/apg/patterns/radio/ (2026-10-05) |
| SwiftUI segmented picker | `Picker` with `.pickerStyle(.segmented)`: "A picker style that presents the options in a segmented control", iOS 13 / macOS 10.15 | https://developer.apple.com/documentation/swiftui/pickerstyle/segmented (2026-10-05) |
| `combobox` is an editable combobox, drawn by `<input list>` and `<datalist>` | The combobox pattern separates select-only from editable comboboxes, which "allow typing, either accepting arbitrary values or filtering suggestions"; the native pair is a text input with a `datalist` of suggestions, "not a replacement for `<select>`", where the content entered "always comes from the `value` attribute". Playwright reports it as `combobox "Fruit": app` | https://www.w3.org/WAI/ARIA/apg/patterns/combobox/, https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/datalist (2026-10-05); measured with Playwright 1.63.0 |
| `datalist` is linked by `id` | The input's `list` attribute must equal the datalist's `id`; every Weft element has an `id`, so the renderers always derive one for the datalist | https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/datalist (2026-10-05) |
| SwiftUI has no editable combobox | No view in SwiftUI combines a text field and a suggestion list outside `searchable`; the generator uses a `TextField` with a trailing `Menu` of the options | Not found: no SwiftUI view is documented as a combobox; the `searchable` modifier's suggestions belong to a search field (https://developer.apple.com/documentation/swiftui/controls-and-indicators, checked 2026-10-05) |
| SwiftUI controls | `Slider(value:in:step:)` (iOS 13, `value` a `Binding<V>`, `in` a `ClosedRange<V>`, so a lower bound above the upper one is a runtime error the generator must prevent); `Stepper(_:value:in:step:)` (iOS 13); `DatePicker(_:selection:in:displayedComponents:)` (iOS 13); `ColorPicker(_:selection:supportsOpacity:)` (iOS 14) | https://developer.apple.com/documentation/swiftui/slider, https://developer.apple.com/documentation/swiftui/stepper, https://developer.apple.com/documentation/swiftui/datepicker, https://developer.apple.com/documentation/swiftui/colorpicker (2026-10-05) |
| Figma and Penpot draw no native controls | Both tools build a control from frames, rectangles and text, as the library already does for `switch` and `checkbox`; the drawings use the same primitives (`createFrame`, `createRectangle`, `createText`; Penpot `createBoard`, `createRectangle`, `createText`) and keep the Weft source in plugin data | `@figma/plugin-typings` 1.140.0, `@penpot/plugin-types` 1.5.0 `index.d.ts` (2026-10-05) |

## 16. Moon cache correctness (T56, T57)

Checked on 2026-10-05 against the pinned tools: moon 2.5.6, Vitest 5.0.3, cargo 1.99.0. "Measured" rows are experiments in the T56 worktree, reproducible with the commands given; the other rows cite the vendor's documentation.

| Decision or fact | Basis | Source (checked) |
| --- | --- | --- |
| A glob input can be negated with `!`, and `@group(name)` expands a file group | "Globs can be negated by prefixing with `!`"; file groups are referenced in inputs as `@group(name)` | https://moonrepo.dev/docs/config/project (2026-10-05) |
| Moon's docs say gitignored files are not inputs, but `node_modules` and the gitignored build outputs were hashed | Measured: `.moon/cache/hashes/<hash>.json` of `mcp:test` lists `packages/*/node_modules/.bin/*`, `packages/mcp/node_modules/.vite/vitest/*/results.json`, `packages/core/native/*` and `packages/visual/diffs/*`, all in the root `.gitignore`; the docs' own advice is to exclude explicitly | moon 2.5.6 in the worktree; https://moonrepo.dev/docs/config/project (2026-10-05) |
| Vitest writes its results cache under Vite's `cacheDir`, `node_modules/.vite` by default | "Vitest stores cache for test results to run the longer and failed tests first"; `cacheDir` moves it | https://vitest.dev/config/cache (2026-10-05) |
| The cause of the uncached `mcp:test`: the results file is an input of the next run | Measured: `moon run mcp:test` twice gave hashes 4898c4f7 and bf68f179; the second manifest differs from the first by exactly `packages/mcp/node_modules/.vite/vitest/da39a3ee5e6b4b0d3255bfef95601890afd80709/results.json` | moon 2.5.6, Vitest 5.0.3 |
| The inherited `/packages/**/*` made it every package's problem, and two more folders had the same effect | Measured: after the `node_modules` exclusion, `moon run :test` twice still re-ran all 22 test tasks; the manifests differed by `packages/core/native/*` (built by `core:test`'s dependency `root:native` while other suites were already hashed) and `packages/visual/diffs/*`; a third pair of runs re-ran only `shared:test`, whose own `/plugins/*/**/*` hashed the plugins' `node_modules` | moon 2.5.6 |
| Excluding `node_modules` loses no dependency change | Measured: the manifest of `mcp:test` carries a `toolchain: javascript` entry with the project's resolved production dependencies (integrity hashes from the lockfile); `/pnpm-lock.yaml` is added for the dependencies of sibling packages | moon 2.5.6 |
| The addon's files stay out of the suites' inputs, and its hash reaches them through `deps` | Under `WEFT_ENGINE=auto` every suite that imports `@weft/core` loads `packages/core/native/` when it is built, so the addon is something each suite reads. A task dependency's hash flows into the dependent's hash with the `hash` strategy, the default for a dependency that declares outputs (`root:native` does); `root:native` is therefore a dependency of the inherited `test` task, which also builds it before any suite starts. Measured: `moon query tasks` lists `root:native` among the `deps` of every test task except `tooling:test`, which replaces its deps (it never loads the core); a one-line change in `crates/weft-node` re-ran every suite (the T57 Result in `done.md`) | https://moonrepo.dev/docs/config/project, task dependency `cacheStrategy` (2026-10-05); https://moonrepo.dev/docs/concepts/cache; moon 2.5.6 in the worktree |
| Rejected: moving Vitest's cache with `cacheDir` | It fixes Vitest's cache only; `native/` and `diffs/` are not Vitest's, and one exclusion list in the inherited task covers all of them | https://vitest.dev/config/cache (2026-10-05) |
| The WebAssembly module's crate closure | `cargo tree -p weft-wasm -e normal,build --target wasm32-unknown-unknown`, with and without `--features web`: `weft-wasm`, `weft-binding`, `weft-catalog`, `weft-core`, plus `weft-import` and `weft-web` with `web`; `weft-node` adds nothing to the native addon's six; `weft-swiftui`, `weft-cli` and `weft-snapshots` are in neither | cargo 1.99.0 in the worktree; https://doc.rust-lang.org/cargo/commands/cargo-tree.html |
| The closure is computed from `cargo metadata`, not written twice | `resolve.nodes[].deps[].dep_kinds[].kind` is `null` for a normal dependency, `"build"` or `"dev"`; the test walks the workspace members that are not development-only from the root crate; `--all-features` joins the builds with and without `web` (an over-approximation is the safe side) | https://doc.rust-lang.org/cargo/commands/cargo-metadata.html (2026-10-05) |
| Files a crate reads from outside its folder are inputs too | `include_str!` resolves "relative to the current file"; the closure's sources include `packages/catalog/catalog.json` and `packages/catalog/tokens/default.tokens.json` (weft-catalog). The second was missing from the old input list: a change to the default tokens served a stale module. The test now fails for a missing include | https://doc.rust-lang.org/std/macro.include_str.html (2026-10-05); `crates/weft-catalog/src/core.rs` |
| Narrowed per crate folder, not per `src/` | The crate folders also hold `tests/` and docs the build does not read, but `build.rs`, `Cargo.toml` and any file named by a macro could sit anywhere in the folder, and proving otherwise for each crate is what the test would have to do again; the broader input is kept and costs a rebuild only for a crate the build contains | decision |
| The build reads no other file | No `.cargo/config.toml` or `rust-toolchain*` in the repository (the toolchain comes from `mise.toml`, a moon implicit input through `.moon/`), `weft-node/build.rs` only calls `napi_build::setup()`, and the other crates have no build script (`ls crates/*/build.rs`) | repository files, 2026-10-05 |
| Measured: the WebAssembly task after a change in a crate outside the closure | `moon run root:wasm` with a line appended to `weft-swiftui/src/lib.rs` and to `weft-cli/src/main.rs`: cached (7 ms, same hash 8c5b857e); the same on a line in `weft-core/src/lib.rs`: rebuilt (15.9 s, new hash); reverted: cached. `root:native` likewise: cached after a `weft-swiftui` change, rebuilt after a `weft-web` change | moon 2.5.6 in the worktree |

## 17. 3D transforms and models (T52)

| Question | Finding | Source (checked) |
| --- | --- | --- |
| How a web element is tilted in 3D | The `transform` list takes `perspective(<length>)`, `rotateX()`, `rotateY()`, `rotateZ()`; `perspective()` computes into the element's own matrix, so no parent needs a `perspective` property, and the list is applied right to left (the last function acts on the points first) | https://www.w3.org/TR/css-transforms-2/ (2026-10-06) |
| Which viewer draws a glTF on the web | `@google/model-viewer`, Apache-2.0, version 4.3.1 (latest), last published 2026-06-04 | https://registry.npmjs.org/@google%2fmodel-viewer (2026-10-06) |
| What a page shows while the viewer is not there | A child with `slot="poster"` "will replace the default poster" and "is shown until the model is loaded and revealed"; an element that is not defined (no script) renders its children, so the child image is what a screenshot of a page without the script shows | https://github.com/google/model-viewer/blob/v4.3.1/packages/modelviewer.dev/data/docs.json (slot `poster`, 2026-10-06); HTML Standard, unknown elements are `HTMLUnknownElement` / undefined custom elements, inline with their children |
| Whether `Model3D` runs on iOS | No: in the iOS 27.0 simulator SDK, `_RealityKit_SwiftUI.swiftinterface` declares `Model3D` as `@available(visionOS 1.0, *)` with `iOS`, `macOS`, `tvOS`, `watchOS` unavailable; `RealityView` is `iOS 18.0`, `macOS 15.0` | `Xcode.app/.../iPhoneSimulator27.0.sdk/System/Library/Frameworks/_RealityKit_SwiftUI.framework/.../arm64-apple-ios-simulator.swiftinterface` (Xcode on this machine, 2026-10-06) |
| Model formats per platform | model-viewer loads glTF 2.0 (`.glb`, `.gltf`); RealityKit loads USDZ (and `.reality`); neither reads the other's format, so the element carries both paths | https://modelviewer.dev/docs/ and Apple's RealityKit documentation (the SDK interface above shows no glTF loader) |
| Design tools cannot draw a 3D tilt | The plugin APIs give a layer a 2D `rotation` (degrees) and a 2D affine `relativeTransform`; neither has a perspective or an axis | https://developers.figma.com/docs/plugins/api/properties/nodes-rotation/ (2026-10-06); Penpot's shape `rotation` in `packages/penpot/src/api.ts` |
