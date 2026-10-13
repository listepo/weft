# Ideas

- Renderers for platforms other than the web.
- Publish the repository.
- Check the console blocks of the `docs/` guide in CI, so a page cannot drift from the commands it shows.
- SwiftUI context for screens that place fragments: the importer's source check fails on them, because the reader invents ids for the expanded instances, so their context does not read back. The generator could write instance ids the reader keeps.
- Replace the Rust track of `tooling/select.ts` (nearest-crate match, reverse-dependency closure, nextest filterset, clippy `-p`, `cargo fmt --check`; about 75 lines) with the org's `scoped-check` (`packages/crates/scoped-check`, commit `f51a788`), as nextest, clippy and fmt gates in a `scoped-check.toml`. Open points, from T92: Weft's two Rust rules that `scoped-check` lacks (a data file that a Rust test reads selects all Rust tests; a file in `crates/` that belongs to no crate forces the full run) need extra gates or a wrapper; the moon and Vitest selection stays in `select.ts`, so there would be two plans to merge; `scoped-check` is a Rust binary to install in mise and CI.
