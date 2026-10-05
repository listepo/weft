// The comparisons must be sharp enough to matter: a screen nudged by one pixel, or with one word
// changed for another of the same length, has to fail both against its reviewed baseline and
// against a pristine render of itself. If these pass while the screen tests do too, the screen
// tests are known to catch changes this small.
import { beforeEach, expect, test } from "vitest";
import { commands } from "vitest/browser";
import type { ComponentType } from "react";
import { components, screens } from "virtual:weft-screens";
import { refit, showReact } from "./stage.ts";

type Props = { data?: unknown };

const NAME = "login";
const login = screens.find((s) => s.name === NAME)!;

const body = (id: string) => {
  const doc = (document.getElementById(id) as HTMLIFrameElement).contentDocument;
  if (doc === null) throw new Error(`#${id} has no document`);
  return doc.body;
};

beforeEach(async () => {
  const component = (await components[NAME]!.react()).default as ComponentType<Props>;
  await showReact("pristine", component, { data: login.data });
  await showReact("changed", component, { data: login.data });
});

/** Changes the `#changed` frame, then expects every comparison to see it. */
async function expectCaught(label: string, change: (body: HTMLElement) => void) {
  // The unchanged render is the baseline; anything else would make the rest of this meaningless.
  const before = await commands.matchScreenshot("#changed", `web/${NAME}`, { readOnly: true });
  expect(["match", "missing"]).toContain(before.status);
  change(body("changed"));
  await refit("changed");

  const across = await commands.compareElements("#pristine", "#changed", `mutation/${label}`);
  expect(across.differing).toBeGreaterThan(0);

  const after = await commands.matchScreenshot("#changed", `web/${NAME}`, {
    readOnly: true,
    label: `mutation/${label}.baseline`,
  });
  // Without reviewed baselines for this platform only the comparison across frames can run.
  if (after.status !== "missing") expect(after.status).toBe("differ");
}

test("a one-pixel nudge of one element is caught", async () => {
  await expectCaught("nudge", (b) => {
    const button = b.querySelector("button")!;
    button.style.position = "relative";
    button.style.left = "1px";
  });
});

test("a one-pixel taller gap is caught", async () => {
  await expectCaught("gap", (b) => {
    b.querySelector("button")!.style.marginTop = "1px";
  });
});

test("one word changed for another of the same length is caught", async () => {
  await expectCaught("word", (b) => {
    const walker = b.ownerDocument.createTreeWalker(b, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      if (node.nodeValue === "Forgot password?") {
        node.nodeValue = "Forgot passcode?";
        return;
      }
    }
    throw new Error('no "Forgot password?" text to change');
  });
});
