# Toolchain

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| node | mise | Runs TypeScript sources and tests directly | https://github.com/nodejs/node |
| pnpm | mise | Workspace package manager | https://github.com/pnpm/pnpm |
| moon | mise | Workspace tasks with caching (`moon ci`) | https://github.com/moonrepo/moon |
| rust | mise | Compiles the core to native code and to `wasm32-unknown-unknown` | https://github.com/rust-lang/rust |
| wasm-pack | mise | Builds `packages/core/wasm/` from `crates/weft-wasm` (`moon run root:wasm`) | https://github.com/rustwasm/wasm-pack |
| wasm-bindgen | mise | JS glue for the WebAssembly build | https://github.com/wasm-bindgen/wasm-bindgen |
| napi (`@napi-rs/cli`) | mise | Builds the native Node and Bun addon | https://github.com/napi-rs/napi-rs |
| deno | mise | Runtime the bindings are tested on (`moon run root:runtimes`) | https://github.com/denoland/deno |
| bun | mise | Runtime the bindings are tested on (`moon run root:runtimes`) | https://github.com/oven-sh/bun |
| claude (Claude Code) | https://code.claude.com/docs/en/setup | Hosts the plugin (`plugins/claude-code`); `claude plugin validate` checks its manifests | https://github.com/anthropics/claude-code |
| Cursor | https://cursor.com/download | Hosts the plugin (`plugins/cursor`); no test needs it | https://cursor.com |
| Open Design | https://open-design.ai | Hosts the plugin (`plugins/open-design`); no test needs it | https://github.com/nexu-io/open-design |
| cox | https://github.com/pyrlyn/cox (`cox plugin install plugins/cox`) | Hosts the plugin (`plugins/cox`); no test needs it, the end-to-end test loads the module with cox's engine | https://github.com/pyrlyn/cox |
| cargo-nextest | mise | Runs the Rust tests | https://github.com/nextest-rs/nextest |
| jactionlint | mise (`github:jdx/jactionlint`) | Lints the workflows (CI, the pre-push hook and `mise x -- jactionlint`) | https://github.com/jdx/jactionlint |
| hk | mise | Runs jactionlint as a pre-push hook (`hk.pkl`) | https://github.com/jdx/hk |
| shellcheck | mise | Checks `run:` scripts inside jactionlint | https://github.com/koalaman/shellcheck |
| cargo-insta | `cargo install cargo-insta` (optional) | Reviews changed snapshots (`cargo insta review`); the tests run without it | https://github.com/mitsuhiko/insta |
| Xcode (`xcrun swiftc`, `xcodebuild`, `swift`) | Mac App Store | Typechecks the Swift that weft-swiftui generates, for iOS 17 and macOS 14, and builds the SwiftPM plugins, the sample projects and the Source Editor Extension (`plugins/xcode`), and WeftStudio for macOS (`studio/macos`, arm64); those tests skip without it | https://developer.apple.com/xcode/ |
| XcodeGen | mise | Generates the `.xcodeproj` of the Xcode sample, of the Source Editor Extension (`plugins/xcode`) and of WeftStudio for macOS (`studio/macos`) from YAML, so no project file is committed | https://github.com/yonaskolb/XcodeGen |
| uv | mise | Installs the Python tools of the `pipx:` backend | https://github.com/astral-sh/uv |
| Translate Toolkit | mise (`pipx:translate-toolkit`) | Localization file converters and checks (`po2json`, `pofilter`, `pocount`) | https://github.com/translate/translate |
| codex (optional) | https://github.com/openai/codex | `plugins/xcode/scripts/xcode-agents.ts` registers the MCP server with it; its test skips without it | https://github.com/openai/codex |
| iOS Simulator (`xcrun simctl`) | Xcode, with the iOS 27.0 runtime and an iPhone 17 simulator | `@weft/visual` builds the generated SwiftUI screens into an app and screenshots them; skipped without it | https://developer.apple.com/documentation/xcode/running-your-app-in-simulator-or-on-a-device |
| Chromium for Playwright | `pnpm exec playwright install chromium` | `@weft/visual` screenshots the web targets in Vitest browser mode (skipped without it); the browser leg of `moon run root:runtimes` fails without it | https://github.com/microsoft/playwright |
| .NET SDK 10.0.402 | `actions/setup-dotnet` in the `winui` and `studio-windows` jobs of `ci.yml` (Windows only for the XAML steps) | Builds the WinUI goldens (`crates/weft-winui/windows/WeftScreens.csproj`) and WeftStudio for Windows (`studio/windows`), and runs its tests | https://github.com/dotnet/sdk |

