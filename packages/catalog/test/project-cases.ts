// Project cases (SPEC §10), shared by project.test.ts and the TypeScript–Rust differential check.
// A case is the text of a project file and the files it may name; `content` cases pass the
// project as content, the way the MCP server receives it, and have no files.
import fc from "fast-check";
import { loadProjectText, type ProjectResult } from "../src/index.ts";

export type ProjectCase = {
  project: string;
  /** Files the project may name; `sharedFiles` when absent. */
  files?: Record<string, string>;
  content?: true;
  codes: string[];
};

const text = (value: unknown) => JSON.stringify(value);

const base = {
  color: {
    $type: "color",
    brand: { $value: "#000000" },
    text: { $value: "{color.brand}" },
  },
  space: { $type: "dimension", s: { $value: { value: 4, unit: "px" } } },
};
const brand = { color: { brand: { $value: "#ff0000" }, accent: { $value: "#00ff00" } } };

const rating = {
  description: "Shows a score as stars.",
  role: "img",
  content: "none",
  requiresLabel: true,
  props: { value: { description: "The score.", type: "number", min: 0, max: 5 } },
};

const extension = (components: Record<string, unknown>) =>
  text({ weft: "0.1", name: "acme", version: "1.2.0", components });

const project = (members: Record<string, unknown>) => text(members);

const badge = {
  description: "A short status label.",
  role: "status",
  content: "text",
  props: { tone: { description: "Colour.", type: "enum", values: ["info", "warn"] } },
};
const library = (name: string, prefix: unknown, components: Record<string, unknown>) => ({
  weft: "0.1",
  name,
  version: "1.0.0",
  prefix,
  requires: { "weft-core": "0.2.0" },
  components,
});
const acmeUi = library("acme-ui", "acme", {
  "acme-badge": badge,
  "acme-button": {
    ...badge,
    role: "button",
    props: { variant: { description: "Emphasis.", type: "enum", values: ["default", "danger"] } },
  },
});
/** A project catalog over `acme-ui`: a new kind, a core kind and a library kind widened. */
const shop = (variants: string[]) => ({
  weft: "0.1",
  name: "shop",
  version: "1.1.0",
  requires: { "weft-core": "0.2.0", "acme-ui": "1.0.0" },
  components: {
    rating,
    button: { events: ["press", "longpress"] },
    "acme-button": {
      props: { variant: { description: "Emphasis.", type: "enum", values: variants } },
    },
  },
});

/** Fragment files (SPEC §10.7): a header, a page that uses it, and broken ones. */
const header =
  '<fragment label="Page header" weft="0.2"><param name="title" type="string" required="true"/>' +
  '<param name="back" type="action"/><stack id="bar" direction="row">' +
  '<button id="back" on-press="{$back}">Back</button>' +
  '<heading id="title" level="1" text="{$title}"/></stack></fragment>';

/** Library fragments (SPEC §10.4): files relative to the catalog file, which is in `lib/`. */
const promo =
  '<fragment weft="0.3"><param name="title" type="string" required="true"/>' +
  '<param name="go" type="action"/><stack id="row" direction="row" gap="{token.space.s}">' +
  '<acme-badge id="badge">New</acme-badge><heading id="title" level="2" text="{$title}"/>' +
  '<button id="go" on-press="{$go}">Go</button></stack></fragment>';
const withFragments = (fragments: Record<string, unknown>) => ({ ...acmeUi, fragments });
const beta = (fragments: Record<string, unknown>, requires: Record<string, string>) => ({
  ...library("beta-ui", "beta", {}),
  requires,
  fragments,
});

