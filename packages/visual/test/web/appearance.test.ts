// The example project's light and dark themes (T45) in Chromium, under each
// `prefers-color-scheme`: the static page follows it through its own stylesheet, the React
// component through the `weft css-tokens` stylesheet, and the reference renderer draws the theme
// it was asked for. Each is held to a reviewed baseline, and the reference renderer must look like
// React in both themes.
import { afterAll, beforeAll, describe, test } from "vitest";
import { commands } from "vitest/browser";
import type { ComponentType } from "react";
import { example } from "virtual:weft-appearance";
import component from "virtual:weft-appearance-component";
import { expectBaseline, expectSameLook, showPage, showReact } from "./stage.ts";

const NAME = "example-review";

// Back to Playwright's default, so the screens that follow are drawn as they always were.
afterAll(async () => {
  await commands.colorScheme("light");
});

for (const scheme of ["light", "dark"] as const) {
  const baseline = scheme === "light" ? NAME : `${NAME}-dark`;
  describe(scheme, () => {
    beforeAll(async () => {
      await commands.colorScheme(scheme);
      await showPage("html", example.html);
      // The stylesheet's comment cannot end the style element: it holds no `<`.
      await showReact(
        "react",
        component as ComponentType<{ data?: unknown }>,
        { data: example.data },
        `<style>${example.css}</style>`,
      );
      await showPage("reference", example.reference[scheme]);
    });

    test("the static page matches its reviewed baseline", async ({ skip }) => {
      await expectBaseline("#html", `html/${baseline}`, skip);
    });
    test("React with weft-tokens.css matches its reviewed baseline", async ({ skip }) => {
      await expectBaseline("#react", `web/${baseline}`, skip);
    });
    test("the reference renderer looks like React", async () => {
      await expectSameLook("#react", "#reference", `web/${baseline}.reference`);
    });
  });
}