## mise

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| node | global | https://github.com/nodejs/node | Runtime |
| pnpm | global | https://github.com/pnpm/pnpm | Package manager |
| aqua:moonrepo/moon | global | https://github.com/moonrepo/moon | Task runner |
| rust | global | https://github.com/rust-lang/rust | Compiler with the wasm32 target, rustfmt and clippy |
| deno | global | https://github.com/denoland/deno | Runtime |
| bun | global | https://github.com/oven-sh/bun | Runtime |
| aqua:rustwasm/wasm-pack | global | https://github.com/rustwasm/wasm-pack | WebAssembly packaging |
| github:wasm-bindgen/wasm-bindgen | global | https://github.com/wasm-bindgen/wasm-bindgen | WebAssembly JS glue |
| npm:@napi-rs/cli | global | https://github.com/napi-rs/napi-rs | Native addon builds |
| cargo:cargo-nextest | global | https://github.com/nextest-rs/nextest | Rust test runner |
| aqua:yonaskolb/XcodeGen | global | https://github.com/yonaskolb/XcodeGen | Xcode projects of `plugins/xcode` and `studio/macos` |
| uv | global | https://github.com/astral-sh/uv | Backend for the `pipx:` tools |
| pipx:translate-toolkit | global | https://github.com/translate/translate | Localization converters and checks |
| github:jdx/jactionlint | global | https://github.com/jdx/jactionlint | Workflow lint |
| hk | global | https://github.com/jdx/hk | Pre-push hook runner |
| shellcheck | global | https://github.com/koalaman/shellcheck | Shell checks inside jactionlint |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| serde | local | https://github.com/serde-rs/serde | Catalog and model (de)serialization |
| serde_json | local | https://github.com/serde-rs/json | JSON documents, patches and catalogs; `preserve_order` keeps key order |
| indexmap | local | https://github.com/indexmap-rs/indexmap | Ordered maps for props, slots and tokens |
| semver | local | https://github.com/dtolnay/semver | Cargo's compatibility rule for a catalog's `requires` (weft-catalog); no default features |
| ryu-js | local | https://github.com/boa-dev/ryu-js | Numbers printed as JavaScript prints them, so both cores emit the same bytes |
| thiserror | local | https://github.com/dtolnay/thiserror | Typed errors in weft-core |
| anyhow | local | https://github.com/dtolnay/anyhow | Errors in the `weft` binary |
| clap | local | https://github.com/clap-rs/clap | `weft` command-line parsing |
| proptest | local (dev) | https://github.com/proptest-rs/proptest | Property tests of weft-core, weft-catalog and weft-swiftui: no panics, round trips, idempotent formatting |
| slint-interpreter | local (optional) | https://github.com/slint-ui/slint | The `runtime` feature of weft-slint: compiles the `.slint` files it generates at run time and fills them with sample data, for the editor (T69) and the tests; without default features, so no backend or renderer |
| i-slint-backend-testing | local (dev) | https://github.com/slint-ui/slint | Headless software renderer for the weft-slint screenshot tests; 1.18.1, `renderer-software` only |
| i-slint-core | local (dev) | https://github.com/slint-ui/slint | Sets that testing backend and pins its color scheme to light before a screenshot |
| png | local (dev) | https://github.com/image-rs/image-png | Encodes and decodes the Slint screenshot PNGs; not the full `image` stack |
| i-slint-compiler | local | https://github.com/slint-ui/slint | Syntax tree for the weft-slint importer (`import` feature); 1.18.1, no renderer or backend |
| slint-bindings-ffi | local (git) | https://github.com/pyrlyn/slint-bindings | weft-studio-ffi (T69.1): the host-driven Slint surface (`SbHost`, the `sb_*` C ABI) the editor's shells draw and feed; with it come `slint-bindings-core` and the `slint-embed` and `slint-dart-interpreter` path crates of the same checkout. Not published, so a git dependency pinned by `rev`; its exact `=1.18.1` Slint pins must match this workspace (one `i-slint-core`, checked by `tests/e2e.rs`). On macOS and Windows it also builds Slint's wgpu renderer |
| abi-drift | local (dev) | https://github.com/pyrlyn/crates-packages | weft-studio-ffi `tests/bindings.rs`: the committed C header and C# bindings against what cbindgen and csbindgen generate now (`WEFT_STUDIO_BLESS=1` rewrites them) |
| cbindgen | local (dev) | https://github.com/mozilla/cbindgen | Generates `crates/weft-studio-ffi/include/weft_studio.h` inside that test; the library only, no default features |
| csbindgen | local (dev) | https://github.com/Cysharp/csbindgen | Generates `crates/weft-studio-ffi/bindings/csharp/NativeMethods.g.cs`, the P/Invoke declarations of the Windows shell, inside that test |
| insta | local (dev) | https://github.com/mitsuhiko/insta | Reviewed snapshots of every generator output per corpus screen and catalog example (weft-snapshots); `weft-snapshots::unreferenced` fails on a snapshot no test writes |
| jsonschema | local (dev) | https://github.com/Stranger6667/jsonschema | Checks every exported A2UI message against the vendored A2UI v0.9 JSON Schemas (weft-snapshots); no network or TLS features |
| tree-sitter | local | https://github.com/tree-sitter/tree-sitter | Parses Swift source in the weft-swiftui importer (`import` feature; C, so not in wasm32 builds) |
| tree-sitter-swift | local | https://github.com/alex-pinkus/tree-sitter-swift | The Swift grammar for that parser |
| wasm-bindgen | local | https://github.com/wasm-bindgen/wasm-bindgen | `weft-wasm` exports; pinned exactly to the CLI version in `mise.toml` |
| napi, napi-derive | local | https://github.com/napi-rs/napi-rs | `weft-node` exports for Node and Bun; its macros expand to `allow(unsafe_code)`, so that crate alone relaxes the workspace's `forbid` to `deny` |
| napi-build | local (build) | https://github.com/napi-rs/napi-rs | Link flags of the addon (`weft-node/build.rs`) |
| unicode-normalization | local | https://github.com/unicode-rs/unicode-normalization | NFKD before folding imported names into id slugs, as `String.prototype.normalize` does in the TypeScript importers |
| unicode-properties | local | https://github.com/unicode-rs/unicode-properties | Unicode letter and number classes (`\p{L}`, `\p{N}`) for the JSX generators' check of which text runs print as written |
| html5ever | local | https://github.com/servo/html5ever | Parses HTML in `weft-web` with the WHATWG tree builder, so imported pages read as a browser builds them |
| oxc_parser, oxc_ast, oxc_allocator, oxc_span | local | https://github.com/oxc-project/oxc | Parse JSX and TSX in `weft-web`'s React and SolidJS importers; pure Rust, builds for wasm32 |
| cox-plugin-sdk | local (`plugins/cox`, git) | https://github.com/pyrlyn/cox | The cox plugin's exports and ABI types (with `cox-plugin-api` and `extism-pdk`); not on crates.io, so a git dependency pinned to a commit. The first fetch clones pyrlyn/cox: about 560 MB in `~/.cargo/git/db` and 290 MB checked out |
| roxmltree | local | https://github.com/RazrFalcon/roxmltree | The XML tree of weft-winui: its tests check the generated XAML is well-formed, `import_winui` compares a page with the one its source comment regenerates, and `read_winui` reads hand-written XAML; namespaces resolved, DTDs refused, nodes bounded |
| toml | local (dev, `plugins/cox`) | https://github.com/toml-rs/toml | Reads `plugin.toml` into cox's manifest type in the plugin's manifest test |
| wasm-plugin-host | local (dev, `plugins/cox`, git) | https://github.com/pyrlyn/crates | The extism engine of cox's plugin host (`packages/crates`), which the plugin's end-to-end test loads the module with; not on crates.io yet, so pinned to a commit |

