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

// Known gaps the convention importers leave visible on screen, found by these tests (T36). Each
// listed round trip must still differ, so a fix fails here until its entry is removed.
const JSX_ROUND_TRIP_GAPS: Record<string, string> = {
  account: "the number field's value is written through `_float(...)`, which the importer drops",
  inbox: "a bound tab label comes back empty and the tabs' selected binding is lost",
  leaderboard: "an array-index binding is written as `_get(...)`, which the importer drops",
};

// The static page is a template for a host to fill: it shows no data, and its own layout
// stylesheet stacks field captions above full-width controls and spaces stacked children apart,
// while the generated components render the data with no stylesheet at all. So the two look the
// same only on a screen with no bound content and no stacked layout. Seen in the diff images
// while writing T36; the same screens still differ from a component given no data. Each listed
// screen must still differ, so one that starts matching fails here until it is removed.
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
    test("React, SolidJS and the reference renderer give the same accessibility tree", async () => {
      const react = await commands.ariaSnapshot("#react");
      expect(await commands.ariaSnapshot("#solid")).toBe(react);
      expect(await commands.ariaSnapshot("#reference")).toBe(react);
    });

    test("round trip through the HTML importer looks the same", async () => {
      await expectSameLook("#html", "#html-back", `roundtrip/${name}.html`);
    });
    const jsxRoundTrip = name in JSX_ROUND_TRIP_GAPS ? test.fails : test;
    jsxRoundTrip("round trip through the React importer looks the same", async () => {
      await expectSameLook("#react", "#react-back", `roundtrip/${name}.react`);
    });
    jsxRoundTrip("round trip through the SolidJS importer looks the same", async () => {
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
