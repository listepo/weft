// Every corpus screen, rendered in Chromium by each web target and compared three ways: against
// its reviewed baseline, across targets, and across round trips. A failed comparison names the
// diff image it wrote under `packages/visual/diffs/`.
import { beforeAll, describe, expect, test } from "vitest";
import { commands } from "vitest/browser";
import type { ComponentType } from "react";
import { components, screens } from "virtual:weft-screens";
import {
  BACKDROP,
  expectBaseline,
  expectSameLook,
  refit,
  showMarkup,
  showPage,
  showLit,
  showReact,
  showSolid,
} from "./stage.ts";

type Props = { data?: unknown };

// Known gap: an inline fragment is a component of its own in generated JSX, and the convention
// import drops it with its call sites as a `kinds` loss, since an importer never produces a `<use>`
// (SPEC §10.7). Pinned with `test.fails` so a fix shows up as a failure here.
const JSX_FRAGMENT_GAP = new Set(["receipt"]);

// Drawn by the web generators. The HTML and JSX importers read the forms back in T16.8, so a
// round trip through either drops justify, grow, padding, max-width and min-column-width.
const LAYOUT_IMPORT_GAP = new Set(["dashboard", "glass", "layout", "receipt"]);

for (const screen of screens) {
  const { name, data } = screen;
  const load = async (variant: "react" | "solid" | "react-back" | "solid-back") =>
    (await components[name]![variant]()).default;

  describe(name, () => {
    beforeAll(async () => {
      // What a host adds beside the components: the static page carries its tokens in its own
      // stylesheet, so a component without `weft-tokens.css` would lose every token it reads.
      const backdrop = name === "glass" ? BACKDROP : "";
      const head = `<style>${screen.css}</style>${backdrop}`;
      const props = { data };
      await showReact("react", (await load("react")) as ComponentType<Props>, props, head);
      await showSolid("solid", (await load("solid")) as (p: Props) => unknown, props, head);
      await showLit("lit", name, props, head);
      await showMarkup("reference", screen.reference, head);
      await showPage("html", screen.html, backdrop);
      await showPage("html-back", screen.back.html, backdrop);
      await showReact(
        "react-back",
        (await load("react-back")) as ComponentType<Props>,
        props,
        head,
      );
      await showSolid(
        "solid-back",
        (await load("solid-back")) as (p: Props) => unknown,
        props,
        head,
      );
      await showMarkup("figma-back", screen.back.figma, head);
      await showMarkup("penpot-back", screen.back.penpot, head);
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
    test("Lit looks like React", async () => {
      await expectSameLook("#react", "#lit", `web/${name}.lit`);
    });
    test("the reference renderer looks like React", async () => {
      await expectSameLook("#react", "#reference", `web/${name}.reference`);
    });
    test("the static page looks like React", async () => {
      await expectSameLook("#react", "#html", `web/${name}.html`);
    });
    test("the static page gives React's accessibility tree", async () => {
      expect(await commands.ariaSnapshot("#html")).toBe(await commands.ariaSnapshot("#react"));
    });
    test("React, SolidJS, Lit and the reference renderer give the same accessibility tree", async () => {
      const react = await commands.ariaSnapshot("#react");
      expect(await commands.ariaSnapshot("#solid")).toBe(react);
      expect(await commands.ariaSnapshot("#lit")).toBe(react);
      expect(await commands.ariaSnapshot("#reference")).toBe(react);
    });

    const htmlRoundTrip = LAYOUT_IMPORT_GAP.has(name) ? test.fails : test;
    htmlRoundTrip("round trip through the HTML importer looks the same", async () => {
      await expectSameLook("#html", "#html-back", `roundtrip/${name}.html`);
    });
    const jsxRoundTrip =
      JSX_FRAGMENT_GAP.has(name) || LAYOUT_IMPORT_GAP.has(name) ? test.fails : test;
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

    if (name === "dashboard") {
      // size.sm is 240px and the stats gap is space.lg (24px), so a 400px frame fits one
      // column and a 1000px frame fits the cap of three.
      test("the stats grid is one column when narrow and three when wide", async () => {
        const ids = ["#react", "#solid", "#lit", "#reference", "#html"];
        await atWidth(ids, 400);
        for (const id of ids) expect(columnCount(id), id).toBe(1);
        await atWidth(ids, 1000);
        for (const id of ids) expect(columnCount(id), id).toBe(3);
        await atWidth(ids, 480);
      });
    }
  });
}

/** How many columns the stats grid resolved to inside an iframe. */
function columnCount(selector: string): number {
  const frame = document.querySelector(selector);
  if (!(frame instanceof HTMLIFrameElement) || frame.contentDocument === null) {
    throw new Error(`no iframe ${selector}`);
  }
  const grid = frame.contentDocument.querySelector('[data-weft-id="stats"]');
  if (!(grid instanceof HTMLElement)) throw new Error(`${selector} has no stats grid`);
  const tracks = getComputedStyle(grid).gridTemplateColumns;
  return tracks.split(/\s+/).filter((part) => part !== "" && part !== "none").length;
}

/** Sets each iframe's width and waits until its height matches the reflowed content. */
async function atWidth(selectors: string[], px: number): Promise<void> {
  for (const selector of selectors) {
    const frame = document.querySelector(selector);
    if (!(frame instanceof HTMLIFrameElement)) throw new Error(`no iframe ${selector}`);
    frame.style.width = `${px}px`;
  }
  for (const selector of selectors) await refit(selector.slice(1));
}
