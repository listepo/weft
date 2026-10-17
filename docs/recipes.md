# Recipes

Short, copy-paste answers to "how do I…". They use the login screen from the
[tour](tour.md) (`corpus/login/screen.weft`) and the `weft` binary built by `cargo build -p weft-cli`.
Every command was run against weft 0.1.0. The full flag list of each subcommand is in
[cli.md](cli.md).

```bash
export PATH="$PWD/target/debug:$PATH"
mkdir -p /tmp/wt && cp corpus/login/screen.weft /tmp/wt/login.weft && cd /tmp/wt
```

## Check a file

```bash
weft validate login.weft; echo "exit $?"
```

```
weft: no --catalog and no project; only the syntax and shape layers were checked
exit 0
```

Outside a project, `validate` checks syntax and shape only. Pass `--catalog catalog.json` (or run
inside a project with a `weft.json`) to check against a catalog; add `--strict` to fail on warnings.

## Keep files canonical in CI

`fmt` prints the canonical markup. Diffing it against the file fails when the file drifted:

```bash
weft fmt login.weft | diff -q - login.weft && echo canonical
weft fmt login.weft --write        # or rewrite it in place; untouched when already canonical
```

## Write a static page, then read it back

`--source` keeps the canonical screen in a leading comment, so the importer gives it back exactly:

```bash
weft html login.weft --source --out-dir out      # writes out/login.html
weft import-html out/login.html                  # prints the markup to stdout
```

Without `--out-dir`, `html` prints the page to stdout. What a target cannot hold is listed on stderr as
losses; the import never fails silently.

## Send a screen over A2UI

```bash
weft a2ui login.weft --out-dir out               # writes out/login.a2ui.json
weft import-a2ui out/login.a2ui.json             # back to markup
```

A2UI has no form for design tokens or for the `state` prop, so the first command reports them:

```
login.weft:/screen#login/form#form/stack#fields loss tokens: gap: design tokens have no A2UI form
login.weft:/screen#login/form#form loss props: state: this prop has no A2UI property
```

Going through A2UI and back keeps the fields and bindings; what was reported is lost.

## Give a model a schema

```bash
weft schema > document.schema.json               # JSON Schema 2020-12 for the core catalog
weft schema --catalog catalog.json               # for your own catalog
```

The schema describes the canonical JSON form that the catalog admits, for a model whose decoder
takes a schema.

## How far must a version go up?

`version-check` compares two versions of a screen, fragment or library catalog and says how far the
version must be raised:

```bash
weft version-check old/login.weft login.weft
```

```
patch	body	The canonical JSON changed.
least	0.0.1
login.weft:/screen#login/@version W810 The document declares no version; the changes require 0.0.1. — write version="0.0.1"
```

The command exited 1 here because the new file declares no version. Run it in CI and check its exit code to catch a change that forgot to raise the version.
