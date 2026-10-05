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
| deno | mise | Runtime the bindings are tested on | https://github.com/denoland/deno |
| bun | mise | Runtime the bindings are tested on | https://github.com/oven-sh/bun |
| claude (Claude Code) | https://code.claude.com/docs/en/setup | Hosts the plugin (`plugins/claude-code`); `claude plugin validate` checks its manifests | https://github.com/anthropics/claude-code |
| Cursor | https://cursor.com/download | Hosts the plugin (`plugins/cursor`); no test needs it | https://cursor.com |
| Open Design | https://open-design.ai | Hosts the plugin (`plugins/open-design`); no test needs it | https://github.com/nexu-io/open-design |
| cargo-nextest | mise | Runs the Rust tests | https://github.com/nextest-rs/nextest |
| cargo-insta | `cargo install cargo-insta` (optional) | Reviews changed snapshots (`cargo insta review`); the tests run without it | https://github.com/mitsuhiko/insta |
| Xcode (`xcrun swiftc`) | Mac App Store | Typechecks the Swift that weft-swiftui generates, for iOS 17 and macOS 14; those tests skip without it | https://developer.apple.com/xcode/ |
| iOS Simulator (`xcrun simctl`) | Xcode, with the iOS 27.0 runtime and an iPhone 17 simulator | `@weft/visual` builds the generated SwiftUI screens into an app and screenshots them; skipped without it | https://developer.apple.com/documentation/xcode/running-your-app-in-simulator-or-on-a-device |
| Chromium for Playwright | `pnpm exec playwright install chromium` | `@weft/visual` screenshots the web targets in Vitest browser mode; skipped without it | https://github.com/microsoft/playwright |

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

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| serde | local | https://github.com/serde-rs/serde | Catalog and model (de)serialization |
| serde_json | local | https://github.com/serde-rs/json | JSON documents, patches and catalogs; `preserve_order` keeps key order |
| indexmap | local | https://github.com/indexmap-rs/indexmap | Ordered maps for props, slots and tokens |
| ryu-js | local | https://github.com/boa-dev/ryu-js | Numbers printed as JavaScript prints them, so both cores emit the same bytes |
| thiserror | local | https://github.com/dtolnay/thiserror | Typed errors in weft-core |
| anyhow | local | https://github.com/dtolnay/anyhow | Errors in the `weft` binary |
| clap | local | https://github.com/clap-rs/clap | `weft` command-line parsing |
| proptest | local (dev) | https://github.com/proptest-rs/proptest | Property tests of weft-core, weft-catalog and weft-swiftui: no panics, round trips, idempotent formatting |
| insta | local (dev) | https://github.com/mitsuhiko/insta | Reviewed snapshots of every generator output per corpus screen and catalog example (weft-snapshots) |
| tree-sitter | local | https://github.com/tree-sitter/tree-sitter | Parses Swift source in the weft-swiftui importer (`import` feature; C, so not in wasm32 builds) |
| tree-sitter-swift | local | https://github.com/alex-pinkus/tree-sitter-swift | The Swift grammar for that parser |
| wasm-bindgen | local | https://github.com/wasm-bindgen/wasm-bindgen | `weft-wasm` exports; pinned exactly to the CLI version in `mise.toml` |
| napi, napi-derive | local | https://github.com/napi-rs/napi-rs | `weft-node` exports for Node and Bun; its macros expand to `allow(unsafe_code)`, so that crate alone relaxes the workspace's `forbid` to `deny` |
| napi-build | local (build) | https://github.com/napi-rs/napi-rs | Link flags of the addon (`weft-node/build.rs`) |
| unicode-normalization | local | https://github.com/unicode-rs/unicode-normalization | NFKD before folding imported names into id slugs, as `String.prototype.normalize` does in the TypeScript importers |
| unicode-properties | local | https://github.com/unicode-rs/unicode-properties | Unicode letter and number classes (`\p{L}`, `\p{N}`) for the JSX generators' check of which text runs print as written |
| html5ever | local | https://github.com/servo/html5ever | Parses HTML in `weft-web` with the WHATWG tree builder, so imported pages read as a browser builds them |
| oxc_parser, oxc_ast, oxc_allocator, oxc_span | local | https://github.com/oxc-project/oxc | Parse JSX and TSX in `weft-web`'s React and SolidJS importers; pure Rust, builds for wasm32 |

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
| playwright | local | https://github.com/microsoft/playwright | Accessibility snapshot of rendered pages; drives Chromium for the screenshots in `@weft/visual` |
| @vitest/browser-playwright | local | https://github.com/vitest-dev/vitest | Vitest browser mode on Playwright, for the web screenshots in `@weft/visual` |
| pixelmatch | local | https://github.com/mapbox/pixelmatch | Pixel comparison of screenshots, with a diff image, in `@weft/visual` |
| fast-png | local | https://github.com/image-js/fast-png | Decodes and encodes the PNG screenshots and diff images in `@weft/visual`; pure JavaScript |
| @modelcontextprotocol/sdk | local | https://github.com/modelcontextprotocol/typescript-sdk | MCP server exposing the format to agents |
| oxc-transform | local | https://github.com/oxc-project/oxc | Compiles generated JSX in the equivalence tests and for the browser in `@weft/visual` |
| solid-js | local | https://github.com/solidjs/solid | Server-renders generated SolidJS components in the equivalence tests; renders them in the browser in `@weft/visual` |
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
