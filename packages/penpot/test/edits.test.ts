// A designer's edits in Penpot come back as the expected Weft change. Each case edits the login
// screen in the fake file the way Penpot's UI would, and states the change as Weft patches
// (SPEC §7) applied to the original, plus the loss kinds the read-back must report. The cases
// mirror the Figma ones of @weft/figma, with Penpot's own operations.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { applyPatches, serialize, type Patch } from "@weft/core";
import type { LossKind } from "@weft/from-aria";
import { describe, test } from "vitest";
import {
  FakeBoard,
  FakeComponent,
  FakeGroup,
  FakeOther,
  FakePenpot,
  type FakeShape,
  type FakeText,
} from "./fake-penpot.ts";
import { ensureLibrary, isRawText, readLayers } from "../src/index.ts";
import { built, corpusMarkup, parseStrict, read, tokens } from "./helpers.ts";

const login = corpusMarkup("login");

type Edit = {
  name: string;
  edit: (penpot: FakePenpot, root: FakeBoard) => void | Promise<void>;
  patches: Patch[];
  losses?: LossKind[];
};

const text = (penpot: FakePenpot, root: FakeBoard, layer: string, name = "text"): FakeText =>
  penpot.find<FakeText>(penpot.find(root, layer), name);

function libraryComponent(penpot: FakePenpot, name: string): FakeComponent {
  const found = penpot.library.local.components.find((c) => c.name === name);
  if (found === undefined) throw new Error(`no component ${name}`);
  return found;
}

const newText = (penpot: FakePenpot, characters: string, name: string): FakeText => {
  const made = penpot.createText(characters);
  if (made === null) throw new Error("no text");
  made.name = name;
  return made;
};

