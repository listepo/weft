# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 70% | Claude Code / claude-opus-5-5 |
| T14 | in progress | P2 | 5 | 75% | Claude Code / claude-opus-5-5 |
| T69 | todo | P2 | 5 | 0% | |
| T69.1 | in progress | P2 | 4 | 40% | Claude Code / claude-opus-5-5 |
| T106 | in progress | P2 | 5 | 95% | Claude Code / claude-opus-5-5 |
| T106.7 | in progress | P2 | 4 | 95% | Claude Code / claude-opus-5-5 |
| T105 | in progress | P2 | 5 | 30% | Claude Code / claude-opus-5-5 |
| T105.3 | todo | P2 | 4 | 0% | |
| T105.4 | todo | P2 | 4 | 0% | |
| T105.5 | todo | P2 | 4 | 0% | |
| T105.7 | in progress | P2 | 3 | 10% | Claude Code / claude-opus-5-5 |
| T105.10 | todo | P2 | 2 | 0% | |

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
  - MCP / Claude Code tools (with T30): `use_figma` reads and writes the shared plugin data (T14.4); the Weft tools on top of it are still to be designed;
  - a check of the plugin in the real Figma app, including the inline module script and WebAssembly in its UI iframe;
  - the open questions in the report: single components for kinds without variants, `state` as a variant axis, and the manifest id (plugin data on duplicate and detach is settled by T14.4).

Execution plan, stage 2 (this round; style overrides stay out until the creator approves them):

1. T14.1: the pull over the REST API — done (`done.md`).
2. MCP / Claude Code tools through the Figma MCP server. The Figma MCP server's `use_figma` tool runs Plugin API code, but its own description says `setPluginData` is not supported there, and plugin data is readable only by the plugin that wrote it. The creator chose to move the Weft source to shared plugin data, which any plugin and the REST API (`plugin_data=shared`) can read: T14.2, done (`done.md`).
3. T14.3: the creator dropped files that keep the Weft source in private plugin data; only shared plugin data is supported — done (`done.md`).
4. T14.4: `use_figma` reads and writes shared plugin data, checked in a real test file; the fake follows real duplicate, detach and instance inheritance — done (`done.md`).


### T69. Figma-like desktop design editor

A desktop design editor in the spirit of Figma for Weft documents, on macOS and Windows. Weft is the document format: an editor document is a set of Weft screens (`.weft`, canonical markup and JSON, SPEC §2–§3) with the project's catalog and tokens (SPEC §10), so whatever the editor saves validates, formats and converts with the existing tools, and agents edit the same files through the CLI and MCP server.

