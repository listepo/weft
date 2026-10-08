// `documentSchema` of `@weft/core/document-schema`: the generator of crates/weft-catalog through
// the web module (or the addon). Its text must be the generator's own bytes, which the insta
// snapshots of crates/weft-snapshots pin for the core catalog and for examples/project.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test } from "vitest";
import { documentSchema } from "../src/document-schema.ts";
import type { Catalog } from "../src/model.ts";
import { wasm } from "../src/web.ts";
import { catalog as fixture } from "./catalog.ts";

const snapshots = new URL(
  "../../../crates/weft-snapshots/tests/snapshots/document-schema/",
  import.meta.url,
);

/** The snapshot's schema, indented by serde_json, in the compact form the binding writes. */
function pinned(name: string): string {
  const snap = readFileSync(new URL(`${name}.snap`, snapshots), "utf8");
  const body = snap.slice(snap.indexOf("\n---\n") + 5);
  // Whitespace outside strings goes; a string keeps every character, escapes included.
  return body.replace(/("(?:[^"\\]|\\.)*")|\s+/g, (_, string?: string) => string ?? "");
}

/** The merged catalog of project content, loaded by the Rust project loader (SPEC §10.4). */
function projectCatalog(content: object): Catalog {
  return (
    JSON.parse(wasm.loadProject(JSON.stringify(content), undefined, "{}")) as { catalog: Catalog }
  ).catalog;
}

describe("documentSchema", () => {
  test("gives the generator's bytes for the core catalog", () => {
    assert.equal(documentSchema(projectCatalog({})), pinned("weft-core"));
  });

  test("gives the generator's bytes for the merged catalog of examples/project", () => {
    const read = (path: string) =>
      JSON.parse(
        readFileSync(new URL(`../../../examples/project/${path}`, import.meta.url), "utf8"),
      ) as object;
    const catalog = [read("catalogs/acme-ui.catalog.json"), read("catalog.json")];
    assert.equal(documentSchema(projectCatalog({ catalog })), pinned("project-shop"));
  });

  test("describes only the kinds of the catalog it is given", () => {
    const { $defs } = JSON.parse(documentSchema(fixture)) as { $defs: Record<string, unknown> };
    const kinds = Object.keys(fixture.components);
    for (const kind of kinds) assert.ok(kind in $defs, kind);
    const core = Object.keys(projectCatalog({}).components).filter((k) => !kinds.includes(k));
    assert.ok(core.length > 0);
    for (const kind of core) assert.ok(!(kind in $defs), kind);
  });

  test("throws on a value that is not a catalog", () => {
    assert.throws(() => documentSchema({ components: 1 } as unknown as Catalog));
  });
});