## npm

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| typescript | local | https://github.com/microsoft/typescript-go | Type checking |
| oxlint | local | https://github.com/oxc-project/oxc | Lint |
| oxfmt | local | https://github.com/oxc-project/oxc | Formatting |
| @types/node | local | https://github.com/DefinitelyTyped/DefinitelyTyped | Node type definitions |
| zod | local | https://github.com/colinhacks/zod | Model schemas and JSON Schema export |
| htmlparser2 | local | https://github.com/fb55/htmlparser2 | Parses HTML baselines in the benchmark checkers |
| yaml | local | https://github.com/eemeli/yaml | Reads the YAML frontmatter of a DESIGN.md in `@weft/design-md`; aliases are refused (`maxAliasCount: 0`) |
| fast-check | local | https://github.com/dubzzz/fast-check | Property-based round-trip tests |
| gpt-tokenizer | local | https://github.com/niieani/gpt-tokenizer | Offline token counts for the benchmark |
| oxc-parser | local | https://github.com/oxc-project/oxc | Parses JSX baselines in the benchmark checkers |
| react | local | https://github.com/facebook/react | Reference renderer target |
| react-dom | local | https://github.com/facebook/react | Server rendering for tests and static pages |
| @types/react | local | https://github.com/DefinitelyTyped/DefinitelyTyped | React type definitions |
| @types/react-dom | local | https://github.com/DefinitelyTyped/DefinitelyTyped | React DOM type definitions |
| vitest | local | https://github.com/vitest-dev/vitest | Test runner for every TypeScript suite (`vitest run`) |
| vite | local | https://github.com/vitejs/vite | Required peer of Vitest; transforms the TypeScript sources and tests; its `build` API (rolldown) bundles the scripts and MCP server into `plugins/claude-code/dist`, `plugins/cursor/dist` and `plugins/open-design/dist`, and, through `@weft/design-plugin`, the Figma and Penpot plugins into `plugins/figma/dist` and `plugins/penpot/dist` |
| playwright | local | https://github.com/microsoft/playwright | Accessibility snapshot of rendered pages; drives Chromium for the screenshots in `@weft/visual` and the browser leg of the runtime matrix |
| @vitest/browser-playwright | local | https://github.com/vitest-dev/vitest | Vitest browser mode on Playwright, for the web screenshots in `@weft/visual` and the browser leg of the runtime matrix (`runtimes/vitest.config.ts`) |
| pixelmatch | local | https://github.com/mapbox/pixelmatch | Pixel comparison of screenshots, with a diff image, in `@weft/visual` |
| fast-png | local | https://github.com/image-js/fast-png | Decodes and encodes the PNG screenshots and diff images in `@weft/visual`; pure JavaScript |
| @modelcontextprotocol/sdk | local | https://github.com/modelcontextprotocol/typescript-sdk | MCP server exposing the format to agents |
| oxc-transform | local | https://github.com/oxc-project/oxc | Compiles generated JSX in the equivalence tests and for the browser in `@weft/visual` |
| solid-js | local | https://github.com/solidjs/solid | Server-renders generated SolidJS components in the equivalence tests; renders them in the browser in `@weft/visual` |
| lit | local | https://github.com/lit/lit | Runs generated Lit elements in the browser in `@weft/visual`, where their accessibility tree is compared with React's |
| babel-preset-solid | local | https://github.com/solidjs/solid/tree/main/packages/babel-preset-solid | Compiles generated SolidJS JSX for server rendering in the equivalence tests and for the DOM in `@weft/visual` |
| @babel/core | local | https://github.com/babel/babel | Runs babel-preset-solid; 7.x because the preset requires Babel 7 |
| @types/babel__core | local | https://github.com/DefinitelyTyped/DefinitelyTyped | Types for @babel/core in the equivalence tests |
| p-limit | local | https://github.com/sindresorhus/p-limit | Concurrency limit for benchmark requests |
| undici | local | https://github.com/nodejs/undici | Fetch without the header timeout, for slow local model servers |
| ajv | local | https://github.com/ajv-validator/ajv | Validates the Cursor plugin manifests against Cursor's own JSON schemas |
| ajv-formats | local | https://github.com/ajv-validator/ajv-formats | The `uri` and `email` formats those schemas use |
| @figma/plugin-typings | local | https://github.com/figma/plugin-typings | Official Figma Plugin API types; the conversion is typed against them |
| @penpot/plugin-types | local | https://github.com/penpot/penpot | Official Penpot plugin API types (MPL-2.0); `@weft/penpot` and the Penpot plugin are typed against them. Pinned to 1.5.0 from the `next` tag, approved by the creator: `latest` (1.4.2) has no design tokens API |
| ses | local | https://github.com/endojs/endo | The SES library Penpot runs plugins in; the Penpot plugin's bundle test evaluates the plugin in an SES compartment |