**Prerequisite: the Slint bindings.** The macOS (Swift) and Windows (WinUI 3) integration MUST use the Slint bindings of T68: the project `~/GitHub/listepo/apps/slint-bindings`, repository `listepo/slint-bindings` (https://github.com/listepo/slint-bindings), with its crates `slint-bindings-core` (custom `slint::platform`, software renderer, window adapter, input) and `slint-bindings-ffi` (C ABI), its `swift/` package (a Slint surface in SwiftUI through `NSViewRepresentable`) and its `windows/` package (the same surface in WinUI 3). No separate or ad-hoc embedding lives in Weft. A gap found there (input, IME, focus, DPI, accessibility, rendering, lifecycle) is fixed in slint-bindings and consumed from it, never worked around here. T68 is done (`done.md`). Weft → Slint is done: T67 and T67.1–T67.5 (`done.md`) cover the whole catalog and the read-back of edited Slint.

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

The creator approved T69.1, the plan of the first milestone; its sub-tasks T69.1a and T69.1b are done.

### T69.1. Editor skeleton

Milestone 1 of T69: a native window on macOS and on Windows hosts one Slint surface through slint-bindings, shows one corpus screen rendered by `weft-slint`, and opens and saves `.weft` files through the native file dialogs. The creator approved this plan and every decision below with its recommendation. It splits into five sub-tasks, T69.1a–T69.1e, each within the 500-line limit and each claimed as its own row with the plan below. Facts were checked on 2026-10-08; sources are at the end of the card.

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

**G1–G4 are delivered** in slint-bindings `main` at `8971685e01fee6776e5121dffb13e3eb43709437` (the repository moved to `pyrlyn/slint-bindings`; `listepo/slint-bindings` redirects); T69.1c pins that `rev`. G1 and G2: its T11.1 (`291376b`): `slint-interpreter` components, `sb_compile`, `SbHost::new` and `SbHost::into_raw`. G3: its T70 (`a149da9`): `SlintHost(adopting:)`, `SlintNSView` and the SwiftUI `SlintView` take the host from a factory, the `SlintBindings` library target links no Rust library (`just swift-package-check`), and a test shows `crates/weft-slint/tests/fixtures/login.slint` in a SwiftUI window and types into it. G4: its T71 (`8971685`): `SlintHost.Adopt` and `SlintPanel.HostFactory`, MSTest host tests on `windows-latest`, and a CI run that opens the WinUI demo on the same screen and checks the window capture. `sb_host_pixel_size` (T70) gives an adopted host's size.

T68's done criteria ("a Slint component generated by T67 shows and takes input inside a SwiftUI window") cover G1 and part of G2, not G3 or G4 (decision 3). slint-bindings' own plan overlaps Weft: its T12 builds `slint-bindings-weft`, a second Weft → Slint mapping beside `weft-slint`, and its T16 a Slint desktop app for Weft beside T69.

**Sub-tasks** (order: T69.1a and T69.1b now and in parallel; T69.1c after G1–G2; T69.1d after T69.1c and G3; T69.1e after T69.1c, G4 and decision 4):

- **T69.1a. Document core**: done (`done.md`). **T69.1b. Slint runtime**: done (`done.md`).
- **T69.1c. Editor C ABI** (`crates/weft-studio-ffi`, about 350 lines). The git dependency, the `ws_*` calls above, the header and the C# bindings with their drift test (cbindgen and csbindgen, both in `rust.md`), and the `toolchain.md` rows. Done when `tests/e2e.rs` passes headless: `login` opens, `ws_surface_new` at 480×800 and scale 1 renders pixels that are not all transparent through `sb_host_render`; a press and release over the submit button raise one `action` event with the screen's action and id; a save round-trips through the ABI; null pointers, a path that is not UTF-8, a missing file and a refused screen return `false` or `NULL` with a message; `cargo metadata` shows one `i-slint-core`; and the macOS static library exports both `ws_open` and `sb_host_render` (`nm`). Measure the open-to-first-frame time of the largest corpus screen in release and record it here; above 500 ms, cache the compiled definition per source.
- **T69.1d. macOS shell** (`studio/macos`, about 350 lines of Swift). One window per document, a File menu with Open… (⌘O), Save (⌘S) and Save As… (⇧⌘S), `NSOpenPanel` and `NSSavePanel` filtered to `.weft` (the app imports `dev.weft.screen`, which the editor extension exports) behind a small `FilePicking` protocol, an alert with the last error and the diagnostics, the window title from the state, and slint-bindings' `SlintNSView` with the host from `ws_surface_new`. Done when Swift Testing unit tests of the document model pass with a fake picker; an XCUITest launches the app with a corpus screen as a launch argument, finds the window titled `screen.weft`, sees a surface screenshot that is not one colour, and Save As through the fake picker writes the same bytes; and `studio/test/macos.test.ts` runs XcodeGen, `xcodebuild build` and `xcodebuild test` (arm64). A manual checklist in `studio/README.md` covers the real dialogs, a Retina display and live resize.
- **T69.1e. Windows shell** (`studio/windows`, about 350 lines of C#). `MainWindow` with a `MenuBar` (Open Ctrl+O, Save Ctrl+S, Save As Ctrl+Shift+S as `KeyboardAccelerator`s), `FileOpenPicker` and `FileSavePicker` from `Microsoft.Windows.Storage.Pickers` (Windows App SDK 1.8 and later: constructed with `AppWindow.Id`, the result's `Path` is the file) behind an `IFilePicker` interface, a `ContentDialog` for errors, slint-bindings' `SlintPanel` with the host from `ws_surface_new`, and P/Invoke from the generated `NativeMethods.g.cs`. Done when MSTest unit tests of the document model pass with a fake picker; a FlaUI (UIA3) smoke test launches the app with a corpus screen, checks the window title and a panel capture that is not one colour, and Save As writes the same bytes; and `studio/test/windows.test.ts` runs `dotnet build` and `dotnet test` on Windows.

**Done when** the four checks above pass on their platforms, the full check exits 0, every corpus screen opens and renders in both apps through the native open dialog and saves through the native save dialog (byte-identical when unedited), and neither shell contains embedding code of its own: the surfaces are slint-bindings' `SlintNSView` and `SlintPanel`.

**Decisions** (the creator approved the recommendation, the first option, of each):

1. **Name.** WeftStudio: folder `studio/`, crates `weft-studio` and `weft-studio-ffi`, bundle id `dev.weft.studio`, because WeftEditor and `dev.weft.editor` are taken by the Xcode extension's host app. The alternative is to rename that app.
2. **The slint-bindings dependency.** A git dependency pinned by `rev`, with the Swift and C# parts found through `cargo metadata`, as above. Alternatives: a submodule, or waiting for slint-bindings' packages (its T9, milestone M4).
3. **T68's scope.** Add G1–G4 to T68's done criteria, or as tasks in slint-bindings, because T69.1c–e cannot start without them. Drop or re-scope slint-bindings T12 (`slint-bindings-weft`) and T16 (a Slint desktop app for Weft): they duplicate `weft-slint` and T69. Approved and recorded here only: slint-bindings and T68 belong to another agent, which takes G1–G4 and the re-scope there.
4. **Windows verification.** No one in this workspace has built the WinUI code yet, and there is no Windows runner. Recommended: a Windows job after T73 lands (`cargo nextest run -p weft-studio -p weft-studio-ffi` and the Windows leg of `studio:test`), as its own task. The alternative is the creator's Windows machine, by hand. Approved: T69.1f in `roadmap.md`.
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

### T106. Weft plugin for cox

The creator asked for a plugin for cox (`~/GitHub/listepo/apps/cox`, https://github.com/pyrlyn/cox), written in Rust, and allowed extending cox where its plugin API falls short. A cox plugin is a WebAssembly module (extism guest, `wasm32-unknown-unknown`) with a `plugin.toml` manifest, written with cox's Rust SDK (`cox-plugin-sdk`, `plugins/sdk` in cox); its `init` returns the model-callable tools (`ToolSpec`s), served through `cox_tool_call`. The plugin gives the cox agent the Weft tools the MCP server gives other hosts (same names, inputs and outputs) by calling the Rust crates directly: no Node, and no second copy of the behaviour. Later parts give the cox desktop app a Design tab, like Claude Design: the agent's screens rendered as real UI that a person can look at and select from.

Done when T106.1–T106.8 are done. Sub-tasks, in order (each within the task size rule):

- T106.1: the MCP tool logic in Rust (`weft-binding`).
- T106.2: the TypeScript MCP server on that Rust logic.
- T106.3: `plugins/cox`, the cox plugin with the tools.
- T106.4: the accessibility snapshot of `weft_render` in Rust, so cox gets `weft_render` too.
- T106.5: skills in cox: a cox extension for plugin-carried skills, and the Weft skills in the plugin.
- T106.6: Design tab, design: these cards and a cox design proposal (`docs/design/` in cox), before code.
- T106.7: Design tab, cox side: the plugin-API extension and the desktop host (cox tasks).
- T106.8: Design tab, Weft side: the plugin renders the screens, keeps the tab live and passes the selection to the agent.

What exists (checked on origin/main): the MCP server is `packages/mcp` (nine tools: `weft_primer`, `weft_capabilities`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render`, `weft_context`), bundled into the Claude Code, Cursor and Open Design plugins by `plugins/shared`. Its checks already run on the Rust core through `@weft/core` and `@weft/catalog`, but the tool layer itself (primer text, limits, diagnostics truncation, project scoping, the capabilities shape, the catalog index, context filtering, the tool descriptions) is TypeScript, and `weft_render`'s accessibility tree and page come from `@weft/render-react` (TypeScript only). `crates/weft-binding` holds the JSON-text functions both engines call; `crates/weft-web` has the static HTML/CSS generator (`to_html`). cox has no way for a plugin to carry skills (`plugin.toml` has `[[agents]]` and `[[mcp]]`, no skills) and its desktop plugin API is text widgets only (PL§15.4 lists plugin web views as "never", marked open in A134), so T106.5 and T106.7 extend cox.

### T106.7. Design tab: cox side

The cox tasks the T106.6 proposal defines, as built: T33.47.1–T33.47.10 in pyrlyn/cox (`docs/design/plugin-canvas.md`, A156, branch `t33.47-plugin-canvas`); cox's A157 split T33.47.8 into T33.47.8.1–T33.47.8.10, one layer each, and added T33.47.10, a fix. They cover the plugin-API types, manifest key and grant line, the host calls and SDK arms, the `cox-app` state, wire and FFI, the isolated `CanvasPage`, the pane, its shortcut and palette rows, and the user docs. Done when their checks pass in cox and the branch is pushed. The Weft plugin's `cox-plugin-sdk` pin moves to that branch in T106.8.

Status: every cox card is done and pushed (branch head `fd35f0d0`) but T33.47.8's last checks: by hand, ⌘⇧D and the palette row toggle the pane, two frames show, and a click adds a chip that a second click replaces; and the falsifier 3 measurement in Instruments. Both need a person at the app, so they are with the creator.

### T105. Weft to WinUI 3 and back

Convert a Weft screen into a WinUI 3 page for a Windows App SDK app, and read WinUI XAML back into Weft, as T67 does for Slint and T34 for SwiftUI. The design is `docs/winui-design.md`; the external facts, with sources, are `research.md` §23.

**Decisions (creator).** A new target modelled on `crates/weft-slint` and `crates/weft-swiftui`. The output is a XAML `Page` that binds with `{x:Bind}` to a generated C# view model: data paths are its properties, the actions of `on-<event>` are its commands. The importer reads the XAML only; the C# is derived. Checked locally by golden files, XML checks and round trips, and in CI by a Windows job that `dotnet build`s a WinUI project holding every generated corpus screen; screenshots may follow as a separate task. The creator approves the design before any code.

**Execution plan (this round, design only).** Write `docs/winui-design.md` (mapping of every core kind and prop, tokens, layout vocabulary, bindings, events, slots, light and dark, losses, round trip, XML parser, CI job, open questions), `research.md` §23, and the sub-tasks below. The creator approved the design and accepted the recommendation of every open question in `docs/winui-design.md`, including a `roxmltree` row in the workspace `rust.md` when T105.8 adds the crate; T105.1 may start.

**Done when** every sub-task is done: every corpus screen and catalog example generates, its golden files build in the Windows job, `import_winui(generate(x)) == x` for each, `read_winui` reads the generated XAML without its source comment with only the losses SPEC §9 lists, and SPEC §9, §10.6, AGENT-SPEC and the crate README describe the target.

### T105.3. WinUI value and choice controls

`select`, `combobox`, `radio-group`, `segmented-control`, `slider`, `stepper`, `date-picker`, `color-picker`, `link`, `image`, `model`, `alert`, `menu`, with the `Choice` helper and the date and colour converter functions. SPEC §9 rows first. About 480 lines of code. Done when their examples and corpus screens generate, match goldens and read back through the comment.

### T105.4. WinUI repetition and containers

`<each>` (`ItemsRepeater` with typed templates), `list`, `table`, `tabs`, `dialog` (the `Dialog.IsOpen` helper), `section`, `grid` without `min-column-width`, and the slots. SPEC §9 rows first. About 490 lines of code; split at claim time if it grows. Done as T105.3.

### T105.5. WinUI layout vocabulary, tokens and materials

`justify` and `grow` through `Grid` tracks, `align`, `padding`, `max-width`, `min-column-width` (`ColumnsPanel`, `ColumnsLayout`), `WeftTokens.xaml` (beyond the dimensions T105.6 writes) and per-page tokens with `export.winui.sharedTokens` and `--shared-tokens`/`--no-shared-tokens` (moved from T105.6: a key ships with its behaviour), `ThemeDictionaries` for light, dark and high contrast, material as `AcrylicBrush`, the 3D tilt. Checks the unverified resource-key and perspective facts in the Windows build. About 460 lines of code. Done when the `layout`, `glass`, `tilt` and `appearance` corpus screens generate, build and read back.

### T105.7. Windows CI build of the generated pages

A `winui` job on `windows-latest` in `.github/workflows/ci.yml`: checkout and `actions/setup-dotnet` pinned by SHA, .NET SDK 10.0.402, `dotnet build` of a WinUI 3 class library (`Microsoft.WindowsAppSDK` 2.5.1, warnings as errors) that includes every golden page, view model and the shared files. A Rust test that every corpus screen and catalog example has a golden. Not a required check at first. About 120 lines. Done when the job is green on the T105.1–T105.2 goldens and fails on a deliberately broken golden (shown in the PR).

Execution plan:

1. `crates/weft-winui/windows/WeftScreens.csproj`: a `net10.0-windows10.0.19041.0` WinUI class library (`UseWinUI`, `Nullable`, `TreatWarningsAsErrors`, `Microsoft.WindowsAppSDK` 2.5.1) that takes every golden `*Page.xaml`, `*.cs` and `WeftTokens.xaml` from `tests/fixtures/` (not the importer's `handwritten.xaml`).
2. `WeftTokens.xaml` becomes a golden of the default tokens, so the build has the dictionary the pages name.
3. The `winui` job in `ci.yml` (not a required check: only `pipeline / gate` is). `.github/infra.yml` turns off pyrlyn/ci's `dotnet` check, which would otherwise run `dotnet test` of the new project on Linux and, through the required gate, make the build required; moving the build there is the step that makes it required later.
4. Rust test in `tests/winui.rs`: every corpus screen and catalog example has its three goldens except the screens in `PENDING`, an explicit list T105.3–T105.5 shrink (it replaces `COVERED`); no golden belongs to a screen that is gone.
5. Verify: `moon run root:changed`, jactionlint, the job green on the PR, a red run on a deliberately broken golden linked in the PR, then reverted.

### T105.10. WinUI target: SPEC, AGENT-SPEC and docs

SPEC §9 "From WinUI" and the loss table complete, the AGENT-SPEC check, `crates/weft-winui/README.md`, and the corpus-wide round-trip test both ways. About 60 lines of code. Done when the merge gate passes and every SPEC §9 statement about WinUI has a test.
