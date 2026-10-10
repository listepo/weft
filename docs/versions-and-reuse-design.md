# Design: versions and reusable fragments

Status: approved for versions (§3 and the version parts of §7, §8 and §9). Inline fragments, variants and library fragments in this proposal are not built yet. Once each remaining part is approved, it lands in `SPEC.md`, `AGENT-SPEC.md`, the Rust core (which `@weft/core` runs through WebAssembly and the native addon), the MCP primer and the fixtures together, one part per task.

## Decisions

The creator accepted the recommended answer to every open question in §11:

1. Optional `version` on fragments and screens, classified as in §3.
2. A screen's version is classified by its host contract (§3.3), not a free label.
3. One variant parameter per fragment.
4. Library token references are checked against the consuming project.
5. Build order: versions, then inline fragments, then variants. Library fragments wait.

## 1. Problem

Weft already has two things this design builds on:

- **The format version.** `weft="0.2"` on the root says which version of the format a document is written in (SPEC §8). Catalogs have their own semver and pin each other with `requires` (§5, §10.4).
- **Fragments.** A block kept once in its own file and placed with `<use>` (§10.7, T31): value, action and slot parameters, local ids, instance paths, bounded expansion, checks `W801`–`W807`.

What is still missing:

1. **A document has no version of its own.** `weft="0.2"` versions the format, not the content. Nothing says that `page-header` changed its parameters and that every screen using it needs a look, or that a change only moved a label.
2. **Fragments need a project.** A screen that repeats one block three times cannot define it in place; without `weft.json` there are no fragments (§10.7).
3. **Fragments stay in one project.** `docs/fragments-design.md` left sharing to T17, and T17 left fragments out of its scope. A component library can share kinds (T17.1–T17.3) but not the compositions built from them.
4. **A fragment has one structure.** Parameters vary values, but a compact and a full product card differ in which elements they hold, and Weft has no conditional logic (a non-goal of SPEC §1 and of the fragments design).

Two kinds of reuse are in play, and this design keeps them apart:

| | Catalog kind (`acme-button`) | Fragment (`<use fragment="page-header">`) |
| --- | --- | --- |
| What it is | A new element that every renderer must implement natively | A composition of existing kinds, expanded before rendering |
| Works on every target without code | No (SwiftUI needs `AcmeButtonView`, the web falls back to the role) | Yes |
| Where it is defined | A catalog (§5) | A fragment file, now also a screen or a library |
| Planned work | T17.2, T17.3 | This design |

A "widget" in the everyday sense is one or the other. When the host has a native control, it is a kind. When it is built from kinds the catalog already has, it is a fragment.

The design must keep the format's rules: one way to say a thing (SPEC §1), screens name neither their project nor their resources (§10), documents are untrusted and never reach the network (`AGENTS.md`), a published diagnostic code never changes meaning, and models' first-try validity, which the benchmark measures, must not suffer. Every addition is opt-in, so a screen that uses none of it is unchanged.

## 2. Design in short

1. **`version` on `<fragment>` and `<screen>`**: an optional `MAJOR.MINOR.PATCH`. For a fragment the level of a change is computed from its interface by the classifier catalogs already use; a body change is a patch. A command checks that the declared version was raised enough.
2. **Requirements stay on the unit of distribution.** A shared fragment ships in a library, and the library's version is what the project pins, with the `requires` rule that exists. Screens and `<use>` never name versions.
3. **Inline fragments**: `<fragment name="…">` as a direct child of `<screen>`, local to that screen, the same syntax as a fragment file. They work without a project.
4. **Library fragments**: a library catalog (one with a `prefix`) may list fragments named under its prefix, shipped with it as files or in its package. They reach the host only through their parameters.
5. **Variants**: one enum parameter marked `variant="true"` selects one of several `<variant when="…">` bodies. The value is a literal at the use, because structure is not data.
6. **One format bump**, to `weft` 0.3, for every new attribute and element. Library fragments change the catalog file, not the format.

### Example

A screen with an inline fragment that has two variants, versions on both:

