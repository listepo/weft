// The shared plugin UI over a fake DOM: what it sends for each button, and what it shows for each
// reply. The plugin bundle tests run the same UI built, inside each tool's split.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { beforeAll, describe, test } from "vitest";
import { loadTokens, type Token } from "@weft/catalog";
import { resolverDocument, type PluginRequest } from "@weft/design-tool";
import { FakeElement } from "./ui-harness.ts";

const tokens = readFileSync(
  new URL("../../catalog/tokens/default.tokens.json", import.meta.url),
  "utf8",
);
const login = readFileSync(new URL("../../../corpus/login/screen.weft", import.meta.url), "utf8");

// A resolver the way a design tool exports it: inline sets only, as a pasted document must be.
// Without the `$root` token, which a resolver document made by `resolverDocument` cannot name.
const base = new Map(
  [...loadTokens(JSON.parse(tokens)).tokens].filter(([path]) => !path.includes("$")),
);
const white = (hex: string, c: number): Token => ({
  type: "color",
  value: { colorSpace: "srgb", components: [c, c, c], hex },
});
const dark = new Map(base).set("color.white", white("#000000", 0));
const themeResolver = JSON.parse(
  JSON.stringify(
    resolverDocument({
      name: "theme",
      default: "light",
      contexts: new Map([
        ["light", base],
        ["dark", dark],
      ]),
    }),
  ),
) as Record<string, any>;
// A second modifier that is not the appearance, to choose between.
const twoModifiers = JSON.stringify({
  ...themeResolver,
  modifiers: {
    density: {
      contexts: {
        compact: [],
        roomy: [{ space: { md: { $type: "dimension", $value: { value: 20, unit: "px" } } } }],
      },
      default: "compact",
    },
    ...themeResolver["modifiers"],
  },
  resolutionOrder: [...themeResolver["resolutionOrder"], { $ref: "#/modifiers/density" }],
});

const elements = new Map<string, FakeElement>();
const byId = (id: string): FakeElement => {
  let found = elements.get(id);
  if (found === undefined) elements.set(id, (found = new FakeElement()));
  return found;
};
const sent: PluginRequest[] = [];
const created: (FakeElement & { download?: string })[] = [];
const blobs: Blob[] = [];
let reply: (message: unknown) => void = () => {};

beforeAll(async () => {
  Object.assign(globalThis, {
    WEFT_DEFAULT_TOKENS: JSON.parse(tokens),
    document: {
      getElementById: byId,
      createElement: () => {
        created.push(new FakeElement());
        return created.at(-1);
      },
    },
  });
  URL.createObjectURL = (blob) => {
    blobs.push(blob as Blob);
    return "blob:fake";
  };
  URL.revokeObjectURL = () => {};
  const { startUi } = await import("../src/index.ts");
  startUi({ send: (request) => sent.push(request), listen: (onReply) => (reply = onReply) });
});