export const sharedFiles: Record<string, string> = {
  "fragments/header.weft": header,
  "fragments/page.weft":
    '<fragment weft="0.2"><use id="top" fragment="page-header" title="Shop"/><text id="t">Hi</text></fragment>',
  "fragments/loop.weft":
    '<fragment weft="0.2"><stack id="s"><use id="again" fragment="loop"/></stack></fragment>',
  "fragments/broken.weft":
    '<fragment weft="0.2"><stack id="a"><outlet name="nope"/></stack></fragment>',
  "fragments/screen.weft": '<screen id="s" weft="0.2"><text id="t">x</text></screen>',
  "fragments/bad.weft": "<fragment><",
  "fragments/uses-promo.weft":
    '<fragment weft="0.3"><use id="promo" fragment="acme-promo" title="Sale"/></fragment>',
  "lib/fragments/acme-promo.weft": promo,
  "lib/fragments/acme-reach.weft":
    '<fragment weft="0.3"><stack id="s" gap="{token.space.nope}">' +
    '<text id="t" text="{$.user.name}"/><button id="b" on-press="cart.add">Add</button></stack></fragment>',
  "lib/fragments/acme-wide.weft":
    '<fragment weft="0.3"><stack id="s"><rating id="r" label="Score" value="3"/></stack></fragment>',
  "lib/fragments/beta-page.weft":
    '<fragment weft="0.3"><use id="promo" fragment="acme-promo" title="Sale"/></fragment>',
  "lib/acme-kit.json": text(withFragments({ "acme-promo": "fragments/acme-promo.weft" })),
  "lib/beta-ui.json": text(
    beta({ "beta-page": "fragments/beta-page.weft" }, { "weft-core": "0.2.0", "acme-ui": "1.0.0" }),
  ),
  "tokens/base.tokens.json": text(base),
  "tokens/brand.tokens.json": text(brand),
  "tokens/list.tokens.json": "[1, 2]",
  "tokens/broken.tokens.json": text({ color: { $type: "color", x: { $value: "{color.nope}" } } }),
  "broken.json": "{",
  "deep.json": `${"[".repeat(800)}${"]".repeat(800)}`,
  "huge.json": '{"a": 1e400}',
  "surrogate.json": '{"a": "\\ud800"}',
  "proto.tokens.json": '{"__proto__": {"$type": "color", "x": {"$value": "#fff"}}}',
  "data.schema.json": text({
    type: "object",
    properties: { user: { type: "object", properties: { name: { type: "string" } } } },
  }),
  "data-anyof.schema.json": text({ type: "object", properties: { a: { anyOf: [] } } }),
  "data-bad.schema.json": text({ type: "thing" }),
  "lib/acme-ui.json": text(acmeUi),
  "shop.json": text(shop(["default", "danger", "ghost"])),
  "catalog.json": extension({
    rating,
    button: {
      props: {
        variant: {
          description: "Visual emphasis.",
          type: "enum",
          values: ["primary", "secondary", "danger", "ghost"],
          default: "secondary",
        },
      },
      events: ["press", "longpress"],
      states: ["idle"],
    },
    text: { content: "mixed" },
  }),
};

