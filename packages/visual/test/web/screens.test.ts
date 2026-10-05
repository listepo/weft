// Every corpus screen, rendered in Chromium by each web target and compared three ways: against
// its reviewed baseline, across targets, and across round trips. A failed comparison names the
// diff image it wrote under `packages/visual/diffs/`.
import { beforeAll, describe, expect, test } from "vitest";
import { commands } from "vitest/browser";
import type { ComponentType } from "react";
import { components, screens } from "virtual:weft-screens";
import {
  expectBaseline,
  expectSameLook,
  showMarkup,
  showPage,
  showReact,
  showSolid,
} from "./stage.ts";

type Props = { data?: unknown };

// The static page shows the screen's data like the components do (`weft html --data`, T43), but
// it has a layout stylesheet of its own: field captions stack above their controls, a toggle's
// label is a flex row that pushes the next control onto a new line, and stacks are spaced with
// their gap tokens, while the generated components render with no stylesheet at all. So the two
// look the same only on a screen with no field, toggle or stacked layout. No pixel tolerance helps:
// a caption moved onto its own line shifts everything under it. With the data in, the accessibility
// trees are the comparison that means something, and they are compared below. Each listed screen
// must still differ, so one that starts matching fails here until it is removed.
const STATIC_PAGE_DIFFERS = new Set([
  "account",
  "dashboard",
  "data-table",
  "error-state",
  "inbox",
  "leaderboard",
  "login",
  "menu",
  "orders",
  "profile",
  "search-results",
  "settings",
  "signup",
  "tabs",
  "todo-list",
  "wizard-step",
]);

// Screens whose static page, filled with the same data, still exposes a different accessibility
// tree from React's. Seen in these tests while writing T43; none of the differences is in the data.
// The static page writes a `text` as a paragraph where React writes a text run (`P`), shows the
// caption of a field, checkbox, switch or radio as text of its own beside the control (`CAPTION`),
// and gives a link with no `href` the URL `#` (`HREF`). Each must still differ.
const P = "a text is a paragraph";
const CAPTION = "control captions are text of their own";
const HREF = "a link without href gets #";
const STATIC_TREE_DIFFERS: Record<string, string[]> = {
  account: [P, CAPTION],
  dashboard: [P],
  "data-table": [P],
  "error-state": [P, HREF],
  inbox: [CAPTION],
  leaderboard: [P],
  login: [CAPTION, HREF],
  menu: [P],
  orders: [P],
  profile: [P],
  "search-results": [P, CAPTION, HREF],
  settings: [CAPTION],
  signup: [CAPTION, HREF],
  tabs: [P],
  "todo-list": [P, CAPTION],
  "wizard-step": [P, CAPTION],
};

for (const screen of screens) {
  const { name, data } = screen;
  const load = async (variant: "react" | "solid" | "react-back" | "solid-back") =>
    (await components[name]![variant]()).default;

  describe(name, () => {
    beforeAll(async () => {
      await showReact("react", (await load("react")) as ComponentType<Props>, { data });
      await showSolid("solid", (await load("solid")) as (p: Props) => unknown, { data });
      await showMarkup("reference", screen.reference);
      await showPage("html", screen.html);
      await showPage("html-back", screen.back.html);
      await showReact("react-back", (await load("react-back")) as ComponentType<Props>, { data });
      await showSolid("solid-back", (await load("solid-back")) as (p: Props) => unknown, { data });
      await showMarkup("figma-back", screen.back.figma);
      await showMarkup("penpot-back", screen.back.penpot);
    });

    test("React matches its reviewed baseline", async ({ skip }) => {
      await expectBaseline("#react", `web/${name}`, skip);
    });
    test("the static page matches its reviewed baseline", async ({ skip }) => {
      await expectBaseline("#html", `html/${name}`, skip);
    });

    test("SolidJS looks like React", async () => {
      await expectSameLook("#react", "#solid", `web/${name}.solid`);
    });
    test("the reference renderer looks like React", async () => {
      await expectSameLook("#react", "#reference", `web/${name}.reference`);
    });
    (STATIC_PAGE_DIFFERS.has(name) ? test.fails : test)(
      "the static page looks like React",
      async () => {
        await expectSameLook("#react", "#html", `web/${name}.html`);
      },
    );
    (name in STATIC_TREE_DIFFERS ? test.fails : test)(
      "the static page gives React's accessibility tree",
      async () => {
        expect(await commands.ariaSnapshot("#html")).toBe(await commands.ariaSnapshot("#react"));
      },
    );
    test("React, SolidJS and the reference renderer give the same accessibility tree", async () => {
      const react = await commands.ariaSnapshot("#react");
      expect(await commands.ariaSnapshot("#solid")).toBe(react);
      expect(await commands.ariaSnapshot("#reference")).toBe(react);
    });

    test("round trip through the HTML importer looks the same", async () => {
      await expectSameLook("#html", "#html-back", `roundtrip/${name}.html`);
    });
    test("round trip through the React importer looks the same", async () => {
      await expectSameLook("#react", "#react-back", `roundtrip/${name}.react`);
    });
    test("round trip through the SolidJS importer looks the same", async () => {
      await expectSameLook("#solid", "#solid-back", `roundtrip/${name}.solid`);
    });
    test("round trip through Figma looks the same", async () => {
      await expectSameLook("#reference", "#figma-back", `roundtrip/${name}.figma`);
    });
    test("round trip through Penpot looks the same", async () => {
      await expectSameLook("#reference", "#penpot-back", `roundtrip/${name}.penpot`);
    });
  });
}
