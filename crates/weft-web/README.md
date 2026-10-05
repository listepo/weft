# weft-web

Weft and the web, both ways: importers that read HTML and JSX source into a Weft document with a
loss table, and generators that write HTML/CSS, React, SolidJS and Lit. The shared importer parts (loss
table, ids, literals, required-prop stand-ins, limits, the role tree builder) live in
`crates/weft-import`.

## Parsers

| Input | Crate | Why | Source (checked 2026-10-05) |
| --- | --- | --- | --- |
| JSX, TSX | oxc_parser, oxc_ast, oxc_allocator, oxc_span 0.152.0 | The Oxc project's parser: complete JSX and TypeScript syntax, an arena AST that is dropped at once, no C code, so it builds for `wasm32-unknown-unknown`; the fastest maintained Rust parser for JavaScript. MIT, MSRV 1.96. Released 2026-09-28. | https://crates.io/crates/oxc_parser, https://github.com/oxc-project/oxc |
| HTML | html5ever 0.40.1 | The WHATWG tree builder of Servo: a page is read as a browser builds it, including the repairs the standard makes to malformed markup. Builds for `wasm32-unknown-unknown`. Released 2026-09-14. | https://crates.io/crates/html5ever, https://github.com/servo/html5ever |

Rejected for HTML:

- `markup5ever_rcdom` (the reference tree for html5ever) is published only as `0.39.0+unofficial`
  and lags html5ever, so `src/tree.rs` implements the tree sink itself: an index arena, with
  template contents kept as children and no recursion, so deep pages neither overflow the stack
  when parsed nor when dropped (https://crates.io/crates/markup5ever_rcdom).
- `tl` is a fast non-standard parser whose last release is from 2024
  (https://crates.io/crates/tl).

Rejected for JSX:

- `swc_ecma_parser` parses the same syntax but pulls in a larger dependency tree for an AST the
  importer only reads (https://crates.io/crates/swc_ecma_parser).
- `tree-sitter-javascript`/`tree-sitter-typescript` are C, which does not build for
  `wasm32-unknown-unknown` without a C toolchain for that target (the reason `weft-swiftui` keeps
  its tree-sitter importer out of the WebAssembly build).

No CSS parser: the importers read only inline `style` declarations and the generator's own class
names and custom properties, which a declaration split covers; stylesheets are not interpreted.

oxc parses recursively. Native imports run on a 64 MiB thread; in WebAssembly (1 MiB stack)
`src/from_jsx/scan.rs` refuses, before parsing, sources whose nesting would need more than a
conservative stack estimate (`W602`).

The TypeScript importer used htmlparser2, which does not apply the HTML standard's tree
construction. Both agree on well-formed markup; `tests/from_dom.rs` compares the two wherever the
parsed trees agree, which covers every rendered corpus screen, every corpus page and every
hand-written sample.