```xml
<screen id="cart" label="Cart" version="1.4.0" weft="0.3">
  <fragment label="Price row" name="price-row" version="1.0.0">
    <param name="label" required="true" type="string"/>
    <param name="amount" required="true" type="string"/>
    <param default="normal" name="emphasis" type="enum" values="normal total" variant="true"/>
    <variant when="normal">
      <stack id="row" direction="row" justify="space-between">
        <text id="name" text="{$label}"/>
        <text id="value" text="{$amount}"/>
      </stack>
    </variant>
    <variant when="total">
      <stack id="row" direction="row" justify="space-between">
        <heading id="name" level="3" text="{$label}"/>
        <heading id="value" level="3" text="{$amount}"/>
      </stack>
    </variant>
  </fragment>
  <use id="subtotal" amount="{$.cart.subtotal}" fragment="price-row" label="Subtotal"/>
  <use id="total" amount="{$.cart.total}" emphasis="total" fragment="price-row" label="Total"/>
</screen>
```

`subtotal` expands the `normal` variant, `total` the `total` variant. Both give the instance paths `subtotal/name`, `total/value` and so on, whichever variant was chosen.

A library that ships a fragment beside its kinds (`examples/project/catalogs/acme-ui.catalog.json` after T17.1, with one new member):

```json
{
  "weft": "0.2",
  "name": "acme-ui",
  "version": "1.1.0",
  "prefix": "acme",
  "requires": { "weft-core": "0.2.0" },
  "components": { "acme-button": { "…": "…" }, "acme-card": { "…": "…" } },
  "fragments": { "acme-promo": "fragments/promo.weft" }
}
```

```xml
<fragment label="Promo banner" version="1.0.0" weft="0.3">
  <param name="title" required="true" type="string"/>
  <param name="open" type="action"/>
  <acme-card id="card" elevated="true">
    <heading id="title" level="2" text="{$title}"/>
    <acme-button id="cta" on-acme-click="{$open}" variant="primary">Open</acme-button>
  </acme-card>
</fragment>
```

A screen of a project that lists `acme-ui` uses it like any fragment: `<use id="promo" fragment="acme-promo" title="Spring sale" on-open="nav.sale"/>`.

## 3. Versions

### 3.1 Syntax

- `version` is an optional attribute of the root `<screen>`, of the root `<fragment>` of a fragment file, and of an inline `<fragment>` (§4). Its value is `MAJOR.MINOR.PATCH` with no leading zeros, no pre-release and no build metadata, the form a catalog's `version` already has (§5). Anything else is `W230`.
- It is a literal. A binding or a token reference is `W230` too.
- In canonical markup it sorts with the other attributes, as the root's `weft` does (§3). In canonical JSON a root's `version` is a member of `Document`, beside `weft`; an inline fragment's is in its node's `props`:

```ts
type Document = {
  weft: "0.3";
  version?: string;                  // the root's `version` attribute
  context?: Entry[];
  fragments?: Record<string, Node>;  // inline fragments (§4), by name
  root: Node;
};
```

