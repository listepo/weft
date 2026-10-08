# Design: extension catalogs

Status: **proposal, not approved.** Nothing here is implemented. This document is for the creator to approve, change or reject before `SPEC.md`, `AGENT-SPEC.md`, the loader and the tools change. It is T17.0, the design stage of T17. The prior art it relies on, with sources, is `research.md` §22.

## Problem

A project extends the core catalog with exactly one file today (SPEC §10.2, §10.4). Real hosts combine several sources:

- the core catalog, `weft-core`;
- one or more component libraries, for example one imported from a Custom Elements Manifest with `weft import-cem`;
- the project's own kinds, props and variants.

None of this works now:

- `catalog` in `weft.json` is one file name. A library catalog and a project catalog cannot both be loaded. Merging them by hand into one file means a library update overwrites the project's own entries.
- `Project.catalog` is one merged `Catalog`. It does not record which catalog each kind came from. `weft_capabilities` can name only the core and one extension, and `weft_catalog` shows no source.
- Nothing stops two catalogs from defining the same kind. With one extension there is no second catalog to collide with, so the rule was never needed.
- A catalog has no way to travel between projects. There is no publication format, no lookup convention and no registry. `research.md` §9 lists the registry as open, and `docs/fragments-design.md` defers sharing across projects to this task.

The design must keep the format's rules:

- screens name neither their project nor their resources (SPEC §10);
- documents and validators never touch the network (`AGENTS.md`);
- one way to say a thing (SPEC §1, rule 2);
- `x-<vendor>-` names stay opaque to every catalog (SPEC §8);
- models' first-try validity, which the benchmark measures, must not suffer.

## Proposal

In short:

1. **Namespaces are hyphen prefixes.** A shared catalog declares a `prefix`, for example `acme`, and owns every kind named `acme-…`. The kind grammar of SPEC §2 does not change, so no document, parser or generator changes.
2. **Two roles.** A catalog with a `prefix` is a *library*: it defines kinds under its prefix and extends nothing. A catalog without a `prefix` is the *project catalog*: there is at most one, and it alone may extend kinds it did not define (core or library), by the widen-only rules that exist today.
3. **`catalog` takes an ordered array.** Each entry is a file name, as today, or a package reference `{ "package": "@acme/ui" }`. A string is still accepted and means an array of one.
4. **Catalogs declare what they need.** An optional `requires` names the catalogs and versions a catalog was written against, with Cargo's compatibility rule.
5. **No network.** Catalogs come from committed files or from installed packages, found by a fixed convention. Nothing is fetched while validating or rendering.
6. **No registry service.** A curated list in the docs, kept by the creator through pull requests, records known catalogs and their prefixes. An npm keyword makes them searchable.
7. **Four new codes**, `W711`–`W714`, for clashes, bad prefixes, kinds outside their owner, and unmet requirements.

### Example: `examples/project` with a library and a hand-written extension

The project today loads one extension, `shop` (`catalog.json`): a new kind `rating` and a wider `button.variant` (adds `ghost`). The `acme-ui` library comes from the CEM fixture `crates/weft-import/tests/fixtures/cem/acme-ui.json`, imported with a prefix:

```sh
weft import-cem crates/weft-import/tests/fixtures/cem/acme-ui.json \
  --name acme-ui --version 1.0.0 --prefix acme --out-dir examples/project/catalogs
```

The importer writes `catalogs/acme-ui.catalog.json`. It keeps the tags `acme-badge`, `acme-button`, `acme-card` and `acme-rating`. It drops, with losses as today, `date-picker` (already a core kind), `acme_legacy` (not a Weft name) and `x-acme-hidden` (opaque `x-` name). New with `--prefix`: a tag outside `acme-` would be a `kinds` loss too.

