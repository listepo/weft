// The comparator's bottom strip: the part of a screenshot the system draws over is left out of the
// comparison, while a difference anywhere above it still counts.
import { encode } from "fast-png";
import { expect, test } from "vitest";
import { compare } from "../src/compare.ts";

const WIDTH = 4;
const HEIGHT = 10;

/** A white image with one black pixel at `row`. */
function image(row: number): Uint8Array {
  const data = new Uint8Array(WIDTH * HEIGHT * 4).fill(255);
  data.fill(0, row * WIDTH * 4, row * WIDTH * 4 + 3);
  return encode({ width: WIDTH, height: HEIGHT, data, channels: 4, depth: 8 });
}

test("a difference inside the ignored bottom rows does not count", () => {
  expect(compare(image(8), image(9), "unit/bottom").differing).toBe(2);
  expect(compare(image(8), image(9), "unit/bottom", { ignoreBottom: 3 }).differing).toBe(0);
});

test("a difference above the ignored rows still counts", () => {
  expect(compare(image(2), image(3), "unit/above", { ignoreBottom: 3 }).differing).toBe(2);
});
