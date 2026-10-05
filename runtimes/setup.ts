// Fails the run when the tests did not land on the runtime the leg asked for: a launcher that fell
// back to Node would otherwise report a green Deno or Bun leg that never ran there.
import { inject } from "vitest";

const g = globalThis as Record<string, unknown>;
const actual = "Deno" in g ? "deno" : "Bun" in g ? "bun" : "document" in g ? "browser" : "node";
const expected = inject("runtime");

if (actual !== expected) {
  throw new Error(`the ${expected} leg ran its tests on ${actual}`);
}

declare module "vitest" {
  interface ProvidedContext {
    runtime: string;
  }
}