```json
{
  "weft": "0.1",
  "name": "acme-ui",
  "version": "1.0.0",
  "prefix": "acme",
  "requires": { "weft-core": "0.1.0" },
  "components": {
    "acme-button": {
      "description": "The <acme-button> custom element.",
      "role": "generic",
      "content": "mixed",
      "props": {
        "variant": { "description": "…", "type": "enum", "values": ["default", "primary", "danger"], "default": "default" },
        "size": { "description": "…", "type": "enum", "values": ["small", "medium", "large"], "default": "medium" },
        "disabled": { "description": "…", "type": "boolean", "default": false },
        "href": { "description": "…", "type": "string" }
      },
      "slots": { "prefix": { "description": "…" }, "suffix": { "description": "…" } },
      "events": ["acme-click", "acme-focus"]
    },
    "acme-badge": { "…": "…" },
    "acme-card": { "…": "…" },
    "acme-rating": { "…": "…" }
  }
}
```

The hand-written project catalog stays `catalog.json`. It gains `requires` and one entry that widens a library kind:

```json
{
  "weft": "0.1",
  "name": "shop",
  "version": "1.1.0",
  "requires": { "weft-core": "0.1.0", "acme-ui": "1.0.0" },
  "components": {
    "rating": { "…": "unchanged" },
    "button": { "…": "unchanged: variant gains ghost" },
    "acme-button": {
      "props": {
        "variant": { "description": "…", "type": "enum", "values": ["default", "primary", "danger", "ghost"], "default": "default" }
      }
    }
  }
}
```

The project file lists both, libraries first by convention:

```json
{
  "$schema": "../../schemas/weft.schema.json",
  "tokens": "tokens/theme.resolver.json",
  "catalog": ["catalogs/acme-ui.catalog.json", "catalog.json"],
  "actions": ["cart.checkout", "cart.remove", "nav.back"],
  "data": "data.schema.json"
}
```

A screen uses kinds of all three catalogs and names none of them:

```xml
<screen id="review" label="Review" weft="0.1">
  <heading id="title" level="1">Review your order</heading>
  <each id="lines" as="line" in="{$.cart.items}">
    <acme-card id="line" elevated="true">
      <text id="line-name" text="{$line.name}"/>
      <rating id="line-score" label="Customer score" value="{$line.score}" color="{token.color.star}"/>
    </acme-card>
  </each>
  <acme-button id="checkout" variant="ghost" on-acme-click="cart.checkout">Place order</acme-button>
</screen>
```

`heading` and `text` come from `weft-core`, `acme-card` from `acme-ui`, `rating` from `shop`. `acme-button variant="ghost"` is valid because `shop` widened the library's prop.

`weft_capabilities` then reports:

```json
{
  "weft": "0.1",
  "catalogs": [
    { "name": "weft-core", "version": "0.1.0" },
    { "name": "acme-ui", "version": "1.0.0", "prefix": "acme" },
    { "name": "shop", "version": "1.1.0" }
  ]
}
```

What goes wrong, and how it is reported:

| Mistake | Diagnostic |
| --- | --- |
| A second library also declares `"prefix": "acme"` | `W711` at `#/catalog/1/prefix`: the prefix `acme` is already owned by `acme-ui` (`#/catalog/0`); the second catalog is ignored. |
| `shop` defines a new kind `acme-chip` | `W713` at `#/catalog/1/components/acme-chip`: kinds named `acme-…` belong to `acme-ui`; the entry is ignored. |
| `acme-ui` contains an entry for `button` | `W713` at `#/catalog/0/components/button`: a library extends no kind it did not define; the entry is ignored. |
| `shop` narrows `acme-button.variant` | `W707`, as today, but compared with `acme-ui`'s definition. |
| `acme-ui` is removed from the list | `W714` (warning): `shop` requires `acme-ui`. Then `W706` on `#/catalog/0/components/acme-button`, because an entry for a kind no catalog defines must be a whole definition. |
| A screen uses `<acme-chip>` | `W401` (unknown element), as for any kind: a warning for readers, an error in strict mode. |

## Namespaces

Three options were weighed. The kind grammar of SPEC §2 is `[a-z][a-z0-9]*(-[a-z0-9]+)*`.

