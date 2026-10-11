// The JSX importer's stack budget, as crates/weft-web's from_jsx module states it: native
// targets give the reader a 64 MiB thread, and on wasm32 the reader runs on the module's own
// link-time stack — 1 MiB, the wasm32-unknown-unknown default no host can raise per call. The
// scanner's nesting estimate is what fits that stack, so the boundary is a W602 refusal before
// parsing, never a stack trap. These suites run under both engines (`WEFT_ENGINE=wasm` and
// `WEFT_ENGINE=native`, see moon.yml); under wasm the calls below run on that 1 MiB stack, and
// a trap would fail the test instead of returning a diagnostic. The runtime legs run this file
// too; only the linked-stack check reads the module bytes and only where a file system exists.
import assert from "node:assert/strict";
import { describe, test } from "vitest";
import { wasm } from "../src/web.ts";
import { catalog } from "./catalog.ts";

type Wire = {
  document?: unknown;
  losses: unknown[];
  diagnostics: { code: string; message: string }[];
};

const handle = new wasm.Catalog(JSON.stringify(catalog));

function importJsx(source: string): Wire {
  return JSON.parse(handle.importJsx(source, false, undefined)) as Wire;
}

/** The refusal the scanner's estimate gives, before any parsing. */
function nestsDeeper(result: Wire): boolean {
  return result.diagnostics.some((d) => d.code === "W602" && d.message.includes("nests deeper"));
}

/** The first depth the estimate refuses, by binary search on the refusal itself. */
function firstRefused(shape: (n: number) => string): number {
  assert.ok(!nestsDeeper(importJsx(shape(0))), "a shallow component reads");
  let hi = 1;
  let lo = 0;
  while (hi < 4_096 && !nestsDeeper(importJsx(shape(hi)))) {
    lo = hi;
    hi *= 2;
  }
  assert.ok(hi < 4_096, "the nesting estimate refuses something");
  while (lo + 1 < hi) {
    const mid = (lo + hi) >> 1;
    if (nestsDeeper(importJsx(shape(mid)))) hi = mid;
    else lo = mid;
  }
  return hi;
}

/** Nesting shapes whose levels the scanner's estimate counts, one kind per shape. */
const shapes: Record<string, { shape: (n: number) => string; floor: number }> = {
  // An element per level: parse, lower and the DOM reader's recursion.
  elements: {
    shape: (n) => `export default function(){return ${"<a>".repeat(n)}x${"</a>".repeat(n)}}`,
    floor: 250,
  },
  // A parenthesis and an arrow per level: parse and the evaluator's recursion.
  arrows: {
    shape: (n) => `export default function(){return <a>{${"(x=>".repeat(n)}1${")".repeat(n)}}</a>}`,
    floor: 100,
  },
};

/** The module bytes; the browser leg has no file system and skips the check below. */
async function wasmBytes(): Promise<Uint8Array | undefined> {
  try {
    const { readFileSync } = await import("node:fs");
    return readFileSync(new URL("../wasm-web/weft_bg.wasm", import.meta.url));
  } catch {
    return undefined;
  }
}

/** wasm-ld makes `__stack_pointer` a mutable i32 global; its initial value is the stack. */
function linkedStack(bytes: Uint8Array): number {
  let i = 8;
  const leb = (): number => {
    let value = 0;
    let shift = 0;
    for (;;) {
      const byte = bytes[i++] ?? 0;
      value |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return value;
      shift += 7;
    }
  };
  while (i < bytes.length) {
    const section = bytes[i++] ?? 0;
    const size = leb();
    if (section === 6) {
      const count = leb();
      for (let g = 0; g < count; g++) {
        const type = bytes[i++];
        const mutable = bytes[i++];
        let value: number | undefined;
        if (bytes[i] === 0x41) {
          i++;
          value = leb();
        }
        while (bytes[i++] !== 0x0b) {}
        if (type === 0x7f && mutable === 1 && value !== undefined) return value;
      }
      assert.fail("the module has no mutable i32 global");
    }
    i += size;
  }
  assert.fail("the module has no global section");
}

describe("the JSX reader's stack budget", () => {
  test("the WebAssembly module links with the 1 MiB stack the estimate is cut for", async (ctx) => {
    const bytes = await wasmBytes();
    if (bytes === undefined) return ctx.skip();
    assert.equal(linkedStack(bytes), 1 << 20);
  });

  for (const [name, { shape, floor }] of Object.entries(shapes)) {
    test(`${name}: one level past the estimate is W602, one within it reads`, () => {
      const boundary = firstRefused(shape);
      assert.ok(
        boundary > floor,
        `the estimate admits at least ${floor} levels of ${name}, not ${boundary}`,
      );
      // Within the budget: whatever else the reader says, it says it — no trap.
      const within = importJsx(shape(boundary - 1));
      assert.ok(!nestsDeeper(within), "the boundary's neighbour is not refused");
      // One past: the stated diagnostic, on whatever stack this engine has.
      const past = importJsx(shape(boundary));
      assert.ok(nestsDeeper(past));
    });
  }
});
