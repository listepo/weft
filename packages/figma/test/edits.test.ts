// A designer's edits in Figma come back as the expected Weft change. Each case edits the login
// screen in the fake file the way the Figma UI would, and states the change as Weft patches
// (SPEC §7) applied to the original, plus the loss kinds the read-back must report.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { applyPatches, serialize, type Patch } from "@weft/core";
import type { LossKind } from "@weft/from-aria";
import { describe, test } from "vitest";
import {
  FakeComponent,
  FakeFrame,
  FakeInstance,
  FakeOther,
  FakeRectangle,
  type FakeFigma,
  type FakeText,
} from "./fake-figma.ts";
import { built, corpusMarkup, parseStrict, read, tokens } from "./helpers.ts";

const login = corpusMarkup("login");

type Edit = {
  name: string;
  edit: (figma: FakeFigma, frame: FakeFrame) => void | Promise<void>;
  patches: Patch[];
  losses?: LossKind[];
};

const text = (figma: FakeFigma, frame: FakeFrame, layer: string, name = "text"): FakeText =>
  figma.find<FakeText>(figma.find(frame, layer), name);

function variable(figma: FakeFigma, path: string) {
  const found = figma.variables.all.find((v) => v.getPluginData("weft.token") === path);
  if (found === undefined) throw new Error(`no variable for ${path}`);
  return found;
}

function libraryComponent(figma: FakeFigma, name: string): FakeComponent {
  const page = figma.root.children.find((p) => p.name === "Weft library");
  if (page === undefined) throw new Error("no library page");
  return figma.find<FakeComponent>(page, name);
}

