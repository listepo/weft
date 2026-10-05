// What the token-mode tests of every design tool share: the example project's light and dark
// resolver, and the check that a resolver document read back from a file gives every context the
// values the build was given. Shared so each tool proves the same round trip.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Token, TokenModifier } from "@weft/catalog";
import { readProject, readResolver } from "@weft/catalog/node";
import { sameColor, tokenColor, tokenPx } from "../src/index.ts";

const example = fileURLToPath(new URL("../../../examples/project/weft.json", import.meta.url));

/** The example project's default tokens and its `theme` modifier (`light`, `dark`). */
export function exampleModes(): { tokens: Map<string, Token>; modifier: TokenModifier } {
  const { project, diagnostics } = readProject(example);
  assert.deepEqual(diagnostics, []);
  const modifier = project.modifiers?.find((m) => m.name === "theme");
  assert.ok(project.tokens !== undefined && modifier !== undefined);
  return { tokens: project.tokens, modifier };
}

/** Loads a resolver document as a project would and returns its modifiers. */
export function loadResolver(document: unknown): TokenModifier[] {
  const dir = mkdtempSync(join(tmpdir(), "weft-modes-"));
  try {
    const file = join(dir, "tokens.resolver.json");
    writeFileSync(file, JSON.stringify(document));
    const layers = readResolver(file);
    assert.deepEqual(layers.diagnostics, []);
    return layers.modifiers ?? [];
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * Every pixel and colour token of every context of `expected` has the same value in `document`'s
 * modifier of the same name, which has the same default and contexts.
 */
export function assertSameModes(document: unknown, expected: TokenModifier): void {
  const actual = loadResolver(document).find((m) => m.name === expected.name);
  assert.ok(actual !== undefined, `no modifier ${expected.name}`);
  assert.equal(actual.default, expected.default);
  assert.deepEqual([...actual.contexts.keys()], [...expected.contexts.keys()]);
  let compared = 0;
  for (const [name, tokens] of expected.contexts) {
    const read = actual.contexts.get(name) as Map<string, Token>;
    for (const [path, token] of tokens) {
      const px = tokenPx(token);
      const color = tokenColor(token);
      if (px !== undefined) assert.equal(tokenPx(read.get(path)), px, `${name} ${path}`);
      if (color === undefined) continue;
      const back = tokenColor(read.get(path));
      assert.ok(back !== undefined && sameColor(back, color), `${name} ${path}`);
      compared++;
    }
  }
  assert.ok(compared > 0, "no colour was compared");
}