export const projectCases: Record<string, ProjectCase> = {
  "an empty project": { project: "{}", codes: [] },
  "a project file that is not JSON": { project: "{", codes: ["W701"] },
  "a project file that is an array": { project: "[]", codes: ["W701"] },
  "a project file that nests too deep": {
    project: `{"a": ${"[".repeat(800)}${"]".repeat(800)}}`,
    codes: ["W701"],
  },
  "an unknown member and a $schema that is no string": {
    project: project({ token: [], $schema: 1 }),
    codes: ["W702", "W701"],
  },
  "a $schema string is ignored": {
    project: project({ $schema: "https://example.com/weft.schema.json" }),
    codes: [],
  },
  "a __proto__ member is just unknown": {
    project: '{"__proto__": {"tokens": "x"}}',
    codes: ["W702"],
  },
  "token layers, later wins": {
    project: project({ tokens: ["tokens/base.tokens.json", "tokens/brand.tokens.json"] }),
    codes: [],
  },
  "token layers, reversed": {
    project: project({ tokens: ["tokens/brand.tokens.json", "tokens/base.tokens.json"] }),
    codes: [],
  },
  "a token file on its own lacks types": {
    project: project({ tokens: ["tokens/brand.tokens.json"] }),
    codes: ["W705", "W705"],
  },
  "tokens that are neither an array nor a file name": {
    project: project({ tokens: 7 }),
    codes: ["W701"],
  },
  "a resolver file that cannot be read": { project: project({ tokens: "x" }), codes: ["W704"] },
  "a token file named alone is read as a resolver": {
    project: project({ tokens: "tokens/base.tokens.json" }),
    codes: ["W705", "W705"],
  },
  "too many token files": {
    project: project({ tokens: Array.from({ length: 65 }, () => "tokens/base.tokens.json") }),
    codes: ["W701"],
  },
  "exactly 64 token files": {
    project: project({ tokens: Array.from({ length: 64 }, () => "tokens/base.tokens.json") }),
    codes: [],
  },
  "file names that leave the project": {
    project: project({
      tokens: ["/etc/x.json", "../x.json", "a//b.json", "a\\b.json", "c:x.json", "", "a/../b", 5],
    }),
    codes: ["W703", "W703", "W703", "W703", "W703", "W703", "W703", "W701"],
  },
  "a dot segment stays inside": {
    project: project({ tokens: ["./tokens/base.tokens.json"] }),
    files: { ...sharedFiles, "./tokens/base.tokens.json": text(base) },
    codes: [],
  },
  "files that cannot be read or are not JSON": {
    project: project({
      tokens: ["missing.json", "broken.json", "deep.json", "huge.json", "surrogate.json"],
    }),
    codes: ["W704", "W704", "W704", "W704", "W704"],
  },
  "a token file that is not an object": {
    project: project({ tokens: ["tokens/list.tokens.json", "tokens/base.tokens.json"] }),
    codes: ["W705"],
  },
  "a token problem after the merge": {
    project: project({ tokens: ["tokens/base.tokens.json", "tokens/broken.tokens.json"] }),
    codes: ["W705"],
  },
  "a __proto__ token group": {
    project: project({ tokens: ["proto.tokens.json", "proto.tokens.json"] }),
    codes: [],
  },
  actions: {
    project: project({ actions: ["cart.add", "Bad", 5, "nav.home", "a..b"] }),
    codes: ["W708", "W701", "W708"],
  },
  "actions that are not an array": { project: project({ actions: "cart.add" }), codes: ["W701"] },
  "a catalog extension": {
    project: project({ catalog: "catalog.json" }),
    codes: [],
  },
  "a catalog member that is not a name": {
    project: project({ catalog: 5 }),
    codes: ["W701"],
  },
  "a catalog that is not a catalog": {
    project: project({ catalog: "bad.json" }),
    files: { "bad.json": text({ name: "acme", components: {} }) },
    codes: ["W706"],
  },
  "a catalog with an extra member": {
    project: project({ catalog: "bad.json" }),
    files: {
      "bad.json": text({ weft: "0.1", name: "a", version: "1.0.0", components: {}, x: 1 }),
    },
    codes: ["W706"],
  },
  "new kinds with bad names": {
    project: project({ catalog: "c.json" }),
    files: {
      "c.json": extension({
        "x-acme-map": rating,
        each: rating,
        slot: rating,
        use: rating,
        outlet: rating,
        Rating: rating,
        ["__proto__"]: rating,
        "2": rating,
      }),
    },
    codes: ["W706", "W706", "W706", "W706", "W706", "W706", "W706", "W706"],
  },
  "a new kind without a role": {
    project: project({ catalog: "c.json" }),
    files: { "c.json": extension({ rating: { ...rating, role: undefined } }) },
    codes: ["W706"],
  },
  "null is not a value": {
    project: project({ catalog: "c.json" }),
    files: {
      "c.json": extension({
        rating: { ...rating, allowedParents: null },
        button: { events: ["press", null] },
        link: { description: null },
      }),
    },
    codes: ["W706", "W706", "W706"],
  },
  "an entry that is not an object": {
    project: project({ catalog: "c.json" }),
    files: { "c.json": extension({ button: "primary", rating: [] }) },
    codes: ["W706", "W706"],
  },
  "a field of the wrong type": {
    project: project({ catalog: "c.json" }),
    files: {
      "c.json": extension({ button: { props: 5 }, link: { states: "x" }, list: { role: 1 } }),
    },
    codes: ["W706", "W706", "W706"],
  },
  "narrowing changes keep the core definition": {
    project: project({ catalog: "c.json" }),
    files: {
      "c.json": extension({
        button: { role: "link" },
        field: { props: { mask: { description: "Mask.", type: "string", required: true } } },
        stack: { allowedChildren: ["text"] },
        heading: { content: "none" },
        image: { props: { src: { description: "Where.", type: "number", required: true } } },
        alert: { requiresLabel: true },
      }),
    },
    codes: ["W707", "W707", "W707", "W707", "W707", "W707"],
  },
  "widening changes are kept": {
    project: project({ catalog: "c.json" }),
    files: {
      "c.json": extension({
        "radio-group": { allowedChildren: ["option"] },
        text: { content: "mixed", description: "Any text." },
        field: { requiresLabel: false, states: ["valid", "busy"] },
        list: { slots: { footer: { description: "Below the items." } } },
      }),
    },
    codes: [],
  },
  "an unknown definition field": {
    project: project({ catalog: "c.json" }),
    files: { "c.json": extension({ button: { pattern: "x" } }) },
    codes: ["W706"],
  },
  "a library and the project catalog": {
    project: project({ catalog: ["lib/acme-ui.json", "shop.json"] }),
    codes: [],
  },
  "the project catalog merges last wherever it is listed": {
    project: project({ catalog: ["shop.json", "lib/acme-ui.json"] }),
    codes: [],
  },
  "a list of one catalog": { project: project({ catalog: ["catalog.json"] }), codes: [] },
  "an empty list of catalogs": { project: project({ catalog: [] }), codes: [] },
  "too many catalogs": {
    project: project({ catalog: Array.from({ length: 33 }, () => "catalog.json") }),
    codes: ["W701"],
  },
  "exactly 32 catalogs, all but one claiming a taken name": {
    project: project({ catalog: Array.from({ length: 32 }, () => "catalog.json") }),
    codes: Array.from({ length: 31 }, () => "W711"),
  },
  "catalog list entries that are not file names": {
    project: project({ catalog: ["lib/acme-ui.json", 5, "../x.json", "missing.json"] }),
    codes: ["W701", "W703", "W704"],
  },
  "a catalog named after the core": {
    project: project({ catalog: ["core.json"] }),
    files: { "core.json": text({ ...shop([]), name: "weft-core", requires: undefined }) },
    codes: ["W711"],
  },
  "two libraries with one prefix": {
    project: project({ catalog: ["lib/acme-ui.json", "two.json"] }),
    files: { ...sharedFiles, "two.json": text({ ...acmeUi, name: "acme-two" }) },
    codes: ["W711"],
  },
  "malformed and reserved prefixes": {
    project: project({ catalog: ["x", "weft", "date", "button", "Acme", "a-b", "", "ok"] }),
    files: Object.fromEntries(
      ["x", "weft", "date", "button", "Acme", "a-b", "", "ok"].map((prefix) => [
        prefix,
        text(library(`lib-${prefix}`, prefix, {})),
      ]),
    ),
    codes: ["W712", "W712", "W712", "W712", "W712", "W712", "W703"],
  },
  "a second catalog without a prefix": {
    project: project({ catalog: ["catalog.json", "shop.json"] }),
    codes: ["W712"],
  },
  "a library defines only kinds under its prefix": {
    project: project({ catalog: ["lib.json"] }),
    files: {
      "lib.json": text(
        library("acme-ui", "acme", { "acme-chip": badge, button: { states: ["x"] }, acme: badge }),
      ),
    },
    codes: ["W713", "W713"],
  },
  "the project catalog defines no kind under a library prefix": {
    project: project({ catalog: ["lib/acme-ui.json", "chip.json"] }),
    files: { ...sharedFiles, "chip.json": extension({ "acme-chip": badge, "acmes-chip": badge }) },
    codes: ["W713"],
  },
  "unmet requirements warn and the catalog still loads": {
    project: project({ catalog: ["shop.json", "old.json"] }),
    files: {
      ...sharedFiles,
      "old.json": text({
        ...acmeUi,
        name: "old-ui",
        prefix: "old",
        requires: { "weft-core": "1.0.0" },
        components: {},
      }),
    },
    codes: ["W714", "W714", "W706"],
  },
  "requirements that are not versions": {
    project: project({ catalog: ["a.json", "b.json", "c.json"] }),
    files: {
      "a.json": text({ ...acmeUi, requires: { "weft-core": "0.1" } }),
      "b.json": text({ ...acmeUi, requires: { "weft-core": 1 } }),
      "c.json": text({ ...acmeUi, requires: ["weft-core"] }),
    },
    codes: ["W706", "W706", "W706"],
  },
  "a prefix that is not a string": {
    project: project({ catalog: ["a.json"] }),
    files: { "a.json": text({ ...acmeUi, prefix: 7 }) },
    codes: ["W706"],
  },
  "the project catalog may not narrow a library kind": {
    project: project({ catalog: ["lib/acme-ui.json", "narrow.json"] }),
    files: { ...sharedFiles, "narrow.json": text(shop(["default"])) },
    codes: ["W707"],
  },
  "a data schema": {
    project: project({ data: "data.schema.json" }),
    codes: [],
  },
  "a data schema with unsupported and malformed keywords": {
    project: project({ data: "data-anyof.schema.json", tokens: [] }),
    codes: ["W710"],
  },
  "a malformed data schema": {
    project: project({ data: "data-bad.schema.json" }),
    codes: ["W709"],
  },
  "a data file name that is no string": {
    project: project({ data: { type: "object" } }),
    codes: ["W701"],
  },
  "every member at once": {
    project: project({
      $schema: "weft.schema.json",
      tokens: ["tokens/base.tokens.json", "tokens/brand.tokens.json"],
      catalog: "catalog.json",
      actions: ["cart.add"],
      data: "data.schema.json",
    }),
    codes: [],
  },
  "every tool section with valid settings": {
    project: project({
      validate: { mode: "strict" },
      format: { write: true },
      render: { data: "sample.json", tokens: ["tokens/base.tokens.json"], outDir: "out/pages" },
      export: { react: { outDir: "src/screens" } },
      import: { html: { outDir: "imported" } },
      mcp: { limits: { markupChars: 1000, patches: 5, inputElements: 30_000 } },
      plugins: { "my-plugin": { anything: [1, "x"] } },
    }),
    codes: [],
  },
  "tool settings of the wrong type are left out": {
    project: project({
      validate: { mode: "loose" },
      format: { write: "yes" },
      render: { data: 7, tokens: 7, outDir: "../out" },
      export: [],
      mcp: { limits: { markupChars: 0, patches: 2.5, diagnostics: -1 } },
      plugins: { a: 1 },
    }),
    codes: ["W701", "W701", "W701", "W701", "W703", "W701", "W701", "W701", "W701", "W701"],
  },
  "unknown keys in tool sections only warn": {
    project: project({
      validate: { mod: "strict" },
      export: { cobol: { outDir: "ios" }, react: { outdir: "x" } },
      import: { penpot: {} },
      mcp: { limit: {} },
    }),
    codes: ["W702", "W702", "W702", "W702", "W702"],
  },
  "the Open Design plugin's settings are checked, other plugins are not": {
    project: project({
      plugins: {
        "open-design": { tokensDir: "tokens/imported" },
        "my-plugin": { tokensDir: "../anything" },
      },
    }),
    codes: [],
  },
  "a bad Open Design tokensDir is a file name error, a wrong key an unknown key": {
    project: project({
      plugins: { "open-design": { tokensDir: "../escape", tokenDir: "tokens" } },
    }),
    codes: ["W703", "W702"],
  },
  "an Open Design tokensDir that is no string": {
    project: project({ plugins: { "open-design": { tokensDir: 3 } } }),
    codes: ["W701"],
  },
  "file names in tool sections follow the project rules": {
    project: project({ render: { tokens: ["ok.json", "/abs.json", "a\\b.json"], data: "c:/x" } }),
    codes: ["W703", "W703", "W703"],
  },
  "fragments named by files, one using another": {
    project: project({
      fragments: { "page-header": "fragments/header.weft", page: "fragments/page.weft" },
    }),
    codes: [],
  },
  "fragments that are not an object": { project: project({ fragments: [] }), codes: ["W701"] },
  "fragments with a bad name, a non-string, a bad file name and a missing file": {
    project: project({
      fragments: { Bad: "fragments/header.weft", a: 1, b: "../x.weft", c: "fragments/none.weft" },
    }),
    codes: ["W701", "W701", "W703", "W704"],
  },
  "fragment markup problems keep their codes": {
    project: project({
      fragments: {
        bad: "fragments/bad.weft",
        broken: "fragments/broken.weft",
        screen: "fragments/screen.weft",
      },
    }),
    codes: ["W110", "W101", "W804", "W201"],
  },
  "a fragment that uses itself": {
    project: project({ fragments: { loop: "fragments/loop.weft" } }),
    codes: ["W805"],
  },
  "a catalog file with fragments": {
    project: project({ catalog: "bad.json" }),
    files: {
      "bad.json": text({ weft: "0.1", name: "a", version: "1.0.0", components: {}, fragments: {} }),
    },
    codes: ["W706"],
  },
  "a library fragment, used by a project fragment, with the project's tokens": {
    project: project({
      tokens: ["tokens/base.tokens.json"],
      catalog: ["lib/acme-kit.json"],
      fragments: { page: "fragments/uses-promo.weft" },
    }),
    codes: [],
  },
  "a library fragment that reaches the host, and a token the project lacks": {
    project: project({
      tokens: ["tokens/base.tokens.json"],
      catalog: ["lib/acme.json"],
      actions: ["cart.add"],
    }),
    files: {
      ...sharedFiles,
      "lib/acme.json": text(withFragments({ "acme-reach": "fragments/acme-reach.weft" })),
    },
    codes: ["W306", "W716", "W716"],
  },
  "a library fragment sees the library's scope, not the project catalog's kinds": {
    project: project({ catalog: ["lib/acme.json", "catalog.json"] }),
    files: {
      ...sharedFiles,
      "lib/acme.json": text(withFragments({ "acme-wide": "fragments/acme-wide.weft" })),
    },
    codes: ["W401"],
  },
  "fragment names outside their owner": {
    project: project({
      catalog: ["lib/acme.json"],
      fragments: { "acme-page": "fragments/page.weft" },
    }),
    files: {
      ...sharedFiles,
      "lib/acme.json": text(
        withFragments({
          promo: "fragments/acme-promo.weft",
          "beta-x": "fragments/acme-promo.weft",
        }),
      ),
    },
    codes: ["W715", "W715", "W715"],
  },
  "a library uses the fragments of a library it requires": {
    project: project({ catalog: ["lib/beta-ui.json", "lib/acme-kit.json"] }),
    codes: [],
  },
  "a library does not see the fragments of a library it does not require": {
    project: project({ catalog: ["lib/beta.json", "lib/acme-kit.json"] }),
    files: {
      ...sharedFiles,
      "lib/beta.json": text(
        beta({ "beta-page": "fragments/beta-page.weft" }, { "weft-core": "0.2.0" }),
      ),
    },
    codes: ["W801"],
  },
  "library fragment entries of the wrong shape": {
    project: project({ catalog: ["lib/a.json", "lib/b.json"] }),
    files: {
      "lib/a.json": text(withFragments({ "acme-x": 1, Bad: "x.weft", "acme-y": "../x.weft" })),
      "lib/b.json": text({ ...beta({}, {}), fragments: [] }),
    },
    codes: ["W706", "W706", "W706", "W703"],
  },
  "content: a library with fragments as markup": {
    project: project({
      catalog: [withFragments({ "acme-promo": promo })],
      fragments: {
        page: '<fragment weft="0.3"><use id="p" fragment="acme-promo" title="x"/></fragment>',
      },
    }),
    content: true,
    codes: [],
  },
  "content: fragments as markup": {
    project: project({ fragments: { "page-header": header } }),
    content: true,
    codes: [],
  },
  "content: tool settings point below the argument": {
    project: project({ validate: { mode: 1 } }),
    content: true,
    codes: ["W701"],
  },
  "content: every member": {
    project: project({
      tokens: [base, brand],
      catalog: JSON.parse(sharedFiles["catalog.json"] ?? "null") as unknown,
      actions: ["cart.add"],
      data: { type: "object", properties: { a: { type: "string" } } },
    }),
    content: true,
    codes: [],
  },
  "content: a list of catalogs": {
    project: project({ catalog: [shop(["default", "danger", "ghost"]), acmeUi] }),
    content: true,
    codes: [],
  },
  "content: a list entry that is not a catalog": {
    project: project({ catalog: [acmeUi, "shop.json"] }),
    content: true,
    codes: ["W706"],
  },
  "content: problems point below the argument": {
    project: project({
      tokens: [[1], base, { color: { y: { $value: "{nope}" } } }],
      catalog: { weft: "0.1" },
      data: { properties: { a: { not: {} } } },
      extra: true,
    }),
    content: true,
    codes: ["W702", "W705", "W705", "W706", "W710"],
  },
};

/** Random projects assembled from the files above, so both languages meet odd combinations. */
export function randomProjects(seed: number, runs: number): [string, ProjectCase][] {
  const names = [...Object.keys(sharedFiles), "missing.json", "../x.json", 7, null];
  const member = fc.oneof(
    fc.constant(undefined),
    fc.constantFrom(...names),
    fc.array(fc.constantFrom(...names), { maxLength: 4 }),
  );
  const actions = fc.oneof(
    fc.constant(undefined),
    fc.array(fc.constantFrom("cart.add", "nav.home", "Bad", 3, "x.y.z", ""), { maxLength: 4 }),
  );
  const projects = fc.record({
    tokens: member,
    catalog: member,
    data: member,
    actions,
    extra: fc.constantFrom(undefined, 1, "x"),
  });
  return fc
    .sample(projects, { seed, numRuns: runs })
    .map((p, i) => [`random project ${i}`, { project: text(p), codes: [] }]);
}

export function runCase(c: ProjectCase): ProjectResult {
  const files = c.files ?? sharedFiles;
  return loadProjectText(
    c.project,
    c.content === true
      ? { prefix: "#/project" }
      : { read: (name) => (Object.hasOwn(files, name) ? files[name] : undefined) },
  );
}
