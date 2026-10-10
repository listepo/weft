# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

## Cloud review findings (2026-10-08)

New bugs, dead code and moves from a read-only Cursor cloud review of `main` at `26b84ed` (agent `bc-0243e483-4830-5d73-b2ca-cd1ce9880bed`; full report: `cloud/weft.md` in the private `listepo/roadmap` repo). They take ids T75–T93, ordered P0, P1, P2. **confirmed** means seen in the tree or reproduced; **suspected** means plausible from the code but not proven. Line numbers are as of the review. None of these is in the task table yet: to take one, add its row and write its execution plan the usual way.

| ID | Priority | Kind | Status | Where | Fix |
| --- | --- | --- | --- | --- | --- |
| T78 | P2 | bug | confirmed (same run) | `crates/weft-swiftui/tests/swift.rs:201-208` | The Swift typecheck tests ran 9+ minutes and were killed when the gate failed. Give nextest a timeout and a Swift module cache, or keep them out of the batch with flaky runtimes. |
| T87 | P2 | bug | suspected | `crates/weft-web/src/from_jsx/mod.rs:37-66` | JSX import runs on a 64 MiB stack thread only off `wasm32`. Use the same budget or iterative lowering on WASM, or document the limit. |
| T88 | P2 | bug | suspected | `crates/weft-catalog/src/resolver.rs:52-54` vs `:102-103` | The resolver's `tree()` matches context keys case-sensitively; `appearance()` does not. Make `tree()` case-insensitive. |
| T90 | P2 | dead code | confirmed gap (orphans not listed) | `toolchain.md:18`; about 750 insta snapshots | Nothing checks for unreferenced snapshots. Run `cargo insta test --unreferenced=reject` in CI, or once and delete the orphans. |
| T92 | P2 | move | suspected | `tooling/changed.ts`, `tooling/select.ts` → the org's `scoped-check` | A fourth copy of the affected-test planner. The other copies were not checked from this repo. |
| T93 | P2 | move | confirmed | `crates/weft-swiftui/src/import/read.rs`, `crates/weft-web/src/jsx/mod.rs` → smaller modules in the same crates | Split the two giant importer files. |

Already tracked here, not added again: `find_project` and the file-reading `load_project` (`crates/weft-cli/src/main.rs:445-500`) moving into `crates/weft-catalog` is T69.1a; the two private copies of the Slint compile/data code in `crates/weft-slint/tests/` moving to a `runtime` feature is T69.1b.

Not added: Weft → Slint generation stays in `crates/weft-slint` (T69.1 decision 3; slint-bindings should not grow a second renderer); the `dead_code` allows on shared test modules are not dead code.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 70% | Claude Code / claude-opus-5-5 |
| T14 | in progress | P2 | 5 | 70% | Claude Code / claude-opus-5-5 |
| T39 | in progress | P1 | 4 | 80% | Claude Code / claude-opus-5-5 |
| T39.9 | todo | P2 | 3 | 0% | |
| T68 | in progress | P2 | 4 | 0% | Grok Bot / grok |
| T69 | todo | P2 | 5 | 0% | |
| T69.1 | todo | P2 | 4 | 0% | |
| T12.3 | todo | P2 | 4 | 0% | |
| T16 | todo | P2 | 5 | 30% | |
| T16.3 | todo | P2 | 3 | 0% | |
| T16.8 | todo | P2 | 3 | 0% | |
| T16.4 | todo | P2 | 4 | 0% | |
| T16.5 | todo | P2 | 3 | 0% | |
| T16.7 | todo | P2 | 3 | 0% | |
| T17 | todo | P2 | 4 | 50% | |
| T17.2 | todo | P2 | 3 | 0% | |
| T17.3 | todo | P2 | 3 | 0% | |
| T41 | todo | P2 | 2 | 0% | |
| T96 | todo | P2 | 3 | 0% | |
| T97 | todo | P2 | 2 | 0% | |
| T98 | todo | P2 | 2 | 0% | |
| T99 | todo | P3 | 1 | 0% | |

### T96. Too-deep documents: report, do not hide

Found while merging #18. After T79, `serialize`, `stringify` and `canonicalize` (`crates/weft-binding/src/api.rs`, `crates/weft-core/src/canonical.rs`) return the output of an empty placeholder document (`to_document(Null)`) for a cyclic or too-deep input instead of failing. Only `applyPatches` reports W200. A caller gets a valid-looking empty screen and no signal.

Done when: these entry points return W200 (or the binding's error type) for a too-deep or cyclic input, in Rust and TypeScript alike, with differential fixtures; no entry point turns such input into an empty document silently.

### T97. Design-tool plugin data over 100 kB

Found while merging #18. T83 made `writeJson` (`packages/design-tool/src/keys.ts`) throw once an entry passes 100 kB. It is called partway through `packages/design-tool/src/build.ts`, so one node with a very large source aborts the whole Figma or Penpot build. The message says "bytes" but the check counts string length (UTF-16 code units), not UTF-8 bytes.

Done when: the cap is measured in UTF-8 bytes as Figma documents it; an oversized entry is reported as a build diagnostic and the rest of the build completes (the layer is drawn, its source is not stored, and pull reads it as foreign); tests cover a multi-byte source near the cap.

### T98. `fmt --write` keeps symlinks and permissions

Found while merging #18. T81's atomic write (temp file, then rename) in the Rust CLI (`crates/weft-cli/src/main.rs`) and the TypeScript CLI replaces the target path. Formatting a symlinked file turns the link into a regular file, and the file's permissions are not kept.

Done when: both CLIs resolve a symlink and write to its target, copy the original file's permissions onto the temp file before the rename, and have tests for a symlinked file and a read-only-group file.

### T99. Node capped read handles a short read

Found while merging #18. The T86 capped read in `packages/catalog/src/node.ts` makes a single `readSync` call into a buffer of `maxChars*4+1` bytes allocated up front. A short read is unlikely on a regular file but is not handled, and the buffer is sized for the worst case.

Done when: the read loops until EOF or the cap, and a test covers a reader that returns fewer bytes than asked.

### T8. Evaluation

T20–T25 have landed, so the final runs are the ones on the Rust core and the stricter benchmark. The harness steps below are in the tree. The full edit and read runs are not: this environment has no `ANTHROPIC_API_KEY` and no LM Studio server, and the runs already in `test.md` are the earlier one-sample measurements.

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.

Execution plan:

1. Repair cycle in `bench/src/run-tasks.ts`: when an edit reply is invalid (unparseable or format errors), send one follow-up prompt with the previous reply and the validator's diagnostics, and record first-try and after-repair validity and success separately. Same rule for every format, so the baselines get the same help. Cover it with a mocked-provider test in `bench/test/checkers.test.ts`.
2. Summaries and the rendered table show first-try valid, valid after repair, first-try success, success after repair, and mean output tokens.
3. Models: Claude Sonnet 5.5, Claude Opus 5.5, Claude Haiku 4.5 (`DEFAULT_MODELS`).
4. Raw results go into the repository: stop ignoring `bench/results/`.
5. Trial run (one screen, all formats, one model) to check cost and the harness; the creator reviews it before the full run.
6. Full edit and read runs on the three models; `run.ts tokens` again so the report has Anthropic token counts.
7. `bench/EVALUATION.md`: results against the done criteria (Weft first-try validity at least 95%, after one repair at least 99%), comparison with the baselines, failure analysis, continue/stop recommendation.
8. Verify with `pnpm run ci`.

Progress: steps 1–5 are done (repair cycle, summaries, `DEFAULT_MODELS`, results committed under `bench/results/`, trial row in `test.md`). `bench/EVALUATION.md` records those runs against the numeric bar and does not close the task. Remaining: step 6, the three-sample edit and read runs on the current harness, then a history row and a revision of the report. Step 8 waits on that.

### T14. Figma round trip and plugin

Convert Weft to Figma and back without loss, and ship a Figma plugin. Approved scope:

- **Library:** generate a Weft component library in Figma from the catalog and the design tokens (one component set per catalog kind, variants for its enum props, Figma variables for the tokens). Screens are built from instances of this library.
- **Weft to Figma:** every node becomes an auto-layout frame or a library instance, and its Weft source (kind, props, bindings, data paths, ids) is stored in the node's plugin data.
- **Figma to Weft, lossless:** a screen that was never edited comes back byte-identical after `weft fmt`. A designer's edits come back too: text, order, added or removed library instances, variants, and visual edits (spacing, colors, sizes, radii). A visual edit maps to a token when it matches one and otherwise to a style override, which needs a format extension (`AGENT-SPEC.md`, the parsers in TS and Rust, the catalog). Its design goes to the creator before it is built.
- **Foreign layers:** layers that did not come from Weft (vectors, images, free frames) convert lossily, with a loss table, as before: Figma layers carry no semantics.
- **Where it runs:**
  - a Figma plugin: open or paste a `.weft` file to build frames; select a frame to export `.weft`.
  - CLI `weft figma pull`: reads through the REST API (`GET /v1/files/:key/nodes`). The REST API is read-only, so building frames stays in the plugin.
  - MCP / Claude Code tools (with T30), working through the Figma MCP server.

Done when every corpus screen survives Weft to Figma to Weft byte-identical, a scripted set of designer edits comes back with the expected Weft diff, and the plugin, CLI command and MCP tools pass their tests in `moon ci`.

Execution plan, stage 1 (library, both conversions, plugin; the CLI command, the MCP tools and style overrides are later stages):

1. `packages/figma` (`@weft/figma`), pure TypeScript against a narrow interface of the Plugin API subset it uses. A type test checks that the official `@figma/plugin-typings` types satisfy that interface, so the real `figma` object and the in-memory fake (`test/fake-figma.ts`) are interchangeable.
2. Library: one page with a component set per catalog kind (variants for its enum props and `state`, with an `(unset)` value where the prop has no default) and one variable collection holding the design tokens (dimensions as FLOAT in px, colors as COLOR). Kinds and tokens are recorded in plugin data, so a second run finds and reuses them.
3. Weft to Figma: kinds whose content is `none` or `text` become library instances, the rest auto-layout frames (`stack` direction, gap bound to its token variable, align, wrap; `grid` columns); `<each>` and named slots become marked frames; text children become text layers. Each node keeps its Weft source (kind, id, props, events) in plugin data, and the root keeps the document version.
4. Figma to Weft: a node is rebuilt from its stored source, and the Figma state is compared with the state that source would build. What is equal keeps the source as written (so an unedited screen is byte-identical); what differs is the designer's edit: text, label, child order, removed nodes, variant props, hidden, stack direction/align/wrap, and a gap that matches a token (bound variable or equal value). Copied nodes get fresh ids. New library instances become new nodes with generated ids and stand-ins for required props. Visual edits with no Weft prop or no matching token are reported as losses.
5. Foreign layers (text, free or auto-layout frames, groups, images, vectors) convert lossily with a loss table in the shape of `@weft/from-aria`; reuse its id, literal and limit helpers.
6. `plugins/figma`: `manifest.json`, a thin `code.ts` over `@weft/figma` (build from pasted or opened `.weft`, export the selected frame), `ui.html`; bundled into one file with Vite, with a test that the bundle builds.
7. Tests (Vitest): every corpus screen round-trips byte-identical through the fake; a scripted set of designer edits yields the expected Weft; foreign layers yield the expected losses; library generation is idempotent.
8. `packages/figma/README.md` with the Plugin API facts the code relies on, each with its official URL and the date checked. `docs/figma-style-overrides-design.md`: the style-override proposal for the creator (not built).
9. Wire into pnpm, moon, `tsconfig.json`, `toolchain.md`; verify with `mise exec -- moon run :test root:typecheck root:lint root:rust-test root:rust-lint`.

Progress (stage 1 done, on branch `t14-figma`):

- Steps 1–9 are done. `@weft/figma` builds the library, both conversions, foreign layers and the plugin message handler. `plugins/figma` holds the manifest, a thin `code.ts`, `ui.html` and the Vite bundle. The bundle is tested by running it in a bare VM over the fake.
- Tests:
  - all 12 corpus screens round-trip byte-identical with no losses;
  - 19 scripted designer edits match the expected Weft patches and loss kinds;
  - building the library twice reuses it.
- `from-aria` now exports `freshId`, `literal`, `fillRequired` and the limit helpers, which both importers share.
- After T22 (WebAssembly core), the plugin is split. Figma's main thread has no WebAssembly, so it only builds and reads layers (`code.js`, with the core's loader stubbed). The UI iframe parses and serializes with the core inlined in `ui.html`. A harness runs both halves, the main thread without WebAssembly. The main thread interprets no values: the UI formats `text` and `label` for the build, and `finishRead` reads typed text with the core's `readValue` (a new `weft-wasm` export), so the value rules have one implementation.
- `docs/figma-style-overrides-design.md` waits for the creator's approval.
- Remaining:
  - style overrides: after approval, the format extension in TS and Rust, then the Figma mapping for colors, radii and padding (only `gap` maps to a token today);
  - a pull from a real Figma file, to confirm the REST facts marked unverified in `packages/figma/README.md` (rotation in radians, number precision, the shape of `sharedPluginData`);
  - MCP / Claude Code tools (with T30): the source is shared plugin data now (T14.2); still to check that `use_figma` can read and write it;
  - a check of the plugin in the real Figma app, including the inline module script and WebAssembly in its UI iframe;
  - the open questions in the report: single components for kinds without variants, `state` as a variant axis, the manifest id, and plugin data on duplicate/detach.

