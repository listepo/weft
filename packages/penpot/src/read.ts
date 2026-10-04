// Penpot to Weft: the shared read-back of @weft/design-tool over Penpot shapes (`layer.ts`).
import {
  finishRead,
  readLayers as readShared,
  type ReadOptions,
  type ReadResult,
} from "@weft/design-tool";
import type { PShape } from "./api.ts";
import { penpotLayers } from "./layer.ts";

export { isRawText, type RawText, type ReadOptions, type ReadResult } from "@weft/design-tool";

/**
 * Reads a board built by `buildScreen` (or any shape) back into a Weft document that is not yet
 * finished: typed text is still `RawText`, and nothing is canonical or validated. `finishRead`
 * does that with the WebAssembly core, which Penpot's plugin sandbox cannot run, so the plugin
 * finishes in its UI. `readScreen` does both where the core runs.
 */
export function readLayers(shape: PShape, options: ReadOptions): Promise<ReadResult> {
  return readShared(penpotLayers()(shape), options);
}

/** Reads a shape back into a canonical, validated Weft document. */
export async function readScreen(shape: PShape, options: ReadOptions): Promise<ReadResult> {
  return finishRead(await readLayers(shape, options), options);
}