const cases: Edit[] = [
  {
    name: "text of a button",
    edit: (f, frame) => {
      text(f, frame, "button#submit").characters = "Log in";
    },
    patches: [{ op: "set", id: "submit", prop: "text", value: "Log in" }],
  },
  {
    name: "a binding typed as text",
    edit: (f, frame) => {
      text(f, frame, "button#submit").characters = "{$.cta}";
    },
    patches: [{ op: "set", id: "submit", prop: "text", value: { bind: "$.cta" } }],
  },
  {
    name: "label of a field",
    edit: (f, frame) => {
      text(f, frame, "field#email", "label").characters = "E-mail";
    },
    patches: [{ op: "set", id: "email", prop: "label", value: "E-mail" }],
  },
  {
    name: "order of two links",
    edit: (f, frame) => {
      const slot = f.find<FakeFrame>(frame, "slot:footer");
      slot.insertChild(0, f.find(frame, "link#signup"));
    },
    patches: [{ op: "move", id: "signup", parent: "form", slot: "footer", index: 0 }],
  },
  {
    name: "a removed heading",
    edit: (f, frame) => f.find(frame, "heading#title").remove(),
    patches: [{ op: "remove", id: "title" }],
  },
  {
    name: "an added library instance",
    edit: (f, frame) => {
      const button = libraryComponent(f, "variant=primary, state=(unset)").createInstance();
      f.find<FakeText>(button, "text").characters = "Help";
      f.find<FakeFrame>(frame, "slot:footer").appendChild(button);
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
    edit: (f, frame) => {
      f.find<FakeInstance>(frame, "button#submit").setProperties({
        variant: "danger",
        state: "busy",
      });
    },
    patches: [
      { op: "set", id: "submit", prop: "variant", value: "danger" },
      { op: "set", id: "submit", prop: "state", value: "busy" },
    ],
  },
  {
    name: "a gap bound to another token",
    edit: (f, frame) => {
      f.find<FakeFrame>(frame, "stack#fields").setBoundVariable(
        "itemSpacing",
        variable(f, "space.lg"),
      );
    },
    patches: [{ op: "set", id: "fields", prop: "gap", value: { token: "space.lg" } }],
  },
  {
    name: "a gap typed as a value that matches a token",
    edit: (f, frame) => {
      const stack = f.find<FakeFrame>(frame, "stack#fields");
      stack.setBoundVariable("itemSpacing", null);
      stack.itemSpacing = 8;
    },
    patches: [{ op: "set", id: "fields", prop: "gap", value: { token: "space.sm" } }],
  },
  {
    name: "a gap that matches no token",
    edit: (f, frame) => {
      const stack = f.find<FakeFrame>(frame, "stack#fields");
      stack.setBoundVariable("itemSpacing", null);
      stack.itemSpacing = 13;
    },
    patches: [],
    losses: ["tokens"],
  },
  {
    name: "stack direction and alignment",
    edit: (f, frame) => {
      const stack = f.find<FakeFrame>(frame, "stack#fields");
      stack.layoutMode = "HORIZONTAL";
      stack.counterAxisAlignItems = "CENTER";
    },
    patches: [
      { op: "set", id: "fields", prop: "direction", value: "row" },
      { op: "set", id: "fields", prop: "align", value: "center" },
    ],
  },
  {
    name: "a hidden layer",
    edit: (f, frame) => {
      f.find(frame, "link#reset").visible = false;
    },
    patches: [{ op: "set", id: "reset", prop: "hidden", value: true }],
  },
  {
    name: "a fill override with no Weft prop",
    edit: (f, frame) => {
      f.find<FakeInstance>(frame, "button#submit").fills = [
        { type: "SOLID", color: { r: 0, g: 0.5, b: 0 } },
      ];
    },
    patches: [],
    losses: ["tokens"],
  },
  {
    name: "a duplicated layer",
    edit: (f, frame) => {
      const slot = f.find<FakeFrame>(frame, "slot:footer");
      slot.appendChild(f.find<FakeInstance>(frame, "link#reset").clone());
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
    edit: (f, frame) => {
      const form = f.find<FakeFrame>(frame, "form#form");
      const note = f.createText();
      note.characters = "Terms apply";
      note.name = "note";
      const box = new FakeFrame(f);
      box.name = "Promo";
      box.layoutMode = "HORIZONTAL";
      box.setBoundVariable("itemSpacing", variable(f, "space.sm"));
      box.fills = [];
      box.appendChild(note);
      form.insertChild(1, box);
      const logo = new FakeRectangle();
      logo.name = "Logo";
      logo.fills = [{ type: "IMAGE" }];
      form.insertChild(0, logo);
      form.appendChild(new FakeOther("VECTOR"));
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
    edit: (f, frame) => {
      const lines = ["Second", "First"].map((characters, i) => {
        const t = f.createText();
        t.characters = characters;
        t.y = 40 - i * 40;
        return t;
      });
      const group = new FakeOther("GROUP", lines);
      group.name = "Notes";
      f.find<FakeFrame>(frame, "form#form").appendChild(group);
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
    edit: (f, frame) => {
      const button = libraryComponent(f, "variant=danger, state=(unset)").createInstance();
      f.find<FakeText>(button, "text").characters = "Delete";
      const detached = new FakeFrame(f);
      detached.name = "button";
      for (const child of button.children.slice()) detached.appendChild(child);
      f.find<FakeFrame>(frame, "slot:footer").appendChild(detached);
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
      const { figma, frame } = await built(login);
      await c.edit(figma, frame);
      const result = await read(figma, frame);
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

describe("a screen whose library was built twice", () => {
  test("reuses the components and variables", async () => {
    const { figma } = await built(login);
    const { ensureLibrary } = await import("../src/index.ts");
    const variables = figma.variables.all.length;
    const page = figma.root.children.find((p) => p.name === "Weft library");
    const board = page?.children[0] as FakeFrame;
    const sets = board.children.length;
    await ensureLibrary(figma, coreCatalog, tokens);
    assert.equal(figma.variables.all.length, variables);
    assert.equal(board.children.length, sets);
    assert.equal(figma.root.children.filter((p) => p.name === "Weft library").length, 1);
  });
});