describe("the plugin UI", () => {
  test("sends a parsed screen to build and an export request", () => {
    byId("source").value = login;
    byId("build").click();
    assert.equal(sent.at(-1)?.type, "build");
    byId("export").click();
    assert.equal(sent.at(-1)?.type, "export");
  });

  test("sends nothing for broken markup", () => {
    const before = sent.length;
    byId("source").value = "<screen";
    byId("build").click();
    assert.equal(sent.length, before);
    assert.ok(byId("notes").items.length > 0);
  });

  test("shows replies as text and ignores anything that is not a reply", () => {
    reply({ type: "error", message: "<b>no</b>" });
    assert.equal(byId("status").textContent, "<b>no</b>");
    for (const junk of [null, "built", { type: 1 }]) reply(junk);
    assert.equal(byId("status").textContent, "<b>no</b>");
    reply({ type: "built", id: "1" });
    assert.equal(byId("status").textContent, "Built.");
  });

  test("shows what a build skipped", () => {
    reply({ type: "built", id: "1", notes: ['The theme context "dark" has no Figma mode'] });
    assert.equal(byId("status").textContent, "Built.");
    assert.ok(byId("notes").items.some((i) => i.textContent.includes('"dark"')));
  });

  test("lists the modifiers of a pasted resolver and chooses the appearance one", () => {
    byId("resolver").value = twoModifiers;
    byId("load-resolver").click();
    assert.equal(byId("status").textContent, "Resolver loaded with 2 modifiers.");
    assert.deepEqual(
      byId("modifier").items.map((i) => i.value),
      ["", "theme", "density"],
    );
    assert.equal(byId("modifier").value, "theme");

    byId("source").value = login;
    byId("build").click();
    const request = sent.at(-1);
    assert.ok(request?.type === "build" && request.modifier !== undefined);
    assert.equal(request.modifier.name, "theme");
    assert.deepEqual(request.modifier.contexts.map(([name]) => name).sort(), ["dark", "light"]);
    // The resolver's default context is the token set of the build.
    assert.deepEqual(new Map(request.tokens), base);
  });

  test("builds with the modifier the user picks, or with none", () => {
    byId("modifier").value = "density";
    byId("build").click();
    const picked = sent.at(-1);
    assert.ok(picked?.type === "build" && picked.modifier !== undefined);
    assert.equal(picked.modifier.name, "density");
    assert.equal(picked.modifier.default, "compact");

    byId("modifier").value = "";
    byId("build").click();
    const plain = sent.at(-1);
    assert.ok(plain?.type === "build" && plain.modifier === undefined);
  });

  test("emptying the resolver goes back to one mode", () => {
    byId("resolver").value = "  ";
    byId("load-resolver").click();
    assert.deepEqual(
      byId("modifier").items.map((i) => i.value),
      [""],
    );
    byId("build").click();
    const request = sent.at(-1);
    assert.ok(request?.type === "build" && request.modifier === undefined);
  });

  test("shows the diagnostics of a resolver the loader only partly reads", () => {
    const withMissingFile = {
      ...themeResolver,
      sets: { ...themeResolver["sets"], extra: { sources: [{ $ref: "missing.tokens.json" }] } },
      resolutionOrder: [
        ...themeResolver["resolutionOrder"],
        { $ref: "#/sets/extra" },
        { $ref: "#/sets/nowhere" },
      ],
    };
    byId("resolver").value = JSON.stringify(withMissingFile);
    byId("load-resolver").click();
    const notes = byId("notes").items.map((i) => i.textContent);
    // A file the pasted document cannot read, and a reference that names nothing.
    assert.ok(
      notes.some((n) => n.includes("W704")),
      notes.join("\n"),
    );
    assert.ok(
      notes.some((n) => n.includes("W705")),
      notes.join("\n"),
    );
    // What could be read still loads.
    assert.equal(byId("status").textContent, "Resolver loaded with 1 modifier.");
  });

  test("refuses what is not a resolver, and keeps nothing from the earlier one", () => {
    for (const [text, message] of [
      ["{", "The resolver is not valid JSON."],
      ['{"color":{}}', "The document is not a resolver: no resolutionOrder."],
      [`{"a":"${"x".repeat(1_000_001)}"}`, "The resolver is larger than 1000000 characters."],
    ] as const) {
      byId("resolver").value = text;
      byId("load-resolver").click();
      assert.equal(byId("status").textContent, message);
    }
    byId("build").click();
    const request = sent.at(-1);
    assert.ok(request?.type === "build" && request.modifier === undefined);
  });

  test("refuses a resolver file over the bound before reading it", async () => {
    let read = false;
    const file = {
      size: 1_000_001,
      text: async () => {
        read = true;
        return "";
      },
    };
    await byId("resolver-file").listeners.get("change")?.({ target: { files: [file] } });
    assert.equal(read, false);
    assert.equal(byId("status").textContent, "The resolver is larger than 1000000 characters.");
  });

  test("shows the file's modes as a resolver and offers them as a download", async () => {
    const exported = {
      type: "exported",
      document: { version: "0.1", root: { type: "screen", id: "login", children: [] } },
      losses: [],
      diagnostics: [],
      resolver: themeResolver,
    };
    reply(exported);
    assert.equal(byId("download-resolver").disabled, false);
    const shown = byId("resolver-out").value;
    assert.deepEqual(JSON.parse(shown), themeResolver);

    byId("download-resolver").click();
    assert.equal(created.at(-1)?.download, "tokens.resolver.json");
    assert.equal(await blobs.at(-1)?.text(), shown);

    // What was downloaded loads back as the same modes.
    byId("resolver").value = shown;
    byId("load-resolver").click();
    assert.equal(byId("modifier").value, "theme");

    // A file with one mode offers nothing from the earlier export.
    reply({ ...exported, resolver: undefined });
    assert.equal(byId("resolver-out").value, "");
    assert.equal(byId("download-resolver").disabled, true);
  });
});
