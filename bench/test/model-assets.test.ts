import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import { test } from "vitest";
import { glb, png, usdz } from "../../corpus/showroom/assets/generate.ts";

// The showroom screen ships one authored asset set (corpus/README.md). The files are small so the
// repository stays light, and reproducible so that nobody has to trust a binary.
const dir = new URL("../../corpus/showroom/assets/", import.meta.url);
const KIB = 1024;

test("gem.glb and gem.usdz are exactly what generate.ts writes", () => {
  assert.ok(readFileSync(new URL("gem.glb", dir)).equals(glb()));
  assert.ok(readFileSync(new URL("gem.usdz", dir)).equals(usdz()));
});

test("the asset files stay small", () => {
  for (const [name, bound] of [
    ["gem.glb", 4 * KIB],
    ["gem.usdz", 4 * KIB],
    ["gem.png", 8 * KIB],
  ] as const) {
    assert.ok(statSync(new URL(name, dir)).size <= bound, name);
  }
});

test("the glb is glTF 2.0 and the png has the size of the screen's still", () => {
  const bin = readFileSync(new URL("gem.glb", dir));
  assert.equal(bin.toString("latin1", 0, 4), "glTF");
  assert.equal(bin.readUInt32LE(4), 2);
  assert.equal(bin.readUInt32LE(8), bin.length);
  const still = readFileSync(new URL("gem.png", dir));
  assert.equal(still.toString("latin1", 1, 4), "PNG");
  assert.equal(still.readUInt32BE(16), 320);
  assert.equal(still.readUInt32BE(20), 240);
  assert.equal(png().readUInt32BE(16), 320);
});

test("the licence of the assets is stated in the corpus README", () => {
  const readme = readFileSync(new URL("../../corpus/README.md", import.meta.url), "utf8");
  assert.match(readme, /showroom/);
  assert.match(readme, /Apache-2\.0/);
});
