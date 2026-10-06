import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse } from "@weft/core";
import { expectedTree, modelAssetUrl } from "../src/index.ts";
import { doc, dom, el, html } from "./helpers.ts";

const gem = {
  src: "assets/gem.glb",
  usdz: "assets/gem.usdz",
  fallback: "assets/gem.png",
  label: "A gem",
};

test("a model is a model-viewer whose child is the poster still", () => {
  const m = dom(doc(el("model", "m", gem))).byId("m");
  assert.equal(m.name, "model-viewer");
  assert.equal(m.attribs["role"], "img");
  assert.equal(m.attribs["alt"], "A gem");
  assert.equal(m.attribs["src"], "assets/gem.glb");
  assert.equal(m.attribs["ios-src"], "assets/gem.usdz");
  assert.equal(m.attribs["interaction-prompt"], "none");
  assert.match(
    html(doc(el("model", "m", gem))),
    /<img slot="poster" alt="" src="assets\/gem\.png"\/><\/model-viewer>/,
  );
});

test("a model without a usdz has no ios-src", () => {
  const { usdz: _, ...rest } = gem;
  assert.equal(dom(doc(el("model", "m", rest))).byId("m").attribs["ios-src"], undefined);
});

test("a model is an img in the declared tree, named by its label", () => {
  const tree = expectedTree(doc(el("model", "m", gem)), { catalog: coreCatalog });
  assert.match(JSON.stringify(tree), /"role":"img"[^}]*"name":"A gem"/);
});

test("an unsafe asset path is dropped by the renderer", () => {
  const out = html(
    doc(
      el("model", "m", {
        src: "javascript:alert(1).glb",
        usdz: "../up.usdz",
        fallback: "data:image/png;base64,AAAA",
        label: "x",
      }),
    ),
  );
  assert.ok(!out.includes("javascript"));
  assert.ok(!out.includes("up.usdz"));
  assert.ok(!out.includes("data:"));
  assert.ok(!out.includes("poster"));
});

// A control character cannot be written in markup at all (W1xx), so only the renderer sees it.
test("the renderer drops a control character in a path", () => {
  assert.equal(modelAssetUrl("src", "a\u0001.glb"), undefined);
  assert.equal(modelAssetUrl("src", "a\u007f.glb"), undefined);
});

const samples: [string, string][] = [
  ["src", "a.glb"],
  ["src", "dir/a.GLTF"],
  ["src", "https://x.test/a.glb"],
  ["src", "https://x.test/a.glb?v=1#top"],
  ["src", "https://x.test/a.png"],
  ["src", "http://x.test/a.glb"],
  ["src", "file:///a.glb"],
  ["src", "data:model/gltf-binary;base64,AA.glb"],
  ["src", "//x.test/a.glb"],
  ["src", "/a.glb"],
  ["src", "../a.glb"],
  ["src", "a/../b.glb"],
  ["src", "a\\b.glb"],
  ["src", "a b.glb"],
  ["src", "a%2e%2e/b.glb"],
  ["src", "a.glb?x"],
  ["src", "a.usdz"],
  ["src", "https://u:p@x.test/a.glb"],
  ["src", "https:///a.glb"],
  ["src", `${"a/".repeat(1100)}a.glb`],
  ["usdz", "a.usdz"],
  ["usdz", "a.glb"],
  ["fallback", "a.png"],
  ["fallback", "a.JPEG"],
  ["fallback", "a.webp"],
  ["fallback", "a.svg"],
  ["fallback", "a.glb"],
];

test("the renderer rule agrees with the W317 check of the core", () => {
  for (const [prop, value] of samples) {
    const props = { ...gem, [prop]: value };
    const markup =
      `<screen id="s" label="S" weft="0.1"><model id="m" fallback="${props.fallback}" ` +
      `label="L" src="${props.src}" usdz="${props.usdz}"/></screen>`;
    const { diagnostics } = parse(markup, { catalog: coreCatalog });
    const w317 = diagnostics.some((d) => d.code === "W317");
    assert.equal(modelAssetUrl(prop, value) === undefined, w317, `${prop}=${value.slice(0, 40)}`);
  }
});
