// The one comparator every screenshot test uses, web and SwiftUI: pixelmatch over decoded PNGs.
// A failed comparison writes the expected and actual images and a diff image next to each other
// under `packages/visual/diffs/`, so a failure can be looked at, not only counted.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { decode, encode } from "fast-png";
import pixelmatch from "pixelmatch";

export const DIFFS = fileURLToPath(new URL("../diffs/", import.meta.url));

export type Comparison = {
  width: number;
  height: number;
  /** Pixels that differ beyond pixelmatch's color threshold, anti-aliasing excluded. */
  differing: number;
  /** Where the diff image was written, when anything differs. */
  diff?: string;
};

type Rgba = { width: number; height: number; data: Uint8Array };

function rgba(png: Uint8Array): Rgba {
  const image = decode(png);
  const { width, height, channels } = image;
  // 16-bit samples keep their high byte; screenshots are 8-bit, so this only guards the decoder.
  const shift = image.depth === 16 ? 8 : 0;
  const data = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) {
    const at = (c: number) => (image.data[i * channels + c] ?? 0) >> shift;
    const gray = channels < 3;
    data[i * 4] = at(0);
    data[i * 4 + 1] = gray ? at(0) : at(1);
    data[i * 4 + 2] = gray ? at(0) : at(2);
    data[i * 4 + 3] = channels === 4 ? at(3) : channels === 2 ? at(1) : 255;
  }
  return { width, height, data };
}

// Images of different sizes are compared on the larger canvas, the missing area filled with a
// color no screen uses, so a one-pixel change in height counts as a difference instead of an error.
function pad(image: Rgba, width: number, height: number): Uint8Array {
  if (image.width === width && image.height === height) return image.data;
  const out = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) out.set([255, 0, 255, 255], i * 4);
  for (let y = 0; y < image.height; y++) {
    out.set(image.data.subarray(y * image.width * 4, (y + 1) * image.width * 4), y * width * 4);
  }
  return out;
}

/** Paints the top `top` and the bottom `bottom` pixel rows one color, so whatever the system drew there never counts. */
function blankEdges(data: Uint8Array, width: number, height: number, top: number, bottom: number) {
  const rows = (n: number) => Math.min(height, Math.max(0, Math.ceil(n)));
  data.fill(0, 0, rows(top) * width * 4);
  data.fill(0, (height - rows(bottom)) * width * 4, height * width * 4);
}

const png = (width: number, height: number, data: Uint8Array) =>
  encode({ width, height, data, channels: 4, depth: 8 });

export type CompareOptions = {
  /**
   * Pixel rows at the bottom of both images to leave out of the comparison. The iOS Simulator draws
   * the home indicator there in some screenshots and not in others, whatever the app asks for.
   */
  ignoreBottom?: number;
  /**
   * Pixel rows at the top of both images to leave out of the comparison. A simulator screenshot
   * sometimes includes the Dynamic Island and the screen's rounded corners, and sometimes not.
   */
  ignoreTop?: number;
  /**
   * How many differing pixels still count as a match: the diff images are then not written.
   * Zero by default; only a screen whose pixels the system itself draws nondeterministically
   * (the glass material) may ask for more.
   */
  tolerance?: number;
};

/**
 * Compares two PNG screenshots. `label` names the diff images (`diffs/<label>.diff.png` and the
 * two inputs beside it); nothing is written when the images match (within `tolerance`).
 */
export function compare(
  expected: Uint8Array,
  actual: Uint8Array,
  label: string,
  { ignoreBottom = 0, ignoreTop = 0, tolerance = 0 }: CompareOptions = {},
): Comparison {
  const a = rgba(expected);
  const b = rgba(actual);
  const width = Math.max(a.width, b.width);
  const height = Math.max(a.height, b.height);
  // Copies, since `pad` hands back an image's own data when the sizes already match.
  const left = Uint8Array.from(pad(a, width, height));
  const right = Uint8Array.from(pad(b, width, height));
  blankEdges(left, width, height, ignoreTop, ignoreBottom);
  blankEdges(right, width, height, ignoreTop, ignoreBottom);
  const out = new Uint8Array(width * height * 4);
  const differing = pixelmatch(left, right, out, width, height, { threshold: 0.1 });
  if (differing <= tolerance) return { width, height, differing };
  const diff = join(DIFFS, `${label}.diff.png`);
  mkdirSync(dirname(diff), { recursive: true });
  writeFileSync(diff, png(width, height, out));
  writeFileSync(join(DIFFS, `${label}.expected.png`), expected);
  writeFileSync(join(DIFFS, `${label}.actual.png`), actual);
  return { width, height, differing, diff };
}