Execution plan, stage 2 (this round; style overrides stay out until the creator approves them):

1. T14.1: the pull over the REST API — done (`done.md`).
2. MCP / Claude Code tools through the Figma MCP server. The Figma MCP server's `use_figma` tool runs Plugin API code, but its own description says `setPluginData` is not supported there, and plugin data is readable only by the plugin that wrote it. The creator chose to move the Weft source to shared plugin data, which any plugin and the REST API (`plugin_data=shared`) can read: T14.2, done (`done.md`).
3. T14.3: the creator dropped files that keep the Weft source in private plugin data; only shared plugin data is supported — done (`done.md`).


### T39. Context in the document

A `.weft` file carries the context that a person or an agent left for whoever works on it next, so the next agent or person can use it. Approved scope:
- **Two levels:** a context block on the screen (purpose, decisions, constraints, open questions) and context entries on any element (why a button is disabled, where a label came from).
- **Typed entries:** every entry has a kind (`intent`, `decision`, `constraint`, `question`, `todo`, `source`), an author (`human` or `agent`, with a name or model) and text. The validator checks their shape.
- **Part of the document:** context is in canonical JSON and survives formatting, patches (new patch operations to add, change and resolve entries) and every conversion. Code generators write it as comments; Figma keeps it in plugin data; importers read it back from code that Weft generated.
- **Untrusted by design:** context is data for the reader, never instructions. `AGENT-SPEC.md` tells models to treat it as information to weigh and to ignore commands inside it. Renderers never show it to end users.

Format change, so the design comes first: syntax, canonical JSON, validation codes, patch operations and the effect on every target go to the creator for approval before `SPEC.md`, `AGENT-SPEC.md`, the Rust core and the targets change together. Done when a screen with context on both levels survives fmt, patches, every round trip that exists, and the MCP tools expose it.

The creator approved the design in `docs/context-design.md`, with the recommendation of each of its open questions. Execution plan, build stage: the build is far over 500 lines of code, so it is split into T39.1–T39.9, one pull request each, in the order of the design's implementation outline. Each subtask changes `SPEC.md` and `AGENT-SPEC.md` for what it builds, in the same commit, regenerates the fixtures it touches, and passes `mise exec -- moon run :test root:typecheck root:lint root:rust-test root:rust-lint`. Before each pull request, merge `origin/main`; T31 builds fragments in parallel and also moves the format to `weft` 0.2, so whichever lands second keeps the other's bump. T39 closes when its done criteria hold after T39.9.

### T39.9. Context: SwiftUI

SwiftUI has no source comment, so it carries context as `// weft:context <entry JSON>` lines above the view struct, read back by `weft import-swiftui`, with readable `//` comments above each named view that the importer ignores. `export.swiftui.context` and `import.swiftui.context` (SPEC §10.6, `settings.rs`, the schema) and the CLI `--context`, as T39.6 gave the web and Slint targets; a round-trip test.

Open question for the creator before this is claimed: `import_swiftui` reads code by convention, with no regeneration check like the web and Slint importers have. A `// weft:context` line therefore cannot be told apart from one forged in foreign Swift, and the design reads context only from code recognized as Weft-generated. Options: add a `weft:source` comment with that check to SwiftUI first, or accept the lines on a lenient read and validate them.

### T68. Slint bindings for SwiftUI and WinUI

