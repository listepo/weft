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
  showMarkup,
  showPage,
  showLit,
  showReact,
  showSolid,
} from "./stage.ts";

type Props = { data?: unknown };

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
