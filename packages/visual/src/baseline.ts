// Reviewed baseline screenshots, one folder per platform: font rendering differs between
// operating systems, so a baseline is only meaningful on the platform it was taken on.
// `WEFT_UPDATE_SCREENSHOTS=1` rewrites the baselines; review the images before committing them.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { compare, type CompareOptions, type Comparison } from "./compare.ts";

export const PLATFORM = `${process.platform}-${process.arch}`;
export const BASELINES = fileURLToPath(new URL(`../baselines/${PLATFORM}/`, import.meta.url));

export type BaselineResult =
  | { status: "match"; comparison: Comparison }
  | { status: "differ"; comparison: Comparison }
  | { status: "written"; path: string }
  /** No baseline for this name on this platform; `reviewed` says whether the platform has any. */
  | { status: "missing"; platform: string; reviewed: boolean };

const updating = () => process.env["WEFT_UPDATE_SCREENSHOTS"] === "1";

export type MatchOptions = CompareOptions & {
  /** Never rewrite the baseline, even while updating: for a deliberately changed screen. */
  readOnly?: boolean;
  /** Names the diff images instead of `name`. */
  label?: string;
};

/** `name` is a path below the platform folder without the extension, e.g. `web/login`. */
export function matchBaseline(
  png: Uint8Array,
  name: string,
  { readOnly = false, label = name, ...ignore }: MatchOptions = {},
): BaselineResult {
  const path = join(BASELINES, `${name}.png`);
  if (updating() && !readOnly) {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, png);
    return { status: "written", path };
  }
  if (!existsSync(path)) {
    return { status: "missing", platform: PLATFORM, reviewed: existsSync(BASELINES) };
  }
  const comparison = compare(readFileSync(path), png, `${PLATFORM}/${label}`, ignore);
  return {
    status: comparison.differing <= (ignore.tolerance ?? 0) ? "match" : "differ",
    comparison,
  };
}