Research how Slint embeds into native apps (the `slint::platform` custom platform, the software renderer, the window adapter, input and accessibility) and whether Slint has official bindings for SwiftUI (macOS, iOS) or WinUI 3 in `slint-ui/slint`, its code and its open pull requests. If official bindings exist, record where and use them. If not, build them as a separate project, `~/GitHub/listepo/apps/slint-bindings`, in the public repository `listepo/slint-bindings` (https://github.com/listepo/slint-bindings), pushed to `main`: a Rust core (`slint-bindings-core`) and a C ABI (`slint-bindings-ffi`), a Swift package that hosts a Slint component in SwiftUI through `NSViewRepresentable`, and a WinUI 3 package for Windows. Weft consumes those bindings and does not keep its own embedding: the Weft desktop work builds on T67 (the Slint the screens become) and on this project. Done when the research with its sources is written down in that repository, and a Slint component generated by T67 shows and takes input inside a SwiftUI window on macOS through the bindings; the WinUI 3 half is checked on Windows.

### T69. Figma-like desktop design editor

A desktop design editor in the spirit of Figma for Weft documents, on macOS and Windows. Weft is the document format: an editor document is a set of Weft screens (`.weft`, canonical markup and JSON, SPEC §2–§3) with the project's catalog and tokens (SPEC §10), so whatever the editor saves validates, formats and converts with the existing tools, and agents edit the same files through the CLI and MCP server.

**Prerequisite: the Slint bindings.** The macOS (Swift) and Windows (WinUI 3) integration MUST use the Slint bindings of T68: the project `~/GitHub/listepo/apps/slint-bindings`, repository `listepo/slint-bindings` (https://github.com/listepo/slint-bindings), with its crates `slint-bindings-core` (custom `slint::platform`, software renderer, window adapter, input) and `slint-bindings-ffi` (C ABI), its `swift/` package (a Slint surface in SwiftUI through `NSViewRepresentable`) and its `windows/` package (the same surface in WinUI 3). No separate or ad-hoc embedding lives in Weft. A gap found there (input, IME, focus, DPI, accessibility, rendering, lifecycle) is fixed in slint-bindings and consumed from it, never worked around here. Depends on T68. Weft → Slint is done: T67 and T67.1–T67.5 (`done.md`) cover the whole catalog and the read-back of edited Slint.

**Architecture split.**

| Part | Built with | What it holds |
| --- | --- | --- |
| Editing surface (shared) | Slint, embedded through slint-bindings, one code base for both platforms | The canvas (pan, zoom, rulers, grid, snapping), the rendered screens and frames (each Weft screen rendered through the T67 converter), selection, hover and resize handles, the layers tree of the edited design, the property inspector of the selected element (props, bindings, tokens, states, events, slots), the component and catalog panel (insert a kind, frames and component instances), and any other editing widget that must behave the same on both platforms |
| App shell (native) | SwiftUI and AppKit on macOS, WinUI 3 on Windows | Windows and tabs, menus and the menu bar, toolbars, file open and save dialogs, settings, the document browser and recent files, sidebars and panels outside the editing surface, notifications, the platform's undo menu items and keyboard shortcuts wired to the shared core |
| Document core (shared) | Rust, the Weft crates | Loading and saving `.weft` files and projects, validation (`weft-core`), every edit as a Weft patch (SPEC §7) so undo and redo are patch inverses and agents' patches apply the same way, the catalog and tokens (`weft-catalog`), and the Weft ↔ Slint conversion (`weft-slint`) that renders the screens and reads edits back |

The native shell and the shared surface talk only through the slint-bindings C ABI and a small editor API on top of it (open, save, select, apply patch, events back to the shell); neither side reaches into the other's toolkit.

**Milestones** (each becomes a T69.x task with its own plan when claimed, within the task size rule):

1. Skeleton: a native window on each platform hosting one Slint surface through slint-bindings, showing one corpus screen rendered by `weft-slint`; open and save through the native file dialogs.
2. Canvas: several screens as frames, pan and zoom, rulers, selection and hover from the Weft element tree, keyboard navigation.
3. Editing: move, reorder, insert and delete elements, resize within the layout rules, the layers tree and the property inspector writing Weft patches; undo and redo; validation diagnostics shown on the canvas and in the inspector.
4. Components and tokens: the catalog panel, catalog-extension kinds as components, design tokens from the project (spacing, colours, typography) in the inspector, the light and dark appearances.
5. Round trip with the outside: the saved files stay canonical (`weft fmt` leaves them unchanged), the existing exporters (SwiftUI, web, Slint, A2UI) run from the native menus, and files edited by agents or by hand reload without losing the selection where ids still exist.
6. Packaging: signed builds for macOS (arm64 only) and Windows x86_64, with the toolchain rows in `toolchain.md`.

**Acceptance criteria.**

- The same Slint editing surface code runs inside the SwiftUI app on macOS and inside the WinUI 3 app on Windows through slint-bindings, with no editor-specific embedding code in either shell.
- App chrome (windows, menus, toolbars, dialogs, settings, document browser) is native on each platform and contains no Slint.
- Every corpus screen opens, renders, and saves back byte-identical when nothing was edited; a scripted set of edits (move, insert, delete, change a prop, bind a prop, apply a token) produces the expected Weft diff, and the result passes strict validation.
- Undo and redo restore the exact previous document; edits are recorded as Weft patches.
- Tests: unit tests of the document core and the editor API, integration tests of the scripted edits over the corpus, and a check that every rendered screen's Slint compiles; the native shells have UI smoke tests on their platforms.

**Out of scope.** Multiplayer and real-time collaboration, cloud storage, accounts and sharing, comments and review threads, prototyping and interaction flows, vector drawing tools and boolean shape operations, image editing, plugins for the editor, Figma file import (T14 covers the Figma round trip), a web version (later, through Slint's WebAssembly build), iOS and Android, Intel Macs, and Linux.

Not started. The plan of the first milestone is T69.1; nothing is built until the creator approves it.

### T69.1. Editor skeleton

Milestone 1 of T69: a native window on macOS and on Windows hosts one Slint surface through slint-bindings, shows one corpus screen rendered by `weft-slint`, and opens and saves `.weft` files through the native file dialogs. This card is the plan for the creator's approval; nothing is built before that. It splits into five sub-tasks, T69.1a–T69.1e, each within the 500-line limit and each claimed as its own row with the plan below. Facts were checked on 2026-10-08; sources are at the end of the card.

**What exists today.**

- Weft: `weft-slint` generates every corpus screen, and the generated files compile with `slint-interpreter` 1.18.1 (T67–T67.5). The interpreter is a dev-dependency only. Two private copies of a compile helper live in `crates/weft-slint/tests/slint.rs` and `tests/screenshots.rs`, and the second also feeds `data.json` into a component's properties (about 150 lines). Project discovery (`find_project`) and the file-reading project loader are private to `crates/weft-cli/src/main.rs`. The workspace forbids `unsafe_code`; `weft-node` overrides it with its own `[lints]`.
- The name WeftEditor and the bundle id `dev.weft.editor` already belong to the host app of the Source Editor Extension (`plugins/xcode/editor-extension/project.yml`), which also exports the file type `dev.weft.screen`.
- slint-bindings (`listepo/slint-bindings`, `main` at `2b9fa96`): the table under "What M1 needs from T68".

**Architecture (recommended).**

```text
studio/macos (SwiftUI + AppKit)                studio/windows (WinUI 3, C#)
  menus, NSOpenPanel, NSSavePanel                MenuBar, FileOpenPicker, FileSavePicker
  SlintNSView (from slint-bindings)              SlintPanel (from slint-bindings)
     │ ws_* editor API   │ sb_* surface             │ ws_*            │ sb_*
     └──────── one Rust library: libweft_studio_ffi (.a on macOS, .dll on Windows) ────────┘
        crates/weft-studio-ffi   ws_* C ABI; links slint-bindings-ffi, so sb_* ship in the same library
        crates/weft-studio       document core: open, save, project, render input (no Slint)
        crates/weft-slint        generate; new `runtime` feature: interpreter compile and sample data
        slint-bindings-core/-ffi (git dependency): host-driven platform, EmbeddedHost, SbHost
```

- Rust owns the document and the Slint component. A shell holds two opaque handles, `WsEditor*` for the document and `SbHost*` for the surface, and never sees Weft markup or Slint source. The existing slint-bindings views draw the surface and feed it input; the shells add only chrome and dialogs, so no editor logic is written twice.
- One Rust library per app. Two Rust static libraries in one app would each carry a copy of Slint, and the screen component would not find the platform that slint-bindings installed.

**Where the code lives.**

| Path | What it holds |
| --- | --- |
| `crates/weft-studio` | The document core and the editor API as Rust types. No Slint, no FFI, the workspace lints unchanged. |
| `crates/weft-studio-ffi` | The `ws_*` C ABI. `crate-type = ["rlib"]`; the static library and the DLL are built on demand, as slint-bindings' justfile does. Its own `[lints]` set `unsafe_code = "deny"` and allow it only in the `extern "C"` module, as `weft-node` does. Committed `include/weft_studio.h` (cbindgen) and `NativeMethods.g.cs` (csbindgen), both checked by a drift test, as `scull-ffi` does. |
| `studio/macos` | XcodeGen `project.yml` (as `plugins/xcode` uses it), the SwiftUI app WeftStudio, bundle id `dev.weft.studio`, macOS 14, arm64 only. |
| `studio/windows` | `WeftStudio.WinUI` (.NET 10, Windows App SDK 2.5.1 like the slint-bindings control, unpackaged, x64 and ARM64) and `WeftStudio.Tests`. |
| `studio/test`, `studio/moon.yml` | Vitest drivers that build and smoke-test each shell and skip with a message on the other OS or without its toolchain, as `plugins/xcode/test` does; the task `studio:test`. |

**Depending on slint-bindings.** Recommended: a Cargo git dependency on `slint-bindings-core` and `slint-bindings-ffi` from https://github.com/listepo/slint-bindings, pinned by `rev`. That rev is the only pin. The Swift package and the C# control come from the checkout Cargo made of the same commit: `cargo metadata` gives the `manifest_path` of `slint-bindings-ffi`, and a script passes the checkout to XcodeGen as `${SLINT_BINDINGS_DIR}` (a local package) and to MSBuild as a generated, gitignored `.props` file (a `ProjectReference`). For co-development, a gitignored `.cargo/config.toml` `[patch]` points at a sibling checkout and moves all three languages at once, the way ketch consumes `file-backup` (`rust.md`, "Shared crates across projects"). Rejected: a path dependency (`rust.md`: a bare path breaks every standalone checkout and CI run of a public repo); a published crate (slint-bindings is `publish = false`, and its core depends on the vendored `slint-embed` path crate); a git submodule (every agent worktree would need its own `git submodule update`). When slint-bindings ships packages (its T9: XCFramework, NuGet), the shells switch to them.

**Build and checks.**

- macOS: XcodeGen 2.46.0 (already in `mise.toml`). A pre-build script phase runs `cargo rustc -p weft-studio-ffi --crate-type staticlib --target aarch64-apple-darwin` (`--release` for Release); the app links that library and the frameworks slint-bindings' `Package.swift` lists (AppKit, CoreFoundation, CoreGraphics, CoreText, Foundation). Signed ad hoc, no App Sandbox in M1 (decision 5).
- Windows: `dotnet build studio/windows`; a `BeforeBuild` target runs `cargo rustc -p weft-studio-ffi --crate-type cdylib --target x86_64-pc-windows-msvc` (or `aarch64-pc-windows-msvc`) and copies the DLL next to the app.
- moon: `root:rust-test` and `root:rust-lint` already cover `crates/**` with `--workspace --all-features`, so the core, the runtime feature and the headless FFI tests run in the full check on any OS (the software platform needs no window). `studio` joins `.moon/workspace.yml` and `pnpm-workspace.yaml`; `studio:test` runs the macOS legs on a Mac with Xcode and the Windows legs on Windows with the .NET SDK. The full check runs on macOS by hand today, and the open T73 pull request (#4) adds a macOS runner only, so nothing automated builds the Windows shell (decision 4).

**Slint at run time: interpreter, not compile-time `.slint`.** `slint-build` compiles `.slint` files when the app is built; the editor opens files the user picks at run time, so only `slint-interpreter` fits. It loads `.slint` source at run time (`Compiler::build_from_source`, `ComponentDefinition::create`), and its `ComponentInstance` implements `ComponentHandle`, which is the bound of slint-bindings' `EmbeddedHost<C: ComponentHandle>`: an interpreted screen goes into the existing host unchanged. The build is `async`, but only truly so with a file loader; without one a poll loop is enough, as Weft's tests already do. Features: `compat-1-18` and `std` without defaults (no backend, no renderer), because slint-bindings supplies the platform and the software renderer. Both repositories must resolve to one `i-slint-core`: slint-bindings pins `=1.18.1` and Weft's lockfile has 1.18.1. Style `fluent` on both platforms, as the screenshot baselines use. For later milestones: several screens on one surface (M2) cannot use `ComponentContainer` or `component-factory`, because in 1.18.1 the compiler removes both from its builtin register unless experimental features are on, and `slint::ComponentFactory` is `#[doc(hidden)]` and deprecated as "made public by mistake". M2 will compile one source that holds the editor's own components and the screens as sub-components, with a `weft-slint` option to emit a plain component instead of `inherits Window`. M1 needs neither, and the API below leaves room for both.

**Editor API v0** (`include/weft_studio.h`):

```c
typedef struct WsEditor WsEditor;
typedef void (*WsEventFn)(void *user_data, const char *event_json);

uint32_t ws_api_version(void);                                    /* 0; a shell refuses any other */
WsEditor *ws_editor_new(WsEventFn on_event, void *user_data);
void ws_editor_free(WsEditor *editor);
bool ws_open(WsEditor *editor, const char *path);                 /* UTF-8 path from the native dialog */
bool ws_save(WsEditor *editor, const char *path);                 /* NULL: the current path */
char *ws_state_json(const WsEditor *editor);                      /* free with ws_string_free */
struct SbHost *ws_surface_new(WsEditor *editor, uint32_t width, uint32_t height, float scale);
void ws_string_free(char *text);
const char *ws_last_error(void);
```

- **Threads.** Every call comes from the thread that created the editor (the UI thread), as slint-bindings requires. Slint calls back synchronously while it dispatches input.
- **Errors.** A failed call returns `false` or `NULL` and leaves a message for `ws_last_error` (thread-local, valid until the next call). Every entry point catches panics; a null pointer or a string that is not UTF-8 is an error.
- **Open.** Reads the file (at most 2 MB, the bound `weft-slint` puts on Slint input), finds and loads the project (SPEC §10.1–§10.2, never fatal: project diagnostics are kept), and parses the markup with the project's catalog and tokens, leniently. A syntax error refuses the open with its diagnostics; a document with validation errors opens and cannot render. M1 opens `.weft` markup only, not canonical JSON.
- **Save.** Writes `serialize(document)` atomically (a temporary file in the same directory, then a rename), so a saved file is canonical and `weft fmt` leaves it unchanged; an unedited canonical file saves byte-identical. Saving to another path makes it the document's path.
- **State.** `{"v":0,"path":…,"title":…,"screen":…,"dirty":false,"diagnostics":[…]}`; `title` is the file name, `dirty` is always false until M3 brings edits.
- **Surface.** `ws_surface_new` generates Slint from the document (`weft_slint::generate` with the project's catalog and tokens), compiles it with the interpreter, fills the data properties from the project's `render.data` sample data when it is set (SPEC §10.6; no new setting), connects `perform(action, id)` to an `action` event, and returns a new `SbHost` that the shell owns and frees with `sb_host_free`. A refused screen returns `NULL` with the generator's message. The editor keeps only a weak handle to the component.
- **Events** (JSON with `"v":0`): `{"type":"changed"}` after an open or a save (the shell reads `ws_state_json` for the title), and `{"type":"action","action":…,"id":…}` when the screen calls `perform`. Input reaches the screen through the `sb_host_*` calls of the slint-bindings views; M1 is a live preview (decision 7).
- **Later, without changing v0:** `ws_apply_patches` (SPEC §7 through `weft_core::apply_patches`; undo and redo as inverse patches), selection, and selection events (M2–M3).

**What M1 needs from T68.**

| Need | slint-bindings today (`2b9fa96`) | Gap | Blocks |
| --- | --- | --- | --- |
| A host-driven platform: software renderer into a host buffer, resize and scale, pointer, key and focus input, timers | `slint-bindings-core` `EmbeddedHost<C: ComponentHandle>`, tested; 1.0 ms per changed frame and 8.4 ms for the first at 1600×1200 (release, Apple Silicon) | none | — |
| An interpreted component in that host | `EmbeddedHost` is generic, but nothing hosts a `slint_interpreter::ComponentInstance` | G1: a test in slint-bindings that hosts one (its T11 starts there) | T69.1c |
| A C ABI host over any component, made by another crate | `SbHost` wraps `EmbeddedHost<DemoForm>`; `sb_demo_new` is the only constructor, and the type is private | G2: `SbHost` over an interpreted component, plus a public Rust constructor (`SbHost::into_raw`) so a downstream crate returns `SbHost*`; the `sb_*` symbols stay exported when `slint-bindings-ffi` is linked as an rlib into another library | T69.1c |
| A SwiftUI/AppKit view for a host made elsewhere, usable from another package | `SlintNSView` creates its own demo `SlintHost`; the library target links `../target/debug/libslint_bindings_ffi.a` through `unsafeFlags`, which SwiftPM refuses in a package another package depends on | G3: `SlintNSView` and `SlintHost` take the host from the caller; the library target links no Rust library (the app does); the `sb_demo_*` calls leave the library target | T69.1d |
| A WinUI 3 control for a host made elsewhere | A skeleton never compiled (its T6); `SlintPanel` creates its own host | G4: it builds and runs on Windows (T6) and takes the host from the caller | T69.1e |
| One Slint version | `=1.18.1` exact pins; Weft at 1.18.1 | none; upgrades move together | — |
| Keyboard map and IME, popups, accessibility, GPU, packaging | its T4, T10, T14, T7–T8, T9, all todo | not needed for M1: ASCII typing works, and M1 only shows the screen | — |
| The FFI guard (panic catch, last error) | private in `slint-bindings-ffi` | optional: public, so `ws_*` reuse it and share `sb_last_error`; otherwise `weft-studio-ffi` keeps a copy of about 30 lines | — |

T68's done criteria ("a Slint component generated by T67 shows and takes input inside a SwiftUI window") cover G1 and part of G2, not G3 or G4 (decision 3). slint-bindings' own plan overlaps Weft: its T12 builds `slint-bindings-weft`, a second Weft → Slint mapping beside `weft-slint`, and its T16 a Slint desktop app for Weft beside T69.

**Sub-tasks** (order: T69.1a and T69.1b now and in parallel; T69.1c after G1–G2; T69.1d after T69.1c and G3; T69.1e after T69.1c, G4 and decision 4):

- **T69.1a. Document core** (`crates/weft-studio`, about 300 lines). Move `find_project` and the file-reading project loader from `crates/weft-cli/src/main.rs` into `weft-catalog` and switch the CLI to them, with no change in behaviour. Then `Editor` with `open`, `save`, `state` and `render_input` (the Slint source and the sample data), and an error type with distinct variants (I/O, too large, syntax with diagnostics, generation refused). Done when `crates/weft-studio/tests/editor.rs` passes: every corpus screen opens, renders its input and saves byte-identical into a temporary directory; a non-canonical file saves as `weft fmt` prints it; a syntax error refuses the open with its diagnostics; a strictly invalid document opens and its render is refused with the generator's message; `render.data` feeds the data; a project with errors still opens and reports them; a file over the limit is refused; a failed save leaves the original untouched and no temporary file behind. `crates/weft-cli/tests/deps.rs` asserts that `weft-studio` has no Slint crate, and the CLI tests stay green.
- **T69.1b. Slint runtime** (`crates/weft-slint` feature `runtime`, about 250 lines, mostly moved). `runtime::compile(source, style)` and `runtime::apply_data(instance, source, data)` replace the two test copies and return problems instead of panicking, because the data is untrusted. `slint-interpreter` becomes an optional dependency of the feature; the `deps.rs` optional list gains it. Done when `tests/slint.rs` and `tests/screenshots.rs` use the feature and keep passing against their baselines, and a value of the wrong type is a reported problem, not a panic.
- **T69.1c. Editor C ABI** (`crates/weft-studio-ffi`, about 350 lines). The git dependency, the `ws_*` calls above, the header and the C# bindings with their drift test (cbindgen and csbindgen, both in `rust.md`), and the `toolchain.md` rows. Done when `tests/e2e.rs` passes headless: `login` opens, `ws_surface_new` at 480×800 and scale 1 renders pixels that are not all transparent through `sb_host_render`; a press and release over the submit button raise one `action` event with the screen's action and id; a save round-trips through the ABI; null pointers, a path that is not UTF-8, a missing file and a refused screen return `false` or `NULL` with a message; `cargo metadata` shows one `i-slint-core`; and the macOS static library exports both `ws_open` and `sb_host_render` (`nm`). Measure the open-to-first-frame time of the largest corpus screen in release and record it here; above 500 ms, cache the compiled definition per source.
- **T69.1d. macOS shell** (`studio/macos`, about 350 lines of Swift). One window per document, a File menu with Open… (⌘O), Save (⌘S) and Save As… (⇧⌘S), `NSOpenPanel` and `NSSavePanel` filtered to `.weft` (the app imports `dev.weft.screen`, which the editor extension exports) behind a small `FilePicking` protocol, an alert with the last error and the diagnostics, the window title from the state, and slint-bindings' `SlintNSView` with the host from `ws_surface_new`. Done when Swift Testing unit tests of the document model pass with a fake picker; an XCUITest launches the app with a corpus screen as a launch argument, finds the window titled `screen.weft`, sees a surface screenshot that is not one colour, and Save As through the fake picker writes the same bytes; and `studio/test/macos.test.ts` runs XcodeGen, `xcodebuild build` and `xcodebuild test` (arm64). A manual checklist in `studio/README.md` covers the real dialogs, a Retina display and live resize.
- **T69.1e. Windows shell** (`studio/windows`, about 350 lines of C#). `MainWindow` with a `MenuBar` (Open Ctrl+O, Save Ctrl+S, Save As Ctrl+Shift+S as `KeyboardAccelerator`s), `FileOpenPicker` and `FileSavePicker` from `Microsoft.Windows.Storage.Pickers` (Windows App SDK 1.8 and later: constructed with `AppWindow.Id`, the result's `Path` is the file) behind an `IFilePicker` interface, a `ContentDialog` for errors, slint-bindings' `SlintPanel` with the host from `ws_surface_new`, and P/Invoke from the generated `NativeMethods.g.cs`. Done when MSTest unit tests of the document model pass with a fake picker; a FlaUI (UIA3) smoke test launches the app with a corpus screen, checks the window title and a panel capture that is not one colour, and Save As writes the same bytes; and `studio/test/windows.test.ts` runs `dotnet build` and `dotnet test` on Windows.

**Done when** the four checks above pass on their platforms, the full check exits 0, every corpus screen opens and renders in both apps through the native open dialog and saves through the native save dialog (byte-identical when unedited), and neither shell contains embedding code of its own: the surfaces are slint-bindings' `SlintNSView` and `SlintPanel`.

**Decisions for the creator** (recommendation first):

1. **Name.** WeftStudio: folder `studio/`, crates `weft-studio` and `weft-studio-ffi`, bundle id `dev.weft.studio`, because WeftEditor and `dev.weft.editor` are taken by the Xcode extension's host app. The alternative is to rename that app.
2. **The slint-bindings dependency.** A git dependency pinned by `rev`, with the Swift and C# parts found through `cargo metadata`, as above. Alternatives: a submodule, or waiting for slint-bindings' packages (its T9, milestone M4).
3. **T68's scope.** Add G1–G4 to T68's done criteria, or as tasks in slint-bindings, because T69.1c–e cannot start without them. Drop or re-scope slint-bindings T12 (`slint-bindings-weft`) and T16 (a Slint desktop app for Weft): they duplicate `weft-slint` and T69.
4. **Windows verification.** No one in this workspace has built the WinUI code yet, and there is no Windows runner. Recommended: a Windows job after T73 lands (`cargo nextest run -p weft-studio -p weft-studio-ffi` and the Windows leg of `studio:test`), as its own task. The alternative is the creator's Windows machine, by hand.
5. **App Sandbox.** M1 runs unsandboxed with ad hoc signing. A sandboxed app that `NSOpenPanel` gives one file cannot read the `weft.json`, tokens and catalog in the directories above it (SPEC §10.1) — **unverified**, from Apple's App Sandbox guide, whose page renders by script. Choose in milestone 6: open a project folder, or ask for its directory.
6. **Save.** Always canonical; the alternative keeps the original bytes of an unedited non-canonical file.
7. **Input in M1.** A live preview: pointer and keyboard reach the screen, and its `perform` calls arrive as `action` events. The alternative is a static picture until M2's selection mode.
8. **Style.** `fluent` on both platforms, with no `weft.json` key while it is not a user option. The alternative is the platform style (`cupertino` on macOS).
9. **Slint licence for distributed builds.** The Royalty-free licence 2.0 needs the `AboutSlint` widget in an About dialog reachable from the top-level menu, or the Slint badge on the public download page; GPLv3 is the other no-cost licence. With native-only chrome, the badge. Decide before milestone 6; M1 ships nothing.
10. **Sample data.** Only the existing `render.data` key. The alternative, a `data.json` beside the screen as the corpus has, is a new convention and needs a SPEC change.

**Risks.**

- Open latency: every open compiles the screen and `std-widgets`; T69.1c measures it and caches the compiled definition if it is slow.
- App size: the interpreter carries the Slint compiler.
- Slint upgrades must move Weft and slint-bindings together (exact pins there; T69.1c checks for one `i-slint-core`).
- CPU rendering of large windows until slint-bindings' GPU milestone (its T7–T8).
- The surface is invisible to VoiceOver and Narrator until slint-bindings T14, so the UI tests check it by screenshot.
- `#[no_mangle]` functions of an rlib can be left out of a library that never references the crate; T69.1c references it and checks the exports.
- `slint-embed` is a path dependency inside the slint-bindings repository, under `flutter/`; T69.1c confirms that it resolves from the git checkout.

**Not in M1:** editing, patches, undo and redo, more than one screen, the canvas, selection, the layers tree and the inspector, recent files and the document browser, settings, autosave, canonical JSON documents, sandboxing, signing and packaging, GPU rendering, IME beyond ASCII, accessibility.

**Sources** (checked 2026-10-08):

- slint-interpreter 1.18.1: run-time loading, `ComponentInstance` implements `ComponentHandle`, the async build: https://docs.rs/slint-interpreter/1.18.1/slint_interpreter/
- Slint 1.18.1, experimental `ComponentContainer` and `component-factory`: https://github.com/slint-ui/slint/blob/v1.18.1/internal/compiler/typeregister.rs (`builtin` removes both) and https://github.com/slint-ui/slint/blob/v1.18.1/api/rs/slint/lib.rs (`ComponentFactory` hidden and deprecated)
- Slint licences 1.18.1: https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md and https://github.com/slint-ui/slint/blob/v1.18.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md
- Cargo git dependencies (found anywhere in the repository, locked in `Cargo.lock`): https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
- SwiftPM 6.4.0, unsafe flags make a product ineligible for other packages: https://github.com/swiftlang/swift-package-manager/blob/swift-6.4.0-RELEASE/Sources/Runtimes/PackageDescription/BuildSettings.swift
- XcodeGen 2.46.0, `${VARIABLE}` in the spec and local packages: https://github.com/yonaskolb/XcodeGen/blob/2.46.0/Docs/ProjectSpec.md
- Windows App SDK pickers (1.8 and later, `AppWindow.Id`, `PickFileResult.Path`; page dated 2026-07-15): https://learn.microsoft.com/en-us/windows/apps/develop/files/using-file-folder-pickers
- FlaUI v5.0.0 (2025-02-25; repository active, last push 2026-08-13): https://github.com/FlaUI/FlaUI
- slint-bindings at `2b9fa96` (2026-10-07): its `README.md`, `plan.md`, `done.md` (T2 frame times), `crates/slint-bindings-core/src/host.rs`, `crates/slint-bindings-ffi/src/lib.rs`, `swift/Package.swift`, `swift/Sources/SlintBindings/`, `windows/README.md`: https://github.com/listepo/slint-bindings/tree/2b9fa96b17464a628970ce1e1d6628b8c2580b30
- Apple App Sandbox, user-selected files (**unverified**: the page renders by script): https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox

### T12.3. Constrained generation: benchmark

Measure whether constrained JSON beats free markup on validity and edit success, and what it costs in output tokens.

**Out of scope (all of T12).** Any change to the markup format or the validator. Streaming of constrained JSON (T11 covers partial markup). Grammar-level decoding (GBNF, regex) for markup. Making constrained JSON the format agents are told to use.

**Context.**
- The benchmark has no structured output: `bench/src/provider.ts` has `Provider.complete(prompt: string)` only (Anthropic Messages and batches, and an OpenAI-compatible `/chat/completions` endpoint for LM Studio). The formats are `weft`, `html`, `jsx` and `a2ui` (`bench/src/neutral.ts`, `formats.ts`); replies are taken from a code fence (`extractDocument`). The method is in `test.md` (strict validation, one repair prompt, 3 samples, a History row for every kept run); `bench/src/primers.ts` changes only as a method change.
- The repository records nothing about what each provider supports for structured output (recursive `$ref`, `anyOf` size, schema size limits). Those facts are checked when the task is claimed and written into `research.md` with sources.

**Scope.** `Provider.complete(prompt, { schema? })` for Anthropic and OpenAI-compatible servers; two new formats, `weft-json` (free canonical JSON) and `weft-json-constrained`, to separate "JSON instead of markup" from "constrained instead of free"; a JSON primer and an adapter that reuses the Weft neutral tree; corpus JSON derived from `screen.weft` at run time (no second committed copy); the same repair rule; `test.md` method sections, a trial run with its History row, and a results section in the T8 evaluation report.

**Done when.**
- A mocked-provider test shows that the schema is sent and that the reply is scored by the same checks as every other format.
- A trial run is in `test.md` History with its raw results in `bench/results/`, and the comparison (valid and success, first try and after repair, mean output tokens) for markup, free JSON and constrained JSON is in the evaluation report.

**Dependencies.** T12.1 and T12.2. T8 (the harness, models and evaluation report; it is in progress in `bench/src/run-tasks.ts`, so the two are sequenced). T28 step 4 (a readback method change to the same harness).

**Open questions for the creator.**
1. Three-way (with the free `weft-json` format) or two-way? Which models (Anthropic plus a local LM Studio model)? What budget? Part of T8's continue/stop decision or a separate report? (T12 question 5.)
2. If a provider cannot take the schema whole (size or recursion limits): a reduced profile per provider, or report the provider as unsupported? (T12 question 6.)
3. Narrow the schema with project data (token names, action names, data paths as enums) once the measurement shows whether it is worth it? T12.1 describes the catalog only. (T12 question 1.)

### T16. Layout vocabulary

Parent of T16.2–T16.8, which carry the work (the task was split this way when T16.1 was claimed). T16.1, the design, is done (`done.md`): the creator approved `docs/layout-design.md` with its twelve decisions, which T16.2–T16.8 build. This card holds the context, scope and done criteria they share.

Weft has two layout kinds today. `stack` lays children out in one line, with `direction`, `gap`, cross-axis `align` and `wrap`; `grid` has a fixed number of equal `columns` and a `gap`. Most of what a layout usually says has no form in Weft: how leftover space on the main axis is shared, which child grows and which hugs its content, inner spacing, a maximum width, and how a grid reflows on a narrow screen. Screens work around these gaps, and every importer drops them as `layout` losses. The task decides how much more Weft says without becoming CSS: a small closed set that every target (web, SwiftUI, Slint, A2UI, json-render, Figma, Penpot) can draw and read back, and nothing that only CSS can express. This changes the format, so the design comes first, as in T39.

**Context.**
- `SPEC.md` §5.1 holds the `stack`/`grid` rows and the `stack.align` note (T55 set its default: a row centres its children, a column keeps its host's layout). §2.2 holds the universal attributes; T52's tilt attributes are the precedent for a per-child attribute. §8: a new prop is a minor catalog change, a new or changed default is major. §9 loss tables: the SwiftUI importer drops `padding` and `frame`, Slint loses grid `row`/`col` placement, A2UI export loses `columns` and `wrap`, A2UI import notes `justify` other than `start` and `weight`.
- Catalog: `packages/catalog/src/core.ts` generates `packages/catalog/catalog.json`, embedded by `crates/weft-catalog/src/core.rs`; examples `packages/catalog/examples/stack.weft` and `grid.weft`; change classifier `crates/weft-catalog/src/diff.rs`.
- Generators: `crates/weft-web/src/html.rs` `layout()` and `base.css`; `crates/weft-web/src/jsx/` (React, SolidJS and Lit share one render plan, `_align` in `jsx/runtime.rs`); `packages/render-react/src/render.ts` `layoutStyle`; `crates/weft-swiftui/src/generate.rs` (`VStack`/`HStack` alignment, `LazyVGrid`, no wrap); `crates/weft-slint/src/generate.rs` (writes `spacing` only); `crates/weft-interop/src/a2ui/export.rs` and the json-render export (T13.1); `packages/design-tool/src/view.ts` `layoutView`, used by the Figma and Penpot builds (it draws `stretch` as `start`).
- Importers: `crates/weft-web/src/dom.rs` and `from_jsx/`; `crates/weft-swiftui/src/import/read.rs`; `crates/weft-slint/src/read.rs`; `crates/weft-interop/src/a2ui/import.rs`; `packages/design-tool/src/read.ts` and `foreign.ts`. Figma `primaryAxisAlignItems` and sizing modes are already typed in `packages/figma/src/api.ts`, Penpot `horizontalSizing`/`verticalSizing` in `packages/penpot/src/api.ts`.
- Corpus workarounds: `dashboard` separates the title and the update time with a `space.xl` gap in a wrapping row, right-aligns its footer with a column `align="end"`, and its 3-column `grid` never reflows; `wizard-step` cannot push Back and Next apart. `crates/weft-snapshots/tests/coverage.rs` fails while any catalog prop or enum value appears in no corpus screen, so new props need a corpus screen.
- Design (done, T16.1): `docs/layout-design.md`, approved, with its **Decisions** section as the contract for the rest; prior art in `research.md` §21. Decision 6 settles the overlap with `docs/figma-style-overrides-design.md`: `padding` lives here, so T14 adds no `style-padding` for `stack` and `grid`.

**Scope.**
1. Design (done, T16.1): a `research.md` prior-art section with URLs and dates (A2UI `justify`/`align`/`weight`, Figma auto layout hug/fill/fixed and min/max, Penpot flex/grid sizing, SwiftUI `Spacer`/`frame`/`layoutPriority`/`ViewThatFits`, Slint `alignment` and `*-stretch`, CSS flex/grid/container queries, Compose), and `docs/layout-design.md` in the shape of `docs/context-design.md`: the candidate vocabulary (main-axis distribution on `stack`, child sizing, padding, max width, grid reflow), markup and canonical JSON, catalog props versus universal attributes, token types, defaults, diagnostic codes, a mapping and loss table per target, versioning, and worked examples on `dashboard` and `wizard-step`. Nothing below starts until the creator approves the design.
2. Spec and catalog: `SPEC.md` §5.1 or §2.2 and the §9 mappings and loss tables; the `AGENT-SPEC.md` layout line and `packages/mcp/src/primer.ts`; `core.ts` and a regenerated `catalog.json`, the catalog examples, `docs/catalog-and-tokens.md`; for a new universal attribute or code, `crates/weft-core/src/rules.rs` and the differential fixtures.
3. Generators: static HTML and the base stylesheet, React/SolidJS/Lit, the reference renderer, SwiftUI, Slint, A2UI, json-render, and the Figma and Penpot builds through `design-tool`.
4. Importers: HTML/DOM, React/SolidJS, SwiftUI, Slint, A2UI and Figma/Penpot read the new vocabulary back, and the `layout` loss rows narrow.
5. Corpus and baselines: replace the workarounds in `dashboard` and `glass` and add a non-benchmark `corpus/layout` screen, so coverage holds, leaving the benchmark screen `wizard-step` unchanged (decision 10); retake and review the insta snapshots, the Figma/Penpot layer trees, and the `packages/visual` web and SwiftUI baselines.
6. `weft.json`: no key is expected; if the design adds a tool option (for example breakpoints that are not tokens), it gets a §10.6 row and a `settings.rs` entry in the same change.

**Out of scope.** Arbitrary CSS (lengths outside tokens, absolute positioning, per-side margins, z-index). Visual style (fill, stroke, radius), which belongs to the style-overrides proposal. Animation. Changing T55's cross-axis default. `bench/src/primers.ts`, which changes only as a method change recorded in `test.md`.

**Done when.**
- The creator has approved the design (done, T16.1).
- The new props or attributes validate in strict and lenient mode, and their codes are in `AGENT-SPEC.md` (`bench/test/agent-spec.test.ts` passes).
- A corpus screen that uses each one passes the `packages/visual` cross-target comparison (React, SolidJS, the reference renderer) and has a reviewed SwiftUI baseline.
- Every target either maps each new form or lists it as a loss in §9; generated code with its source comment removed reads back to the same props on every importer whose row says "maps"; the Figma and Penpot round trips keep it.
- The full check exits 0.

**Dependencies.** T55 and T52 (done) are the precedents. T39 is independent: this ships within `weft` 0.1 (decision 9). T14's style overrides add no `style-padding` for `stack` and `grid` (decision 6). T67.4 and T67.5, which edit the same Slint `read.rs`, are done. T69's "resize within the layout rules" builds on this.

**Split.** T16.1: design (done). T16.2: spec, catalog, core, `AGENT-SPEC.md`, tokens, corpus. T16.3: web generators and the reference renderer. T16.8: web importers. T16.4: SwiftUI generator and importer. T16.5: Slint generator and reader. T16.6: A2UI and json-render (done). T16.7: Figma and Penpot through `design-tool`. T16.2 lands first; the others need only T16.2 and can run in parallel, except that T16.8 reads what T16.3 writes. Each closes its part of T16's "Done when" for its targets and moves its §9 rows from loss to mapping.

### T16.3. Layout vocabulary: web generators and the reference renderer

T16 scope items 3 and 5 for the web, as the design's generate table says. Depends on T16.2.

Steps:

1. `crates/weft-web/src/html.rs`: `data-justify` and `data-grow` with rules in the layout stylesheet; inline `padding`, `width: 100%; max-width` and the capped `auto-fill` grid template, all through `var(--weft-…)`.
2. The shared JSX render plan (`crates/weft-web/src/jsx/`) for React, SolidJS and Lit, with the same declarations as inline styles.
3. `packages/render-react/src/render.ts`: `layoutStyle` for the stack and grid props, and `grow` in `renderNode` beside the tilt.
4. §9 web rows move from loss to mapping; insta snapshots and the `packages/visual` web baselines retaken and reviewed for `dashboard`, `glass` and `layout`.

Size: about 250 lines of code.

Done when: the three screens pass the `packages/visual` cross-target comparison (React, SolidJS, the reference renderer), a narrow-viewport case shows the stats grid at one column and a wide one at three, and `moon run root:changed` exits 0.

### T16.8. Layout vocabulary: web importers

T16 scope item 4 for the web, as the design's import table says. Depends on T16.3, whose output it reads back.

Steps:

1. `crates/weft-web/src/dom.rs`: `data-justify`, `data-grow`, and the inline `padding`, `max-width` and grid template when they name `--weft-` properties.
2. `crates/weft-web/src/from_jsx/`: the same conventions in inline styles.
3. Foreign markup: `justify-content` among the four values, any positive `flex-grow` as `grow`; `space-around`, `space-evenly`, `stretch`, unequal grow ratios, raw lengths and media or container queries are `layout` losses. §9 rows updated.

Size: about 250 lines of code.

Done when: the generated HTML and JSX of `dashboard`, `glass` and `layout`, with the source comment removed, read back to the same props; a test covers each listed loss; `moon run root:changed` exits 0.

### T16.4. Layout vocabulary: SwiftUI

T16 scope items 3 to 5 for `crates/weft-swiftui`, as the design's tables and decisions 4 and 5 say. Depends on T16.2.

Steps:

1. Generator: `justify` on rows as spacers (`space-between` as `HStack(spacing: 0)` with `Spacer(minLength: <gap>)` between children, also inside `<each>`); `grow` on a row child as `.frame(maxWidth: .infinity)`; `padding` as `.padding(theme.<path>)`; `max-width` as the pair of frames; `min-column-width` as a small generated `Layout` that caps at `columns`. `justify` and `grow` in a column stay inert markers and §9 lists them.
2. Importer: read those patterns and markers back; a `padding` or `frame` that is not a theme token stays a `layout` loss.
3. Insta snapshots, and the reviewed SwiftUI baselines in `packages/visual` (`WEFT_SIMULATOR=own` in a worktree). If the generated `Layout` cannot match the web baseline, fall back to `GridItem(.adaptive(minimum:))` and record the difference in §9 (decision 5).

Size: about 350 lines of code, including the generated `Layout` helper.

Done when: the three screens compile, match their reviewed baselines, and read back to the same props with the source comment removed; `moon run root:changed` exits 0.

### T16.5. Layout vocabulary: Slint

T16 scope items 3 to 5 for `crates/weft-slint`. Depends on T16.2 (T67.4 and T67.5 are done).

Steps:

1. Check first whether a stack without `alignment` stretches its children in Slint 1.18.1 (`research.md` §21 marks it **unverified**); if it does, write `alignment: start` on every stack and retake the screenshots.
2. Generator: `alignment` for `justify`, `horizontal-stretch`/`vertical-stretch: 1` for `grow` (and `0` on its siblings), `padding` and `max-width` in px; `min-column-width` is kept in the source comment and drawn with `columns`.
3. Reader: those properties back, px values that equal exactly one dimension token as that token; other values are `layout` losses. §9 rows updated.

Size: about 150 lines of code.

Done when: the three screens compile, their screenshots are reviewed, and they read back to the same props without the source comment; `moon run root:changed` exits 0.

### T16.7. Layout vocabulary: Figma and Penpot

T16 scope items 3 to 5 for the design tools. Depends on T16.2.

Steps:

1. `packages/design-tool/src/view.ts` `layoutView`: `justify` as the main-axis alignment, `grow` as fill on the parent's main axis, `padding` and `max-width` (bound to variables in Figma, px in Penpot), `min-column-width` drawn with `columns` and kept in plugin data.
2. `read.ts` and `foreign.ts`: Weft-built layers from plugin data as now; foreign layers by the import table (four equal paddings, one token, fill as `grow`); the rest are `layout` losses.
3. Layer-tree snapshots of the Figma and Penpot builds and the round trips for the three screens. §9 rows updated.

Size: about 250 lines of code.

Done when: the Figma and Penpot round trips of `dashboard`, `glass` and `layout` keep every prop, a foreign-layer test covers each listed loss, and `moon run root:changed` exits 0.

### T17. Extension catalogs

Parent of T17.1–T17.3, which carry the work (the task was split this way when T17.0 was claimed). T17.0, the design, and T17.1, the spec and loader, are done (`done.md`); for T17.0 the creator approved `docs/extension-catalogs-design.md` with its twelve decisions, which T17.1–T17.3 build. This card holds the context, scope and done criteria they share.

A project can extend the core catalog with exactly one file today. Real hosts combine several sources: the core, one or more component libraries, and their own kinds. So a project needs to load several catalogs at once without their kinds colliding. Once catalogs travel between projects, two more questions need answers: how a catalog is published and found, and who keeps the registry. `research.md` §9 lists "Who keeps a registry of extension catalogs" as open; `docs/catalog-and-tokens.md` and `docs/what-is-weft.md` both say there is no registry; `docs/fragments-design.md` defers namespaces and sharing across projects to this task.

**Context.**
- SPEC §10.2: `catalog` is one file name. §10.4 merges that one extension into the core catalog: new kinds need a whole definition, core kinds are extended by merge or join, the extension may only widen the core (§8 rules via `crates/weft-catalog/src/diff.rs`), codes `W706` (invalid) and `W707` (narrows), and the merged catalog takes the extension's `name` and `version`.
- Code: `Loader::extend_catalog` and `extend_definition` in `crates/weft-catalog/src/project.rs`. `Project.catalog` is a single `Catalog` with no record of which catalog each kind came from. The CLI's `--catalog` loads one whole catalog that replaces the project's (`load_catalog` in `crates/weft-cli/src/main.rs`). `weft_capabilities` already returns `catalogs: [{ name, version }]`, the core first and then the extension (T10); `weft_catalog` lists kinds with no source.
- Kind names follow `[a-z][a-z0-9]*(-[a-z0-9]+)*` (SPEC §2), so there is no `:`. `x-<vendor>-` names are opaque extensions that no catalog may define (§8, §10.4).
- T15.1/T15.2 import a Custom Elements Manifest into an extension whose kinds are the custom-element tags (`acme-button`), with `--name`/`--version`/`--catalog` and the `import.cem.*` settings (`crates/weft-import/src/cem.rs`, `crates/weft-cli/src/cem.rs`, fixture `crates/weft-import/tests/fixtures/cem/acme-ui.json`). A library catalog and a project catalog cannot be used together today.
- Generators already name extension kinds by convention (SwiftUI: `promo-card` becomes `PromoCardView`, SPEC §9); the Figma and Penpot libraries build one component set per kind.
- `@weft/catalog` is `private` in `packages/catalog/package.json`; nothing is published anywhere yet.
- Documents and validators never touch the network (AGENTS.md). Project file names stay inside the project directory (§10.2, `W703`); decision 8 makes package lookup in parent `node_modules` folders a narrow, documented exception.

**Scope.**
- Design (done, T17.0): `docs/extension-catalogs-design.md`, approved, with its **Decisions** section as the contract for the rest.
- Build: SPEC §5, §10.2 (`catalog` also takes an ordered array), §10.4, and new `W7xx` codes for cross-catalog conflicts; the loader keeps which catalog each kind came from, with differential fixtures; CLI `--catalog` can be repeated; MCP `project.catalog` accepts an array, `weft_capabilities` lists every catalog, `weft_catalog` shows each kind's catalog; `import-cem` gets a namespace option with its `import.cem.*` key (SPEC §10.6, `settings.rs`, `schemas/weft.schema.json`); `AGENT-SPEC.md` §1 and §2.8 and the primer; `docs/projects.md`, `docs/catalog-and-tokens.md`, and a publishing guide; the `research.md` §9 row answered.

**Out of scope.** Hosting a registry service. Network fetching during validation or rendering. Changing the `x-` extension rules. Fragments themselves (T31 part B). New generator features beyond naming namespaced kinds.

**Done when.**
- `examples/project` loads the core plus two extensions (one imported from `acme-ui.json`, one written by hand), and kinds from both validate in strict mode.
- A kind defined by two catalogs gets a diagnostic that names both catalogs, never a panic.
- `weft_capabilities` lists all three catalogs; the TypeScript and Rust differential fixtures agree.
- A catalog shipped in a package directory resolves by the documented convention with no network access.
- `schemas/weft.schema.json` is regenerated; SPEC, AGENT-SPEC and the docs are updated.

**Dependencies.** Done: T31 (project file and settings), T15.1/T15.2 (CEM import), T10 (capabilities). T12: its schema must cover the merged catalogs. Affected: T14/T40 (one design-tool library per catalog), T44 (SwiftUI custom views), T69 milestone 4 (catalog kinds as components), the fragments design (T31 part B).

**Decisions** (creator, recorded in `docs/extension-catalogs-design.md`): hyphen prefixes declared by the catalog author (`"prefix": "acme"`), no aliases; libraries define only their own kinds, and the one catalog without a prefix, the project catalog, alone extends core and library kinds, widen-only; `requires` with Cargo's compatibility rule, unmet as the warning `W714`; `catalog` as a file name or an ordered array of at most 32 file and `{ "package" }` entries, libraries merged before the project catalog; new codes `W711`–`W714`; committed files first, npm packages with a `weft.catalog` field later, found by walking up `node_modules`, never over the network; a curated `docs/catalogs.md` and the npm keyword `weft-catalog` instead of a registry service; one design-tool build grouped by catalog; screens stay silent about their catalogs; a repeated `--catalog` replaces the list, and `import-cem` gets `--prefix` with a hint when it is absent.

**Split.** T17.0: design (done). T17.1: spec and loader for several catalogs and prefixes, with fixtures (done). T17.2: CLI, MCP, `import-cem --prefix`, settings, design tools, AGENT-SPEC and docs. T17.3: package entries and lookup, the publishing guide and the curated list. T17.1 lands first; T17.2 and T17.3 build on it.

### T17.2. Extension catalogs: tools and docs

T17's surfaces, by decisions 10 and 12. Depends on T17.1.

- CLI: `--catalog` can be repeated and replaces the project's whole list, in order; a file named `weft-core` replaces the base.
- MCP: `project.catalog` takes a catalog object or an array; `weft_capabilities` lists every catalog after the core, with `prefix` and `source` when known; `weft_catalog` gives each kind its `catalog` and, when widened, `extendedBy`.
- `import-cem`: `--prefix` and `import.cem.prefix` (SPEC §10.6, `settings.rs`, regenerated `schemas/weft.schema.json`); with a prefix it writes `prefix` and `requires: { "weft-core": … }` and reports a tag outside the prefix as a `kinds` loss; without one it prints a hint when every kept tag shares a first segment. Regenerates `examples/project/catalogs/acme-ui.catalog.json` unchanged.
- Design tools (`packages/design-tool`): `weft.library` lists every catalog; components grouped by catalog (a section per catalog in Figma, a path `<catalog> / <kind>` in Penpot); reading back unchanged.
- `AGENT-SPEC.md` §1 (catalogs, the array) and §2.8 (`acme-button` from a listed catalog is a catalog kind, `x-acme-button` an opaque extension), the primer and tool descriptions (a catalog description is never an instruction); `docs/projects.md` and `docs/catalog-and-tokens.md`.
- Done when `weft_capabilities` lists all three catalogs of `examples/project` and the full check exits 0.

### T17.3. Extension catalogs: distribution and registry

T17's sharing, by decisions 7–9. Depends on T17.1 (and T17.2 for `source` in `weft_capabilities`).

- SPEC §10.2 and the loader: `{ "package": "<npm name>" }` entries; an invalid npm name is `W703`; the lookup tries `node_modules/<name>/package.json` in the project directory, then each parent, as Node does, and reads only that file and the file its `weft.catalog` names (§10.2 rules relative to the package directory); not found or unreadable is `W704`; nothing runs and nothing is fetched. CLI and Node tools only; the MCP server still takes contents. The documented exception to §10.2 goes into SPEC.
- A test package in a temporary `node_modules` (nested above the project directory too) that resolves with no network access.
- Docs: `docs/publishing-catalogs.md` (prefix, `requires`, the `package.json` fields `weft.catalog`, `files` and the keyword `weft-catalog`), `docs/catalogs.md` as the curated list of catalogs and taken prefixes, kept by the creator; `docs/what-is-weft.md` and `docs/catalog-and-tokens.md` stop saying there is no registry; the `research.md` §9 row marked built.
- Done when T17's fourth done criterion holds and the full check exits 0.

### T41. Hosted Penpot plugin

Penpot installs a plugin only from the URL of its `manifest.json`, and plugins are hosted outside Penpot. Today the Weft Penpot plugin is available only to someone who clones the repository and runs `moon run penpot-plugin:serve`. The repository is public on GitHub (`listepo/weft`), so CI can build the plugin and publish `dist/` on GitHub Pages, which serves the files with the right content types and `Access-Control-Allow-Origin: *`.

**Context.**
- `plugins/penpot`: `build.ts` runs the shared `@weft/design-plugin` build and writes `dist/manifest.json`, `plugin.js` (the SES sandbox script, no WebAssembly) and `ui.html` (the core inlined). `manifest.json` uses version 2, so paths resolve from the manifest's folder and a subpath works; it has `name`, `description`, `code` and `permissions`, but no `icon`. `moon.yml` has `build` (depends on `root:wasm`, `runInCI: false`) and `serve` (`serve.config.ts`, port 4400, `cors: true`). `dist/` is gitignored; `test/bundle.test.ts` builds into a temp folder and checks the manifest.
- `plugins/penpot/README.md` says hosting is T41 and that nobody has checked whether a real Penpot needs the CORS header; `packages/penpot/README.md` lists hosting among the unverified items, and T40's result in `done.md` leaves hosting open. The plugin's UI needs no network access.
- Plugin identity: Penpot derives `host` from the manifest URL and gives each install a random id, unless a plugin with the same name and host is already registered. Weft keeps shared plugin data under the `weft` namespace, so boards built with a localhost install stay readable from the hosted one.
- GitHub: `main` has no `.github/` folder, and nothing mentions Pages. Open PR #4 (T73) adds `.github/workflows/ci.yml` (the merge gate on pull requests and pushes to `main`, `runs-on: xcode-27`, actions pinned to commit SHAs, `permissions: contents: read`) and moves XcodeGen to `mise.osx.toml` so `mise install` works on Linux. Pages is not enabled (`GET /repos/listepo/weft/pages` returns 404).

**Scope.**
- A Pages deploy job next to T73's gate: only on pushes to `main`, after the gate passes; builds with `moon run penpot-plugin:build` and publishes `plugins/penpot/dist` under a fixed path through `actions/upload-pages-artifact` and `actions/deploy-pages`, pinned to SHAs; `pages: write` and `id-token: write` on that job only, the `github-pages` environment, and a concurrency group so two deploys never race.
- A check step after the deploy: fetch `manifest.json`, `plugin.js` and `ui.html` from the Pages URL and fail unless each returns 200 with a JSON, JavaScript or HTML content type and `Access-Control-Allow-Origin: *`.
- An offline Vitest test that reads the workflow and pins that the deploy runs only on `main` after the gate, builds the Penpot plugin, publishes exactly its `dist/`, and pins every action to a SHA.
- An `icon` in `manifest.json` if Penpot's plugin manager needs one; the build copies it into `dist/`, and `bundle.test.ts` checks it.
- Docs: `plugins/penpot/README.md` gives the install URL first and keeps local serving for development; the root `README.md` mentions the URL; `packages/penpot/README.md` moves hosting from unverified to verified, with the date.
- No new tool option, so no `weft.json` key.

**Out of scope.** Listing the plugin in Penpot's own plugin catalogue. Hosting the Figma plugin. Versioned or per-PR preview deployments. A custom domain. T40's other open checks in a real Penpot (async variants, tokens, grid cells, plugin data on copies, UI theme).

**Done when.**
- A push to `main` deploys, and the check step passes.
- Penpot (design.penpot.app) installs the plugin from the Pages URL in its plugin manager, and Build and Export both work on a corpus screen.
- `plugins/penpot/README.md` gives that URL.
- The full check exits 0.

**Dependencies.** T73: extend its `ci.yml` rather than adding a second gate; T73 also makes `mise install` work on Linux. The creator enables Pages with the source set to "GitHub Actions" (a repository setting; agents do not change it).

**Open questions for the creator.**
1. URL: `https://listepo.github.io/weft/penpot/manifest.json` (a subpath, leaving room for other pages) or the site root?
2. Deploy on every push to `main`, or only on a release tag, so users never get an unreleased plugin?
3. Runner for the deploy build: GitHub-hosted `ubuntu-latest` or the gate's `xcode-27` runner?
4. A repository has one Pages site and each deploy replaces it whole: should the deploy assemble a site now (an index page, room for docs or a `render-react` gallery)?
5. Which icon should the plugin manager show?
