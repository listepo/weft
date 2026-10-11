// The dashboard stats grid caps itself at `columns` and drops to fewer tracks when its own box is
// narrower than `min-column-width` (SPEC §5.1). The suite's other shots are 480 px, which is the
// narrow case; this one also widens the same frame.
import { expect, test } from "vitest";
import { screens } from "virtual:weft-screens";
import { showMarkup, showPage } from "./stage.ts";

function tracks(id: string): number {
  const frame = document.getElementById(id);
  if (!(frame instanceof HTMLIFrameElement) || frame.contentDocument === null) {
    throw new Error(`#${id} is not a loaded iframe`);
  }
  const grid = frame.contentDocument.querySelector("[data-weft-id='stats']");
  if (!(grid instanceof HTMLElement)) throw new Error(`#${id} has no stats grid`);
  return getComputedStyle(grid)
    .gridTemplateColumns.split(" ")
    .filter((part) => part !== "").length;
}

async function width(id: string, px: number): Promise<void> {
  const frame = document.getElementById(id);
  if (!(frame instanceof HTMLIFrameElement)) throw new Error(`#${id} is not an iframe`);
  frame.style.width = `${px}px`;
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

test("the stats grid is one column when narrow and three when wide", async () => {
  const screen = screens.find((s) => s.name === "dashboard");
  if (screen === undefined) throw new Error("no dashboard screen");
  await showPage("stats-html", screen.html);
  await showMarkup("stats-ref", screen.reference, `<style>${screen.css}</style>`);
  await width("stats-html", 320);
  await width("stats-ref", 320);
  expect(tracks("stats-html")).toBe(1);
  expect(tracks("stats-ref")).toBe(1);
  await width("stats-html", 1200);
  await width("stats-ref", 1200);
  expect(tracks("stats-html")).toBe(3);
  expect(tracks("stats-ref")).toBe(3);
});
