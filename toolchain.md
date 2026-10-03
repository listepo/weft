# Toolchain

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| node | mise | Runs TypeScript sources and tests directly | https://github.com/nodejs/node |
| pnpm | mise | Workspace package manager | https://github.com/pnpm/pnpm |
| moon | mise | Workspace tasks with caching (`moon ci`) | https://github.com/moonrepo/moon |
| rust | mise | Compiles the core to native code and to `wasm32-unknown-unknown` | https://github.com/rust-lang/rust |
| wasm-pack | mise | Builds the WebAssembly packages | https://github.com/rustwasm/wasm-pack |
| wasm-bindgen | mise | JS glue for the WebAssembly build | https://github.com/wasm-bindgen/wasm-bindgen |
| napi (`@napi-rs/cli`) | mise | Builds the native Node and Bun addon | https://github.com/napi-rs/napi-rs |
| deno | mise | Runtime the bindings are tested on | https://github.com/denoland/deno |
| bun | mise | Runtime the bindings are tested on | https://github.com/oven-sh/bun |

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
| playwright | local | https://github.com/microsoft/playwright | Accessibility snapshot of rendered pages |
| @modelcontextprotocol/sdk | local | https://github.com/modelcontextprotocol/typescript-sdk | MCP server exposing the format to agents |
| oxc-transform | local | https://github.com/oxc-project/oxc | Compiles generated JSX in the equivalence tests |