const cases: Edit[] = [
  {
    name: "text of a button",
    edit: (p, root) => {
      text(p, root, "button#submit").characters = "Log in";
    },
    patches: [{ op: "set", id: "submit", prop: "text", value: "Log in" }],
  },
  {
    name: "a binding typed as text",
    edit: (p, root) => {
      text(p, root, "button#submit").characters = "{$.cta}";
    },
    patches: [{ op: "set", id: "submit", prop: "text", value: { bind: "$.cta" } }],
  },
  {
    name: "an escaped brace typed as text",
    edit: (p, root) => {
      text(p, root, "button#submit").characters = "{{cta}";
    },
    patches: [{ op: "set", id: "submit", prop: "text", value: "{cta}" }],
  },
  {
    name: "label of a field",
    edit: (p, root) => {
      text(p, root, "field#email", "label").characters = "E-mail";
    },
    patches: [{ op: "set", id: "email", prop: "label", value: "E-mail" }],
  },
  {
    name: "order of two links",
    edit: (p, root) => {
      p.find<FakeBoard>(root, "slot:footer").moveInFlow(p.find(root, "link#signup"), 0);
    },
    patches: [{ op: "move", id: "signup", parent: "form", slot: "footer", index: 0 }],
  },
  {
    name: "a removed heading",
    edit: (p, root) => p.find(root, "heading#title").remove(),
    patches: [{ op: "remove", id: "title" }],
  },
  {
    name: "an added library copy",
    edit: (p, root) => {
      const button = libraryComponent(p, "variant=primary, state=(unset)").instance();
      p.find<FakeText>(button, "text").characters = "Help";
      p.find<FakeBoard>(root, "slot:footer").appendChild(button);
    },
    patches: [
      {
        op: "insert",
        parent: "form",
        slot: "footer",
        markup: '<button id="button-help" variant="primary">Help</button>',
      },
    ],
  },
  {
    name: "a variant and a state",
    edit: (p, root) => {
      const button = p.find<FakeBoard>(root, "button#submit");
      button.switchVariant(0, "danger");
      button.switchVariant(1, "busy");
    },
    patches: [
      { op: "set", id: "submit", prop: "variant", value: "danger" },
      { op: "set", id: "submit", prop: "state", value: "busy" },
    ],
  },
  {
    name: "a gap token applied by the designer",
    edit: (p, root) => {
      p.find<FakeBoard>(root, "stack#fields").applyToken(p.token("space.lg"), ["rowGap"]);
    },
    patches: [{ op: "set", id: "fields", prop: "gap", value: { token: "space.lg" } }],
  },
  {
    name: "a gap typed as a value that matches a token",
    edit: (p, root) => {
      const flex = p.find<FakeBoard>(root, "stack#fields").flex;
      if (flex !== undefined) flex.rowGap = 8;
    },
    patches: [{ op: "set", id: "fields", prop: "gap", value: { token: "space.sm" } }],
  },
  {
    name: "a gap that matches no token",
    edit: (p, root) => {
      const flex = p.find<FakeBoard>(root, "stack#fields").flex;
      if (flex !== undefined) flex.rowGap = 13;
    },
    patches: [],
    losses: ["tokens"],
  },
  {
    name: "stack direction and alignment",
    edit: (p, root) => {
      const flex = p.find<FakeBoard>(root, "stack#fields").flex;
      if (flex === undefined) return;
      flex.dir = "row";
      flex.alignItems = "center";
    },
    patches: [
      { op: "set", id: "fields", prop: "direction", value: "row" },
      { op: "set", id: "fields", prop: "align", value: "center" },
    ],
  },
  {
    name: "a reversed stack",
    edit: (p, root) => {
      const flex = p.find<FakeBoard>(root, "stack#fields").flex;
      if (flex !== undefined) flex.dir = "column-reverse";
    },
    patches: [],
    losses: ["layout"],
  },
  {
    name: "a hidden layer",
    edit: (p, root) => {
      p.find(root, "link#reset").hidden = true;
    },
    patches: [{ op: "set", id: "reset", prop: "hidden", value: true }],
  },
  {
    name: "a fill override with no Weft prop",
    edit: (p, root) => {
      p.find<FakeBoard>(root, "button#submit").fills = [{ fillColor: "#008000", fillOpacity: 1 }];
    },
    patches: [],
    losses: ["tokens"],
  },
  {
    name: "a duplicated layer",
    edit: (p, root) => {
      p.find<FakeBoard>(root, "slot:footer").appendChild(p.find(root, "link#reset").clone());
    },
    patches: [
      {
        op: "insert",
        parent: "form",
        slot: "footer",
        markup: '<link id="link-reset" on-press="nav.reset">Forgot password?</link>',
      },
    ],
    losses: ["ids"],
  },
  {
    name: "foreign layers",
    edit: (p, root) => {
      const form = p.find<FakeBoard>(root, "form#form");
      const box = p.createBoard();
      box.name = "Promo";
      box.addFlexLayout().dir = "row";
      box.applyToken(p.token("space.sm"), ["columnGap"]);
      box.fills = [];
      box.appendChild(newText(p, "Terms apply", "note"));
      form.insertChild(1, box);
      const logo = p.createRectangle();
      logo.name = "Logo";
      logo.fills = [{ fillImage: { id: "image" } }];
      form.insertChild(0, logo);
      form.appendChild(new FakeOther(p, "path"));
    },
    patches: [
      {
        op: "insert",
        parent: "form",
        index: 0,
        markup: '<image id="image-logo" label="Logo" src=""/>',
      },
      {
        op: "insert",
        parent: "form",
        index: 2,
        markup:
          '<stack id="stack-promo" direction="row" gap="{token.space.sm}"><text id="text-terms-apply">Terms apply</text></stack>',
      },
    ],
    losses: ["ids", "values", "values", "ids", "ids", "kinds"],
  },
  {
    name: "a group placed by hand",
    edit: (p, root) => {
      const lines = ["Second", "First"].map((characters, i) => {
        const t = newText(p, characters, characters);
        t.y = 40 - i * 40;
        return t;
      });
      const group = new FakeGroup(p, "group", lines);
      group.name = "Notes";
      p.find<FakeBoard>(root, "form#form").appendChild(group as unknown as FakeShape);
    },
    patches: [
      {
        op: "insert",
        parent: "form",
        index: 3,
        markup:
          '<stack id="stack-notes"><text id="text-first">First</text><text id="text-second">Second</text></stack>',
      },
    ],
    losses: ["ids", "layout", "ids", "ids"],
  },
  {
    name: "a detached library button",
    edit: (p, root) => {
      const button = libraryComponent(p, "variant=danger, state=(unset)").instance();
      p.find<FakeText>(button, "text").characters = "Delete";
      button.detach();
      button.name = "button";
      p.find<FakeBoard>(root, "slot:footer").appendChild(button);
    },
    patches: [
      {
        op: "insert",
        parent: "form",
        slot: "footer",
        markup: '<button id="button-delete">Delete</button>',
      },
    ],
    losses: ["ids", "props"],
  },
];

for (const c of cases) {
  describe(`designer edit: ${c.name}`, () => {
    test("comes back as the expected Weft change", async () => {
      const { penpot, root } = await built(login);
      await c.edit(penpot, root);
      const result = await read(root);
      const expected = applyPatches(parseStrict(login), c.patches, { catalog: coreCatalog });
      assert.ok(expected.document, JSON.stringify(expected.diagnostics));
      assert.equal(serialize(result.document), serialize(expected.document));
      assert.deepEqual(
        result.losses.map((l) => l.kind).sort(),
        [...(c.losses ?? [])].sort(),
        JSON.stringify(result.losses),
      );
      assert.deepEqual(result.diagnostics, []);
    });
  });
}