## NuGet

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| Microsoft.WindowsAppSDK | local (`crates/weft-winui/windows/WeftScreens.csproj`, `studio/windows/WeftStudio.WinUI`) | https://github.com/microsoft/WindowsAppSDK | WinUI 3 and its XAML compiler for the Windows build of the goldens; WeftStudio's window, menus and file pickers (`Microsoft.Windows.Storage.Pickers`), at 2.5.1 like slint-bindings' control |
| MSTest.Sdk | local (`studio/windows/WeftStudio.Tests`) | https://github.com/microsoft/testfx | WeftStudio's document model and smoke tests, on Microsoft.Testing.Platform (`studio/windows/global.json`); 4.5.1 as slint-bindings' host tests |
| FlaUI.UIA3 | local (`studio/windows/WeftStudio.Tests`) | https://github.com/FlaUI/FlaUI | Drives the built WeftStudio through UI Automation in the smoke test: window title, capture, the Save As accelerator |

## Rust tests

`cargo nextest` runs the Rust tests (`root:rust-test`). `.config/nextest.toml` gives the Swift typecheck tests of `weft-swiftui` (`generated_swift_typechecks_*`) a timeout of 15 minutes (`slow-timeout`, terminate after 15 periods of 60 s), so a hang fails as a timed-out test instead of being killed with the gate. A cold typecheck of every generated screen takes about five minutes per SDK and more under load.

Those tests also cache their work under `~/Library/Caches/weft-swiftui/` (falling back to `CARGO_TARGET_TMPDIR/weft-swiftui-cache/` when home is not writable), as `@weft/visual` caches its simulator lock:

- `modules/` is the `swiftc -module-cache-path`, filled by the compiler from the SDK modules.
- `typecheck-<sdk>.stamp` holds the key of the last successful typecheck: the `swiftc` version, the target and every source it saw. A warm run of the same toolchain and sources skips `swiftc` and finishes in about a second. Delete the stamp (or the folder) to typecheck again after a cache is suspect; a new toolchain or any source change writes a new key and typechecks once.

## Linkers

- **Windows (MSVC):** `rust-lld` through `.cargo/config.toml` (`[target.*-pc-windows-msvc] linker`). It ships with rustup, nothing to install.
- **Linux:** `mold`, pinned in `mise.toml` for Linux only and driven by `clang`. Opt-in, off by default: `cargo --config .cargo/mold.toml <cmd>` (or `just mold <cmd>` where the repo has a justfile). CI and the default build keep the stock linker.
- **macOS:** Apple's native linker (mold has no macOS support).
- **Cranelift:** not used. It needs a nightly toolchain and this repository is on stable.
