// Browser commands: the test page asks Node to screenshot an element of it, compare the image and
// read an accessibility snapshot, since only the Playwright side can do those.
import type { BrowserCommand } from "vitest/node";
import type {} from "@vitest/browser-playwright";
import { matchBaseline, type BaselineResult, type MatchOptions } from "./baseline.ts";
import { compare, type Comparison } from "./compare.ts";

const shoot = async (context: Parameters<BrowserCommand<[]>>[0], selector: string) =>
  new Uint8Array(await context.iframe.locator(selector).screenshot({ animations: "disabled" }));

/** Screenshots the element and compares it with the baseline `name` (e.g. `web/login`). */
const matchScreenshot: BrowserCommand<
  [selector: string, name: string, options?: MatchOptions]
> = async (context, selector, name, options): Promise<BaselineResult> =>
  matchBaseline(await shoot(context, selector), name, options);

/** Screenshots two elements and compares them; `label` names the diff image. */
const compareElements: BrowserCommand<[expected: string, actual: string, label: string]> = async (
  context,
  expected,
  actual,
  label,
): Promise<Comparison> =>
  compare(await shoot(context, expected), await shoot(context, actual), label);

/** Emulates `prefers-color-scheme` for the whole page, iframes included. */
const colorScheme: BrowserCommand<[scheme: "light" | "dark"]> = async (context, scheme) => {
  await context.page.emulateMedia({ colorScheme: scheme });
};

/** The accessibility snapshot of the document inside the iframe `selector`. */
const ariaSnapshot: BrowserCommand<[selector: string]> = async (context, selector) =>
  context.iframe.frameLocator(selector).locator("body").ariaSnapshot();

export const commands = { matchScreenshot, compareElements, ariaSnapshot, colorScheme };

declare module "vitest/browser" {
  interface BrowserCommands {
    matchScreenshot: (
      selector: string,
      name: string,
      options?: MatchOptions,
    ) => Promise<BaselineResult>;
    compareElements: (expected: string, actual: string, label: string) => Promise<Comparison>;
    ariaSnapshot: (selector: string) => Promise<string>;
    colorScheme: (scheme: "light" | "dark") => Promise<void>;
  }
}
