// Figma to Weft: the shared read-back of @weft/design-tool over Figma layers (`layer.ts`).
import {
  finishRead,
  readLayers as readShared,
  type ReadOptions,
  type ReadResult,
} from "@weft/design-tool";
import type { FigmaApi, FNode } from "./api.ts";
import { figmaLayers } from "./layer.ts";

export { isRawText, type RawText, type ReadOptions, type ReadResult } from "@weft/design-tool";

/**
 * Reads a frame built by `buildScreen` (or any layer) back into a Weft document that is not yet
 * finished: typed text is still `RawText`, and nothing is canonical or validated. `finishRead`
 * does that with the WebAssembly core, which Figma's main thread cannot run, so the plugin
 * finishes in its UI. `readScreen` does both where the core runs.
 */
export function readLayers(api: FigmaApi, layer: FNode, options: ReadOptions): Promise<ReadResult> {
  return readShared(figmaLayers(api.variables)(layer), options);
}

/** Reads a layer back into a canonical, validated Weft document. */
export async function readScreen(
  api: FigmaApi,
  layer: FNode,
  options: ReadOptions,
): Promise<ReadResult> {
  return finishRead(await readLayers(api, layer, options), options);
}