describe("a glass surface (T51)", () => {
  const glass = corpusMarkup("glass");

  test("comes back unchanged, still taking its material token", async () => {
    const { penpot, root } = await built(glass);
    const result = await read(root);
    assert.equal(serialize(result.document), serialize(parseStrict(glass)));
    assert.deepEqual(result.losses, []);
  });

  test("a blur a designer changed is reported, and the prop stays", async () => {
    const { penpot, root } = await built(glass);
    penpot.find<FakeBoard>(root, "stack#card").backgroundBlur = { value: 4, hidden: false };
    const result = await read(root);
    assert.equal(serialize(result.document), serialize(parseStrict(glass)));
    assert.deepEqual(
      result.losses.map((l) => l.kind),
      ["tokens"],
    );
  });
});

describe("the built screen", () => {
  test("shows the children in Weft order on the canvas", async () => {
    const { penpot, root } = await built(login);
    const names = (name: string) =>
      penpot
        .find<FakeBoard>(root, name)
        .flow()
        .map((s) => s.name);
    assert.deepEqual(names("slot:footer"), ["link#reset", "link#signup"]);
    assert.deepEqual(names("form#form"), [
      "heading#title",
      "stack#fields",
      "button#submit",
      "slot:footer",
    ]);
  });
});

describe("a component copy", () => {
  test("keeps its structure, as Penpot does", async () => {
    const { penpot, root } = await built(login);
    const button = penpot.find<FakeBoard>(root, "button#submit");
    assert.throws(() => button.appendChild(penpot.createBoard()), /structure of a component copy/);
  });
});

describe("the library", () => {
  test("is reused when it is built again", async () => {
    const { penpot } = await built(login);
    const library = penpot.pages.filter((p) => p.name === "Weft library");
    assert.equal(library.length, 1);
    const board = library[0]?.root.children[0] as FakeBoard;
    const kinds = board.children.length;
    const components = penpot.library.local.components.length;
    const set = penpot.library.local.tokens.sets[0];
    const count = set?.tokens.length;
    await ensureLibrary(penpot, coreCatalog, tokens);
    assert.equal(penpot.pages.filter((p) => p.name === "Weft library").length, 1);
    assert.equal(board.children.length, kinds);
    assert.equal(penpot.library.local.components.length, components);
    assert.equal(penpot.library.local.tokens.sets.length, 1);
    assert.equal(set?.tokens.length, count);
  });

  test("holds one component per variant, with the variant values", async () => {
    const penpot = new FakePenpot();
    const library = await ensureLibrary(penpot, coreCatalog, tokens);
    const button = library.kinds.get("button");
    assert.ok(button !== undefined);
    const danger = button.variants.get("variant=danger, state=busy");
    assert.ok(danger?.isVariant());
    assert.deepEqual(danger.variantProps, { variant: "danger", state: "busy" });
  });

  test("updates a token whose value changed and leaves the page the designer was on", async () => {
    const penpot = new FakePenpot();
    const page = penpot.currentPage;
    await ensureLibrary(penpot, coreCatalog, tokens);
    assert.equal(penpot.currentPage, page);
    penpot.token("space.md").value = "99";
    await ensureLibrary(penpot, coreCatalog, tokens);
    assert.equal(penpot.token("space.md").value, "16");
    assert.equal(penpot.token("space.md").type, "spacing");
    assert.equal(penpot.currentPage, page);
  });
});

describe("a grid", () => {
  const markup = `<screen weft="0.1" id="s">
  <grid id="cards" columns="2" gap="{token.space.md}">
    <text id="a">One</text>
    <text id="b">Two</text>
    <text id="c">Three</text>
  </grid>
</screen>
`;

  test("comes back byte-identical", async () => {
    const { root } = await built(markup);
    const result = await read(root);
    assert.deepEqual(result.losses, []);
    assert.equal(serialize(result.document), serialize(parseStrict(markup)));
  });

  test("takes a column the designer added", async () => {
    const { penpot, root } = await built(markup);
    penpot.find<FakeBoard>(root, "grid#cards").grid?.addColumn("flex", 1);
    const result = await read(root);
    const expected = applyPatches(
      parseStrict(markup),
      [{ op: "set", id: "cards", prop: "columns", value: 3 }],
      { catalog: coreCatalog },
    );
    assert.ok(expected.document);
    assert.equal(serialize(result.document), serialize(expected.document));
  });
});

describe("text a designer typed, before the core reads it", () => {
  test("stays raw in what the sandbox returns", async () => {
    const { penpot, root } = await built(login);
    text(penpot, root, "button#submit").characters = "{$.cta}";
    const { document } = await readLayers(root, { catalog: coreCatalog, tokens });
    const form = document.root.children?.[0];
    const submit =
      typeof form === "object"
        ? form.children?.find((c) => typeof c === "object" && c.id === "submit")
        : undefined;
    const typed = typeof submit === "object" ? submit.props?.["text"] : undefined;
    assert.ok(isRawText(typed));
    assert.equal(typed.raw, "{$.cta}");
  });
});