- A patch changes it with a new operation, `{ op: "set-version"; value: string | null }`, which applies to the document root (a fragment file's root has no id to address). `null` removes it. The value is left to validation (`W230`), as `set` leaves its values. An inline fragment's version is set with `set` addressed into that fragment (§4.4) at its root, `"prop": "version"`.

### 3.2 What a fragment's version means

SemVer 2.0.0 asks for a declared public API. A fragment's public API is what a `<use>` can say: its parameters, their types, defaults, values and ranges, its slot parameters and, with §6, its variant axis. The core already turns these into a component definition (`weft_core::fragment::signature`), and the catalog classifier (`weft_catalog::diff_catalogs`, the rules of §8) already sorts a change of a definition into major, minor or none. So:

| Change | Level | Examples |
| --- | --- | --- |
| A use valid against the old fragment can become invalid or mean something else | major | A parameter or slot removed, a new required parameter, a type changed, an enum value removed, a range narrowed, a default changed |
| Only more uses are valid | minor | A new optional parameter, an enum value added, a parameter no longer required, a range widened |
| The interface is the same, the body differs | patch | A label moved, a token changed, an element added inside the body |
| The canonical JSON is the same | none | Formatting, attribute order |

The body is compared in canonical JSON, so formatting never counts. The fragment's `label` is a description, so a change to it alone is a patch.

Major version zero follows Cargo's rule, as `requires` does: below `1.0.0` an incompatible change raises the minor number and anything else raises the patch number.

### 3.3 What a screen's version means

Nothing uses a screen the way a screen uses a fragment, so a screen's version speaks to its host: the app that supplies its data and actions, the tests and patches that address its ids. Proposed classification, computed from two versions of a screen:

| Level | When |
| --- | --- |
| major | The screen names an action or reads a data path the previous version did not, reads a path at a type the previous did not take, gains a writable binding, or loses an element id |
| minor | The screen gains element ids and nothing above |
| patch | Anything else that changes the canonical JSON |

This is open question 2: a screen's version may also stay a plain label with no rule.

### 3.4 Checking a version

A new CLI command compares two versions of one file and says how far its version must be raised:

```sh
weft version-check old/page-header.weft fragments/page-header.weft
```

It prints the changes with their levels, as the catalog diff does, and the least version the new file may declare. When the new file declares a lower version it reports `W810` at `@version` and exits with status 1, so CI can run it against the merge base. It reads both files as untrusted input and never throws. It also accepts two library catalogs, and then classifies their kinds (the existing diff) and their fragments together: a library's version must be raised as far as its most changed part.

The command reads no settings beyond the project file it already finds, so it adds no `weft.json` key. An MCP tool is not proposed: the agent's work is the edit, and CI checks the version.

### 3.5 Where requirements live

A project pins what it installs. A shared fragment is installed as part of a library (§5), so the library's version is the pin, with the `requires` rule and `W714` that exist. Inside one project, a fragment and the screens that use it change in one commit, and validation already reports every use the change breaks (`W802`, `W205`, `W204`, …), so a per-fragment pin would add nothing that validation does not already check.

Not proposed (see §10): `version` on `<use>`, and a per-fragment version pin in `weft.json`.

## 4. Inline fragments

### 4.1 Syntax

- `<fragment name="…">` is a direct child of `<screen>`, after the `<context>` block if there is one and before the first body element. It takes `name` (required, the name grammar of §2), and optionally `label` and `version`. It has no `id` and no `weft`: it is written in the screen's format version.
- Its content is exactly that of a fragment file: `<param>` elements, then a body or, with §6, `<variant>` elements.
- In canonical JSON the inline fragments are `Document.fragments`, a map from name to the fragment's node (kind `fragment`, `label` and `version` in `props`, the parameters and body in `children`). Canonical markup writes them after the context block, sorted by name.
- An inline fragment anywhere else (deeper in the tree, inside `<each>`, a `<slot>`, another fragment or `insert` markup), with no valid `name`, with a `weft` or other attribute, after a body element, or with a name another inline fragment already has, is `W808`, and that fragment is left out.

### 4.2 Scope and names

- An inline fragment is known only in its screen. It works without a project.
- A screen's `<use>` finds a name among the screen's inline fragments, then among the project's. An inline fragment whose name the project also defines is `W808`: one name has one meaning, and a screen cannot quietly replace a shared header.
- An inline fragment may use the project's fragments and the screen's other inline fragments. Cycles are `W805`, and expansion limits are `W806`, unchanged.
- A fragment file may not hold inline fragments (`W808`). Helpers shared by fragments are fragments of the project.

### 4.3 Ids, validation, meaning

Everything in §10.7 applies unchanged: ids inside an inline fragment are local and may repeat the screen's ids, expanded elements are addressed by instance paths, a fragment is checked once on its own and each use against its parameters, and the canonical form keeps uses unexpanded. A context entry's `for` names a screen id, which for a fragment instance is the id of its `<use>`.

### 4.4 Patches

- `set`, `insert`, `remove` and `move` take an optional `fragment` member, the name of an inline fragment. With it, `id` and `parent` address elements of that fragment's body, its root included; without it, the screen as today. A name that no inline fragment has is `W502`, with the nearest name as the hint.
- `{ op: "add-fragment"; markup: string }` adds an inline fragment, given as `<fragment name="…">` markup. A name that is taken is `W513`.
- `{ op: "remove-fragment"; name: string }` removes one. A name that no inline fragment has is `W502`. A `<use>` that still names it fails the result's validation (`W801`), so the patch list is not applied, as with any invalid result.
- Patches still never reach into a project or library fragment (§10.7).

Moving an inline fragment into the project (`weft fragment extract`) and folding repeated subtrees into a fragment are useful refactors for agents, but they are tools, not format, and are left for later.

## 5. Library fragments

### 5.1 Where they live

- A library catalog (§10.4: a catalog with a `prefix`) gains an optional `fragments` member: an object of name → file name, at most 256 entries. File names follow §10.2 relative to the directory of the catalog file, and stay inside the project directory, or inside the package directory for a package entry (T17.3). In project content (§10.1) a value is the fragment's markup, as in `weft.json`.
- The project catalog still has no `fragments` (`W706`): a project's own fragments live in `weft.json`, the one way to say it.
- A library's fragment name starts with `<prefix>-`. A project fragment (in `weft.json` or inline) does not start with the prefix of a loaded library. A library fragment named outside its prefix, a project fragment under a library's prefix, and a fragment name defined by two catalogs are `W715`; the later claim is left out.
- Package lookup (T17.3) reads the catalog file the package names and then the fragment files that catalog names, under the same path rules. It still reads nothing else and runs nothing.

### 5.2 What a library fragment may use

A library fragment is written by one party and used by many projects it does not know, so it is checked against what the library itself declares, not against the consuming project:

- **Kinds:** the core catalog, the library's own kinds, and the kinds of the catalogs in its `requires`, as they define them. A project catalog's widening of a library kind does not count, because another project does not have it.
- **Fragments:** the library's own fragments and those of the libraries in its `requires`.
- **The host:** only through parameters. A read of the host data model (`$.…`) or an action named literally in an `on-*` attribute is `W716`. Data comes in through value parameters and actions through action parameters, so the project decides both at the use.
- **Tokens:** references are checked against the consuming project's tokens when the project loads (`W306`, with a path into the library, `#/catalog/0/fragments/acme-promo/…`). Whether libraries should also ship default tokens is open question 4.

### 5.3 Versions

The library's `version` covers its kinds and its fragments; `weft version-check` classifies both (§3.4). Each fragment of the library may also carry its own `version`, which `weft_catalog` shows, but the version a project pins is the library's.

## 6. Variants

### 6.1 Syntax

- A `<param>` of type `enum` may take `variant="true"`. A fragment has at most one variant parameter.
- A fragment with a variant parameter has, after its parameters, only `<variant when="…">` elements. `when` lists one or more of the parameter's values, separated by spaces. Each value is covered by exactly one `<variant>`. A `<variant>` holds a body: one or more elements, outlets included.
- `<variant>` is structural, like `<outlet>`: no id, no attribute but `when`.
- `variant="true"` on a parameter that is not an enum or on a second parameter, a `<variant>` without a variant parameter or outside a fragment, a `when` value the enum does not have or that two variants cover, an enum value no variant covers, a body element beside the variants, or another attribute on `<variant>`, is `W809`.
- In canonical JSON a variant is a node of kind `variant` with `when` in `props` as a string, in document order after the parameters.

### 6.2 At a use

- The variant parameter takes a literal: which structure a use has is structure, not data, as for `grow` (§2.2). The fragment's signature declares the parameter with `bindable: false`, so a binding is the existing `W217`, with no new code.
- Without the attribute the parameter's `default` applies, or, when it is `required`, the use is `W205` as for any required parameter.
- The body may also read the variant parameter as a value, `{$emphasis}`, like any enum parameter.

### 6.3 Ids, validation, expansion

- Ids are local to each variant. The same id may appear in several variants and means the same part of the fragment, so an instance path such as `total/value` stays valid when a patch switches the variant.
- Each variant body is checked on its own, as the body of a fragment is today. An outlet may appear in some variants and not in others; a variant without the outlet of a slot parameter drops that slot's content, and `weft explain` says so.
- At a use the chosen variant is known from the literal, so the checks of top-level elements where the `<use>` stands (`W302`, `W303`, `W304`) apply to that variant's elements.
- Expansion takes the chosen variant's body and goes on as §10.7 says.

### 6.4 Versions

Adding a value to the variant axis widens the enum (minor), removing one narrows it (major), and changing a variant's body is a patch, all from the classifier of §3.2 with no new rule.

## 7. Diagnostics

| Code | Severity | Meaning |
| --- | --- | --- |
| `W230` | error | `version` is not `MAJOR.MINOR.PATCH`, or is not a literal (§3.1). |
| `W513` | error | `add-fragment` names an inline fragment the document already has. |
| `W715` | error | A fragment name is claimed outside its owner: a library fragment outside the library's prefix, a project fragment under a loaded library's prefix, or one name defined by two catalogs; the later claim is left out (§5.1). |
| `W716` | error | A library fragment reaches the host directly: it reads `$.…` or names an action literally (§5.2). |
| `W808` | error | Inline `<fragment>` misplaced or malformed: not a direct child of `<screen>`, after the body, no valid `name`, another attribute, a name that another inline fragment or a project fragment has, or inside a fragment file (§4.1, §4.2). |
| `W809` | error | Variant misdeclared: `variant` on a parameter that is not an enum or on a second one, `<variant>` without a variant parameter or outside a fragment, a value covered twice or not at all, a body beside the variants, another attribute (§6.1). |
| `W810` | error | Reported by `weft version-check` only: the new version is lower than the changes require (§3.4). |

Existing codes reused with their published meaning: `W205`, `W217`, `W302`–`W304`, `W306`, `W502`, `W706`, `W714`, `W801`, `W805`, `W806`.

## 8. Effect on tools and targets

- **Core** (`crates/weft-core`): parsing and canonical form of `version`, inline fragments and variants; validation; `expand` picks the variant and resolves inline fragments before project ones; patches gain `set-version`, `add-fragment`, `remove-fragment` and the `fragment` member.
- **Catalog loader** (`crates/weft-catalog/src/project.rs`): library `fragments`, ownership (`W715`), library-scoped validation (`W716`); `diff_catalogs` reused for fragment signatures and library fragments.
- **CLI:** `weft version-check`. `weft explain` names the chosen variant of each use.
- **MCP:** `weft_patch` takes the new operations; `weft_catalog` lists inline fragments of the given screen and library fragments with their `catalog` and `version`; `weft_capabilities` advertises `weft` 0.3. The primer and `AGENT-SPEC.md` gain one short section on inline fragments and variants, and one sentence that a library fragment is used like any fragment.
- **Generators:** they expand, so they draw inline and library fragments and the chosen variant with no change beyond the core. `to-jsx`, which emits a component per fragment, emits one per inline fragment too, and resolves the variant at the call site because it is a literal.
- **Figma and Penpot** (T14, T40): a fragment maps to a component and a variant parameter to a variant property, each `<variant>` becoming one component of a component set. That mapping belongs to those tasks; this design only keeps it possible.
- **T12 document schema:** admits no inline fragments, variants or `<use>`, for the reason §3.1 already gives for `<use>`. It may admit `version` on the root, a literal of a fixed pattern.
- **Importers:** never produce inline fragments or variants, for the reason §10.7 gives for `<use>`. They keep a `version` they read from a source comment.
- **Benchmark:** the Weft primer of `bench/src/primers.ts` does not change; nothing here is needed to write a valid screen.

## 9. Format version and migration

- `version`, inline `<fragment>`, `<variant>` and `variant` on `<param>` are additions, so they come together with format 0.3 (SPEC §8: a minor version only adds). A 0.2 document is a valid 0.3 document and needs no migration; writers write `weft="0.3"`.
- A 0.2 markup reader warns about the version (`W403`). It reads `version` as an unknown attribute and `<variant>` as an unknown element, which it keeps. It rejects an inline `<fragment>` (`W201` or `W803`), which fails closed. A 0.2 reader of canonical JSON rejects `version` and `fragments` (`W200`), which also fails closed. Every 0.2 reader of this repository moves to 0.3 in the same change, as with context.
- A library's `fragments` member changes the catalog file, not the format. An older loader rejects such a catalog as `W706` and ignores it, which fails closed; every loader is in this repository.
- `weft-core` does not change: no kind is added.

## 10. Alternatives considered

| Alternative | Why not |
| --- | --- |
| `version` on `<use>` (`<use fragment="page-header" version="^1.2">`) | Screens would name versions of resources (§10), every use repeats the pin, two uses in one screen could disagree, and models would have one more thing to write. |
| A per-fragment pin in `weft.json` | A project installs libraries, not single fragments; it cannot hold two versions of one library's fragment. Inside one project validation already catches every broken use. |
| Only the library's version, no `version` on documents | Covers sharing but leaves a project's own fragments and screens with no way to say that their interface changed. Kept as open question 1. |
| A content hash instead of semver | Says that something changed, not whether uses break. A tool can compute it whenever it needs one. |
| Inline fragments in a `<defs>` block, as SVG keeps `<symbol>` | One more container with no meaning of its own; direct children after `<context>` say the same. |
| Inline names shadow project names | A screen could quietly replace a shared header; one name, one meaning is what the rest of the format keeps. |
| Conditional elements (`<if>`, `when` on any element) | Conditional logic is a non-goal. A variant is a closed set of structures chosen by a literal, which the checks of §10.7 can judge statically. |
| Several variant axes with combinations, as Figma variant properties allow | The number of bodies grows with the product of the axes. One axis plus value parameters covers the cases seen so far; open question 3. |
| Library fragments that read host data or name actions | They would work only in projects whose data schema and action names happen to match. Parameters make the contract explicit. |
| Library fragments checked against the consuming project's catalog | Their validity would depend on each project's widenings, which the library author cannot test. |

## 11. Open questions for the creator

1. **Version on documents.** Keep the optional `version` on fragments and screens with the classification of §3 (recommended), or version only libraries?
2. **Screen version rule.** Classify a screen's changes by its host contract (§3.3, recommended), or keep a screen's `version` as a label with no rule?
3. **Variant axes.** One variant parameter per fragment (recommended), or several with one body per combination?
4. **Library tokens.** Check a library fragment's token references against the consuming project's tokens (recommended, nothing new to ship), or let a library ship default tokens that a project may override?
5. **Order of work.** Build in the order of §12 (recommended: versions, inline fragments, variants, then library fragments after T17.3), or another order?

## 12. Implementation outline

Each part is one task under the 500-line limit, changes `SPEC.md` and `AGENT-SPEC.md` in the same commit, and passes the full check.

1. **Versions:** SPEC §3, §6.2, §7, §8, §10.7; `version` in the parser, canonical form and validation (`W230`); `set-version`; `weft version-check` over fragment signatures and library catalogs with `W810`, reusing `diff_catalogs`; the format moves to 0.3.
2. **Inline fragments:** SPEC §10.7; parser, canonical form, validation (`W808`), expansion order, patches with `fragment`, `add-fragment` and `remove-fragment` (`W513`); MCP `weft_catalog`; to-jsx components; a corpus screen that uses one.
3. **Variants:** SPEC §10.7; `variant` on `<param>`, `<variant>` (`W809`), expansion, `weft explain`; signatures with `bindable: false`; the example of §2 as a fixture.
4. **Library fragments:** SPEC §5, §10.4, §10.7; the loader's `fragments` member, ownership (`W715`), library-scoped validation (`W716`); `examples/project` with `acme-promo`; package lookup once T17.3 lands.

## Sources

| Fact | Source (checked) |
| --- | --- |
| SemVer: a declared public API; MAJOR for incompatible changes, MINOR for backward compatible additions, PATCH for backward compatible fixes; `0.y.z` is initial development | https://semver.org/spec/v2.0.0.html (2026-10-10) |
| Cargo's caret rule, already used by catalog `requires`: compatibility is decided by the left-most non-zero component | https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html (2026-10-10), and SPEC §5 of this repository |
| SVG defines templates in the same document with `<symbol>`, which are not rendered directly and are instantiated by `<use>` | SVG 2, §5.4 "The 'symbol' element", https://www.w3.org/TR/SVG2/struct.html (2026-10-10) |
| A Figma component set contains the variants of a component; its variant properties list their values | https://developers.figma.com/docs/plugins/api/ComponentSetNode/ (2026-10-10) |
| A `.slint` file defines one or several components; a type is private to its file unless exported; component libraries are shared between projects by library paths | https://docs.slint.dev/latest/docs/slint/guide/language/coding/file/ (2026-10-10) |
| Weft fragments, catalogs, versioning, projects and patches | `SPEC.md` §3, §5, §7, §8, §10 of this repository; `docs/fragments-design.md`; `docs/extension-catalogs-design.md` |
