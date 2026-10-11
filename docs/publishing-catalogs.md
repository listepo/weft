# Publishing a catalog

A catalog that more than one project uses is a library: it names a `prefix`, and every component it adds starts with that prefix. This page shows how to make one, publish it as an npm package and use it from a project. [SPEC §5](../SPEC.md#5-catalog) defines the catalog file, [§10.2](../SPEC.md#102-the-project-file) the package entry and [§10.4](../SPEC.md#104-catalog-extension) how catalogs merge.

## Make the catalog

A library catalog is an ordinary catalog file with two more members:

```json
{
  "weft": "0.3",
  "name": "@acme/ui",
  "version": "1.0.0",
  "prefix": "acme",
  "requires": { "weft-core": "0.2.0" },
  "components": {
    "acme-badge": {
      "description": "A short status label.",
      "role": "status",
      "content": "text"
    }
  }
}
```

- `prefix` is one lowercase name segment. Every component the catalog adds is named `<prefix>-…`: `acme-badge`, `acme-button`. A library may not add other kinds or change core ones (`W713`); a project's own catalog does that.
- `requires` names the catalogs it was written for, by version, with Cargo's rule: `"0.2.0"` accepts any `0.2.x`, `"1.2.0"` any `1.x` from `1.2.0`. A project that loads the library without them gets the warning `W714`.
- `name` should be the package name and `version` the package version, so that a project sees one name for both. Nothing checks this.
- Version the catalog by [SPEC §8](../SPEC.md#8-versioning-and-extensibility): adding is a minor change, removing or tightening is a major one. `diffCatalogs` of `@weft/catalog` classifies a change.

For a web component library, `weft import-cem` writes the catalog from its Custom Elements Manifest ([Importing](importing.md)):

```console
$ weft import-cem custom-elements.json --prefix acme --name @acme/ui --version 1.0.0 > weft/acme-ui.catalog.json
```

With `--prefix` it writes `prefix` and `requires`, and reports a custom element whose tag does not start with `acme-` as a loss.

## Choose the prefix

The prefix must not be `x`, `weft`, a core component or a core component's first segment (`date` and `color` are taken, by `date-picker` and `color-picker`); anything else of that shape is `W712`. Two libraries with one prefix cannot load in one project (`W711`).

Nothing assigns prefixes. [The catalog list](catalogs.md) records the ones in use; pick one that is not there, and add yours.

## Publish the package

Point at the catalog from `package.json`, under `weft.catalog`, and ship the file:

```json
{
  "name": "@acme/ui",
  "version": "1.0.0",
  "keywords": ["weft-catalog"],
  "files": ["dist", "weft"],
  "weft": { "catalog": "weft/acme-ui.catalog.json" }
}
```

- `weft.catalog` is a path relative to the package directory, with `/` separators. It may not be absolute, contain `..`, `\` or `:`, or resolve (following symbolic links) outside the package directory.
- `files` must include the catalog, or `npm publish` leaves it out.
- The keyword `weft-catalog` makes the package findable: `npm search keywords:weft-catalog`.
- `exports` does not matter. The tools read the two files from disk, not through module resolution.

## Use it in a project

Install the package as usual, then list it in `weft.json`, before the project's own catalog:

```json
{
  "catalog": [{ "package": "@acme/ui" }, "catalog.json"]
}
```

The `weft` command and the Node tools (the MCP server started with `--project`, the generators) find the package as Node does: `node_modules/@acme/ui` in the project directory, then in each parent directory, so a workspace that installs at its root works. They read only `package.json` and the file `weft.catalog` names. They install, fetch and run nothing; a package that is not installed is `W704`. `weft_capabilities` shows the catalog with `"source": "@acme/ui@1.0.0"`.

A project can also commit the catalog file instead (`"catalog": ["catalogs/acme-ui.catalog.json", "catalog.json"]`), which is what `examples/project` does. That is the simplest choice for a catalog it generates itself.

The MCP server's `project` argument holds the catalogs themselves, not file names or packages: the host reads them and passes their content.
