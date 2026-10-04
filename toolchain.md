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
| cargo-nextest | mise | Runs the Rust tests | https://github.com/nextest-rs/nextest |

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
| wasm-bindgen | local | https://github.com/wasm-bindgen/wasm-bindgen | `weft-wasm` exports; pinned exactly to the CLI version in `mise.toml` |

## npm

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| typescript | local | https://github.com/microsoft/typescript-go | Type checking |
| oxlint | local | https://github.com/oxc-project/oxc | Lint |
| oxfmt | local | https://github.com/oxc-project/oxc | Formatting |
| @types/node | local | https://github.com/DefinitelyTyped/DefinitelyTyped | Node type definitions |
| zod | local | https://github.com/colinhacks/zod | Model schemas and JSON Schema export |
| htmlparser2 | local | https://github.com/fb55/htmlparser2 | Parses HTML baselines in the benchmark checkers |
| fast-check | local | https://github.com/dubzzz/fast-check | Property-based round-trip tests |
| gpt-tokenizer | local | https://github.com/niieani/gpt-tokenizer | Offline token counts for the benchmark |
| oxc-parser | local | https://github.com/oxc-project/oxc | Parses JSX baselines in the benchmark checkers |
| react | local | https://github.com/facebook/react | Reference renderer target |
| react-dom | local | https://github.com/facebook/react | Server rendering for tests and static pages |
| @types/react | local | https://github.com/DefinitelyTyped/DefinitelyTyped | React type definitions |
| @types/react-dom | local | https://github.com/DefinitelyTyped/DefinitelyTyped | React DOM type definitions |
| vitest | local | https://github.com/vitest-dev/vitest | Test runner for every TypeScript suite (`vitest run`) |
| vite | local | https://github.com/vitejs/vite | Required peer of Vitest; transforms the TypeScript sources and tests |
| playwright | local | https://github.com/microsoft/playwright | Accessibility snapshot of rendered pages |
| @modelcontextprotocol/sdk | local | https://github.com/modelcontextprotocol/typescript-sdk | MCP server exposing the format to agents |
| oxc-transform | local | https://github.com/oxc-project/oxc | Compiles generated JSX in the equivalence tests |
| p-limit | local | https://github.com/sindresorhus/p-limit | Concurrency limit for benchmark requests |
| undici | local | https://github.com/nodejs/undici | Fetch without the header timeout, for slow local model servers |
