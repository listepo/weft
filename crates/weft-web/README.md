# weft-web

Weft and the web, both ways: importers that read HTML and JSX source into a Weft document with a
loss table, and generators that write HTML/CSS, React and SolidJS. The shared importer parts (loss
table, ids, literals, required-prop stand-ins, limits, the role tree builder) live in
`crates/weft-import`.

## Parsers

| Input | Crate | Why | Source (checked 2026-10-05) |
| --- | --- | --- | --- |
| HTML | html5ever 0.40.1 | The WHATWG tree builder of Servo: a page is read as a browser builds it, including the repairs the standard makes to malformed markup. Builds for `wasm32-unknown-unknown`. Released 2026-09-14. | https://crates.io/crates/html5ever, https://github.com/servo/html5ever |

Rejected for HTML:

- `markup5ever_rcdom` (the reference tree for html5ever) is published only as `0.39.0+unofficial`
  and lags html5ever, so `src/tree.rs` implements the tree sink itself: an index arena, with
  template contents kept as children and no recursion, so deep pages neither overflow the stack
  when parsed nor when dropped (https://crates.io/crates/markup5ever_rcdom).
- `tl` is a fast non-standard parser whose last release is from 2024
  (https://crates.io/crates/tl).

The TypeScript importer used htmlparser2, which does not apply the HTML standard's tree
construction. Both agree on well-formed markup; `tests/from_dom.rs` compares the two wherever the
parsed trees agree, which covers every rendered corpus screen, every corpus page and every
hand-written sample.