**A. Hyphen prefix** (`acme-button`). The kind is an ordinary name. The prefix is a claim of ownership checked when catalogs load, not a scope in the document. This is how custom elements already avoid clashes on the web (a valid custom element name contains a hyphen; libraries use their own first segment), so a CEM library is prefixed before Weft does anything.

**B. Colon** (`acme:button`). The prefix is a scope in the name.

**C. Collision rule only.** No namespaces; a kind defined by two catalogs is an error, and the project fixes it by dropping one.

Cost of each, by part of the system:

| Part | A. Hyphen prefix | B. Colon | C. Collision rule only |
| --- | --- | --- | --- |
| Grammar (SPEC §2) | None | New name form; `kind` and the diagnostic path segment `kind#id` (§6.1) change | None |
| Parsers (Rust core; TypeScript runs on it through WebAssembly) | None | Name scanner, both differential fixtures | None |
| XML tools | None | XML reserves the colon for namespaces; an undeclared prefix is not namespace-well-formed, so namespace-aware readers (browsers' XML parser, lxml, XSLT) reject the screen unless it declares `xmlns:acme="…"`. Declaring it makes the screen name its catalogs, which SPEC §10 forbids | None |
| Format version | None; `weft` stays 0.1 | Minor bump (0.2); a 0.1 reader rejects the name as malformed (`W1xx`), not as an unknown kind | None |
| Canonical JSON and the T12 schema | None | `kind` values with `:`; the schema's `$defs` keys and JSON Pointers still work | None |
| Generators | None: SwiftUI already names a non-core kind's view after it (`acme-button` → `AcmeButtonView`); web targets render it under its role | Every name mapping (Swift and Slint type names, JSX component names, CSS classes, file names) needs a rule for `:` | None |
| Importers | None: a DOM or CEM tag `acme-button` is the kind `acme-button` | A custom element tag and its kind differ (`acme-button` ↔ `acme:button`); each importer translates | None |
| Design tools | None: component sets are named after the kind | Layer and component names with `:` (allowed, but a second name form to read back) | None |
| Models | Nothing new to learn; one more catalog kind | A new form that the benchmark has never measured; models that know XML tend to add `xmlns` declarations | Nothing new |
| Collision safety | Prevented when catalogs load: a prefix has one owner | Prevented by syntax | Found late: two independent libraries both shipping `card` or `rating` cannot be loaded together at all |

**Recommendation: A.** It changes nothing in documents, parsers or generators, matches the web's own convention, and the prefix makes ownership checkable. C is kept as the backstop (`W711`) for the cases a prefix cannot prevent. B pays for a scope the hyphen already gives, and it breaks namespace-aware XML tools or the rule that screens are silent about their catalogs.

What A does not give: a kind's name does not say, syntactically, which part is the prefix. `date-picker` is a core kind, not the kind `picker` of a catalog `date`. The rule below (a prefix may not be the first segment of a core kind) removes that ambiguity for the core. And A cannot rename a library's kinds when two libraries chose the same prefix; see **Prefix ownership**.

## Prefix ownership

**Who declares the prefix.**

- *Author-declared* (recommended): the catalog file says `"prefix": "acme"`; the loader checks that every new kind it defines starts with `acme-`. The prefix travels with the catalog, so every project that loads `acme-ui` writes the same kind names, and screens, generated code, Figma components and the custom element tags agree.
- *Project alias*: the project file renames a catalog's prefix at load time (`{ "package": "@acme/ui", "prefix": "a" }`), as ESLint's flat config and shadcn's `components.json` let the consumer pick the namespace, and Slint's `import { Button as CoolButton }`. It fixes a clash between two libraries that chose the same prefix. The cost is that the same library has different kind names in different projects: generated SwiftUI expects `AView` in one app and `AcmeButtonView` in another, a CEM-imported kind no longer equals its custom element tag, and screens cannot move between projects. Every generator and importer would need the renaming table.

**Recommendation:** author-declared only, in T17.1. A clash between two libraries' prefixes is `W711`, and the project keeps one of them. If real clashes appear, an alias can be added later as a project-file option without changing catalogs (ESLint did the reverse: user-chosen namespaces first, then `meta.namespace` so a plugin can declare one).

**Prefix grammar.** One name segment, `[a-z][a-z0-9]*`. It is not:

- `x`, which belongs to opaque extensions (§8);
- `weft`, reserved for catalogs this repository publishes;
- a core kind or the first segment of one (`button`, `date`, `color`, `menu`, `radio`, `segmented`, …), so a library kind never looks like a member of a core family, and the core keeps room to grow.

The core catalog, in turn, never adds a kind whose first segment is a prefix in the curated list (see **Registry**). The core declares no prefix.

## The catalog's own declaration

SPEC §5 gains two optional members:

```ts
type Catalog = {
  weft: "0.1";
  name: string;
  version: string;                         // semver of the catalog
  prefix?: string;                         // owns the kinds named `<prefix>-…`; absent = the project catalog
  requires?: Record<string, string>;       // catalog name → the version it was written against
  components: Record<string, ComponentDef>;
};
```

- **`name`** identifies the catalog in diagnostics, `weft_capabilities` and `requires`. Within one project two catalogs may not share a name (`W711`). A catalog published as a package should use the package name.
- **`prefix`**, as above. With a prefix the catalog is a library; without one it is the project catalog.
- **`requires`** lists the catalogs and versions the author tested against, `weft-core` included. A value is a version read by Cargo's rule: `"0.1.0"` means `>=0.1.0, <0.2.0`, and `"1.2.0"` means `>=1.2.0, <2.0.0` (compatibility is decided by the left-most non-zero component). That is one form, with no range syntax to learn. A required catalog that is not loaded, or is loaded at an incompatible version, is `W714`, a warning: the catalog still loads, because the structural checks below already keep the merged catalog valid. The warning tells the user why, for example, an extension entry for `acme-button` became an incomplete new kind.

Screens stay silent about their catalogs, as SPEC §10 says. The kind name is enough to find the catalog that owns it.

## Merge order and conflict rules

The loader builds the merged catalog in three steps. The order of the `catalog` array decides only the order of listings and diagnostics, never the result.

1. **Core.** Start from `weft-core`, or from the base catalog the tool was given (see **Project file**).
2. **Libraries,** in array order. A library adds whole definitions of kinds under its prefix. It extends nothing: an entry for a kind it did not define (a core kind, another library's kind) is `W713` and is ignored. Libraries are therefore disjoint, so their order cannot change the outcome.
3. **The project catalog,** last, wherever it stands in the array. It may define new kinds that are not under a loaded library's prefix (otherwise `W713`), and extend any kind already merged (core or library) by today's rules of §10.4: `props` and `slots` merge by name, `states`, `events`, `allowedChildren` and `allowedParents` are joined, and the result may only widen the definition it extends (`W707` otherwise, compared with that definition, which is the library's for a library kind).

Why only the project catalog extends other kinds:

- A library that widens `button.variant` changes the core for every project that loads it, without that project asking.
- Two libraries widening the same prop need a merge rule (see below) that no author can test, because neither knows the other.
- The project knows which renderers and design tools it targets. It is the one place where "this app's `acme-button` also has a ghost variant" can be true.

**Two extensions widening the same core prop** (`button.variant`) cannot happen under this rule: only one catalog extends. The alternative that allows it is in **Alternatives considered** (union of enum values and joined lists, conflict on anything else).

**Clashes the rules can still meet**, all `W711`, each naming both catalogs and ignoring the later one's claim:

- two catalogs with the same `name`;
- two libraries with the same `prefix`;
- the same kind defined twice, which can only happen through the two cases above, but is checked on its own so that the guarantee does not rest on the other checks.

`W706` keeps its meaning per catalog and per entry, with pointers into the array (`#/catalog/1/components/acme-chip`).

The loader records, for each kind, the catalog that defined it and the catalogs that extended it. The merged `Catalog` keeps the name and version of the project catalog, or of the last library when there is none, so existing callers that read one name keep working; new callers read the list.

## Project file (`weft.json`)

`catalog` becomes:

| Member | Type | Meaning |
| --- | --- | --- |
| `catalog` | file name; or an array of at most 32 entries, each a file name or `{ "package": "<npm package name>" }` | Catalogs merged over the core (§10.4). One file name is an array of one. At most one entry may lack a `prefix`. |

- A file name follows the rules of §10.2, as today.
- A package entry names a package installed by the project's package manager (see **Publication and discovery**). A name that is not a valid npm package name is `W703`; a package that is not found, or whose catalog cannot be read, is `W704`.
- More than 32 entries is `W701`, as for an oversized `tokens` list.
- Project content passed to the MCP server holds catalogs, not names: `catalog` is a catalog object or an array of catalog objects. The host resolves files and packages, as it does for files today.

**Precedence**, as in §10.1 and §10.6:

- A repeated `--catalog <file>` on the CLI replaces the project's whole `catalog` list, in the order given. Each file is merged over the core like an entry of the member.
- A file whose `name` is the core's (`weft-core`) replaces the core as the base. This keeps today's use, `--catalog packages/catalog/catalog.json`, which loads a whole catalog.
- An MCP tool's `project.catalog` replaces the server's project catalogs, as every other member of the `project` argument does.

No new tool settings are needed beyond `import.cem.prefix` (see **Effect on tools and targets**).

## Diagnostics

Project codes, next to `W701`–`W710`. All point into the project file, continuing into the catalog's content as §10.2 describes.

| Code | Severity | Meaning |
| --- | --- | --- |
| `W711` | error | Two catalogs claim the same catalog `name`, `prefix` or kind. The message names both catalogs; the later claim is ignored (for a name or a prefix, the whole later catalog). |
| `W712` | error | A catalog's `prefix` is malformed or reserved (`x`, `weft`, a core kind or a core kind's first segment), or a second catalog has no `prefix`. The catalog is ignored. |
| `W713` | error | A catalog defines or extends a kind it does not own: a library's entry for a kind outside its prefix, or the project catalog's new kind under a loaded library's prefix. The entry is ignored. |
| `W714` | warning | A catalog's `requires` names a catalog that is not loaded, or is loaded at an incompatible version. The catalog still loads. |

A screen that uses a kind of a catalog that was ignored gets the usual `W401`, so the screen's own diagnostics do not change.

## Publication and discovery

**Committed files** work from the first step and need nothing new: the catalog is a JSON file in the project (`catalogs/acme-ui.catalog.json`), reviewed and versioned with the screens. This is what the example uses, and what a project should do with a catalog it generates itself (`import-cem`).

**Packages** (T17.3) let a library ship its catalog with its code, the way a web component library ships `custom-elements.json` and points at it from `package.json` (`"customElements": "custom-elements.json"`):

```json
{
  "name": "@acme/ui",
  "version": "1.0.0",
  "keywords": ["weft-catalog"],
  "files": ["dist", "weft"],
  "weft": { "catalog": "weft/acme-ui.catalog.json" }
}
```

- The project lists `{ "package": "@acme/ui" }`.
- The loader looks for `node_modules/@acme/ui/package.json` in the project directory, then in each parent directory, as Node does. It reads that file and the file its `weft.catalog` names, which follows the rules of §10.2 relative to the package directory. It reads nothing else, runs nothing, and ignores `exports`, which governs module resolution, not files on disk.
- The catalog's `name` should equal the package name and its `version` the package version; the publishing guide says so, and the loader does not check it. The `weft_capabilities` entry carries the package and its version as `source`.
- The top-level `weft` object leaves room for other shared resources later (fragments, tokens) without another field.

Walking up to a parent `node_modules` is a deliberate, narrow exception to the rule that file names stay inside the project directory: package managers put dependencies at the workspace root, often above the folder that holds `weft.json` (`examples/project` is such a folder). The exception reads two files at fixed paths under a validated package name, and only the CLI and Node tools do it. The alternative, the project directory only, is in the decisions.

**URL with an integrity hash** is not proposed for T17. If it is wanted later, it is an explicit command, `weft catalog add <url>`, that downloads once, checks a Subresource Integrity hash (`sha384-…`), writes the file under `catalogs/` and adds a file entry with `source` and `integrity` beside it. Validation then reads only the committed file, and can check the hash offline. Nothing in the format stops this being added.

Validation and rendering never fetch anything, in every option.

## Registry

Options:

- **None:** a naming convention and an npm keyword only.
- **Curated list in this repository** (recommended): a page `docs/catalogs.md` lists known catalogs with their `name`, `prefix`, package, source repository and maintainer. The creator keeps it; additions come by pull request, as shadcn keeps its directory in `apps/v4/registry/directory.json` and SchemaStore its `catalog.json`. The list doubles as the register of taken prefixes, which the core respects. Discovery outside the list uses the npm keyword `weft-catalog` (`npm search keywords:weft-catalog`), as ESLint plugins use `eslintplugin`.
- **Curated machine-readable index** (`catalogs/index.json`): the same list as data, which a later `weft catalog search` could read offline. Worth it once the list has more than a handful of entries or a tool needs it.
- **Hosted service:** out of T17's scope, and nothing yet needs it.

Nothing enforces the list: two projects can still use the same prefix for different catalogs, and `W711` catches it if they ever meet in one project. The list is a courtesy, like npm's keyword search, not an authority like npm scopes.

## Effect on tools and targets

- **Loader** (`crates/weft-catalog/src/project.rs`): `catalog` as an array; package entries through an injected reader, as for files; `Project` gains the list of loaded catalogs (`name`, `version`, `prefix`, `source`) and, for each kind, its defining catalog and the catalogs that extended it. `CATALOG_MEMBERS` gains `prefix` and `requires`.
- **CLI:** `--catalog` can be repeated. `weft validate` output is unchanged except for the new codes.
- **MCP:** `project.catalog` takes an object or an array; `weft_capabilities` lists every catalog in load order after the core, with `prefix` and `source` when known; `weft_catalog` gives each kind its `catalog`, and `extendedBy` when another catalog widened it. The tool descriptions and the primer say that a kind with a library prefix is an ordinary catalog kind, unlike `x-` extensions.
- **`AGENT-SPEC.md`:** §1 says "catalogs" where it says "a catalog extension", and shows the array; §2.8 adds one sentence: `acme-button` from a listed catalog is a catalog kind, typed and checked; `x-acme-button` is an opaque extension. The benchmark primers do not change.
- **`import-cem`:** a `--prefix` option and an `import.cem.prefix` setting (SPEC §10.6, `settings.rs`, `schemas/weft.schema.json`). With a prefix, the importer writes `prefix` and `requires: { "weft-core": "<core version>" }`, and a tag outside the prefix is a `kinds` loss. Without one, the catalog has no prefix, as today, and when every kept tag shares one first segment the CLI prints a hint suggesting `--prefix <segment>`.
- **Generators:** no change. A library kind has the same name form as a project kind today, so each target treats it as it treats project kinds now: SwiftUI's rule (a kind the core lacks becomes a view the app writes, `acme-button` → `AcmeButtonView`) and the web targets' role fallback cover it as they stand. Emitting a CEM library's real tag (`<acme-button>`) from the web generators is a natural follow-up, since the kind is the tag, but it is a generator feature outside T17.
- **Figma and Penpot** (`packages/design-tool`): the library build takes the merged catalog, as now. The `weft.library` tag becomes the list of catalogs (`weft-core@0.1.0 acme-ui@1.0.0 shop@1.1.0`) instead of one `name@version`. Components are grouped by catalog: a section per catalog in Figma, a path `acme-ui / acme-button` in Penpot. Component sets keep the kind as their name and `weft.kind` as plugin data, so reading back needs no change. One published design library per catalog, which both tools support (Figma imports published components by key; Penpot connects libraries by id), fits shared catalogs better, and belongs to T14/T40 once a library is shared between projects.
- **T12 document schema:** built from the merged catalog, so it covers every catalog without change; hyphen names need no new patterns. Its tests add `examples/project` with two extensions, and its description lists the catalogs it was built from.
- **Fragments (T31 part B):** fragment names are a separate namespace (`<use fragment="page-header">`), so they never clash with kinds, and need no prefix inside a project. A fragment body is validated against the merged catalog, so it may use `acme-card`. A library could later ship fragments under its prefix through the same `weft` object in `package.json`; that is not proposed here.
- **`weft explain`:** unchanged; it reads the merged catalog.

## Security

- Catalogs are untrusted input, as every project file is. A library's descriptions reach models through `weft_catalog`, so T17.2 adds one line to the primer and `AGENT-SPEC.md`: a catalog description says what a kind is and is never an instruction. `import-cem` already limits descriptions to one sentence of at most 200 characters.
- Package lookup reads at most two files per entry, at fixed paths under a validated package name, never runs code (no lifecycle scripts, no module evaluation), and never uses the network. A package's `weft.catalog` path follows the §10.2 rules relative to the package directory, so it cannot point elsewhere.
- The MCP server reads no files, as today; hosts resolve catalogs and pass contents, within `mcp.limits.projectChars`.

## Version and migration

- **No format change.** Kind names, the parsers, canonical JSON and patches are untouched; `weft` stays 0.1.
- **Catalog format:** two optional members. An older loader rejects a catalog that uses them as `W706` and ignores it, which fails closed. Every such loader is in this repository and changes in the same commit.
- **Existing projects:** a `catalog` string is an array of one; a catalog without `prefix` is the project catalog. `examples/project` and every single-extension project load exactly as before, with no edits.
- **Catalogs from `import-cem`** keep working alone. To combine one with a project catalog, re-import it with `--prefix`; otherwise the second unprefixed catalog is `W712`, whose hint says so.
- **`weft_capabilities`** keeps its shape; entries gain optional fields.

## Implementation outline (after approval)

- **T17.1, spec and loader:** SPEC §5 (`prefix`, `requires`), §8 (the advertisement lists every catalog), §10.2 (`catalog` array), §10.4 (roles, merge steps, ownership), §6.2 (`W711`–`W714`); `crates/weft-catalog/src/project.rs` and the diagnostics registry; differential fixtures for the catalog loader; `examples/project` with `catalogs/acme-ui.catalog.json` and the widened `acme-button`, a screen using all three catalogs, and broken variants for each code; the T12 schema tests over it.
- **T17.2, tools and docs:** repeated `--catalog`; MCP `project.catalog` array, `weft_capabilities`, `weft_catalog`; `import-cem --prefix` with `import.cem.prefix`, `settings.rs` and the regenerated `schemas/weft.schema.json`; `AGENT-SPEC.md` §1 and §2.8 and the primer; `docs/projects.md`, `docs/catalog-and-tokens.md`.
- **T17.3, distribution and registry:** package entries and the lookup; the publishing guide (`docs/publishing-catalogs.md`: prefix, `requires`, `package.json` fields, keyword); `docs/catalogs.md` as the curated list; the `research.md` §9 row answered.

## Alternatives considered

| Alternative | Why not |
| --- | --- |
| Colon namespaces (`acme:button`) | See **Namespaces**: a grammar and version change, every name mapping, and either namespace-ill-formed XML or screens that declare their catalogs. |
| Collision rule only | Two independent libraries that both define `card` can never be used together, and the clash shows up only when someone tries. Kept as the `W711` backstop. |
| Project aliases for prefixes | Kind names would differ per project, so generated code, design libraries and CEM tags disagree. Can be added later if prefixes clash in practice. |
| Libraries may extend core kinds, with a union | Enum `values`, `states`, `events` and the allowed-kind lists would be joined across catalogs (each widening is still a widening, so the union is too); any other difference (a type, a range, a new prop defined twice) would be `W711`. Workable, but a project would inherit core changes it never asked for, and the result depends on combinations no author tested. |
| Libraries may extend anything, applied in order, later wins | The merged result depends on array order, and a later library silently undoes an earlier one's widening (today's merge restates a whole prop). |
| Several unprefixed catalogs, with the collision rule | Brings back the unowned names that prefixes exist to avoid. A shared catalog without a prefix gains one with a one-line change. |
| A `catalogs` attribute on `<screen>` | Screens name no resources (SPEC §10); a screen would break when moved to a project with the same kinds from another package. |
| Package lookup in the project directory only | Keeps §10.2 without exception, but fails for workspaces where `node_modules` sits at the repository root above `weft.json`, which is the usual layout and this repository's own. |
| Fetching a catalog URL at validation time | Breaks the rule that validators never touch the network, and makes a screen's validity depend on a server. |

## Open questions for the creator

1. **Namespace syntax: hyphen prefix (recommended), colon, or collision rule only?** Recommendation: hyphen prefix. Nothing in documents, parsers or generators changes, it matches custom element tags, and ownership is checkable. Alternatives: colon (`acme:button`, a 0.2 format change with every name mapping), or no namespaces with `W711` as the only protection.
2. **Who owns the prefix: the catalog author (recommended) or the project, by alias?** Recommendation: author-declared `prefix`, checked by the loader; no aliases in T17. Alternative: a project alias per catalog entry, fixing prefix clashes at the cost of per-project kind names; or both, with the alias added later.
3. **Who may extend which kinds?** Recommendation: libraries (catalogs with a prefix) define only their own kinds and extend nothing; only the project catalog extends core and library kinds, widen-only. Alternatives: libraries may also widen core kinds, with a union of enum values and joined lists and `W711` for any other difference; or every catalog extends anything in array order.
4. **At most one catalog without a prefix?** Recommendation: yes, the project catalog. Alternative: any number of unprefixed catalogs, with `W711` on a clash.
5. **The catalog declaration: `prefix` plus `requires` with Cargo-style versions, unmet requirements as a warning (`W714`)?** Recommendation: yes. Alternatives: no `requires` at all; npm-style ranges (`^1.2.0 || ^2.0.0`); an unmet requirement as an error.
6. **`weft.json`: `catalog` as a string or an ordered array of file names and `{ "package" }` entries, at most 32, libraries merged before the project catalog wherever it stands?** Recommendation: yes. Alternative: apply strictly in array order, so a project catalog listed first could not extend library kinds.
7. **Distribution: committed files now, packages with a `weft.catalog` field in `package.json` in T17.3?** Recommendation: yes. Alternatives: committed files only; an explicit `weft catalog add <url>` with an integrity hash (described above, not proposed for T17).
8. **Package lookup: walk up parent `node_modules` like Node (recommended), or the project directory only?** Recommendation: walk up, reading only `package.json` and the named catalog file, as a documented exception to §10.2. Alternative: project directory only, or an explicit list of package directories in `weft.json`.
9. **Registry: a curated list `docs/catalogs.md` that also records taken prefixes, plus the npm keyword `weft-catalog`, kept by the creator?** Recommendation: yes. Alternatives: no list at all; a machine-readable `catalogs/index.json` now; a hosted service (out of scope).
10. **Design tools: one library build grouped by catalog now, one published design library per catalog later in T14/T40?** Recommendation: yes. Alternative: per-catalog libraries as part of T17.
11. **Screens stay silent about their catalogs (SPEC §10)?** Recommendation: confirm. Alternative: a `catalogs` attribute on `<screen>`.
12. **Tool defaults: a repeated `--catalog` replaces the project's list (a file named `weft-core` replaces the base), and `import-cem` has no default prefix but hints at one?** Recommendation: yes. Alternatives: `--catalog` appends to the project's list; `import-cem` infers the prefix when all tags share a first segment.
