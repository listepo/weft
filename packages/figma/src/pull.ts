// `weft figma pull`: a frame of a Figma file, read through the REST API and back into Weft with the
// read-back the plugin uses. Kept out of the plugin bundle (`@weft/figma/pull`): the plugin's
// sandbox has no `fetch`. The access token is a secret: it goes only into the request header, and
// every message that leaves this module has it redacted.
import { finishRead, readLayers, type ReadOptions, type ReadResult } from "@weft/design-tool";
import { MAX_NODES } from "@weft/from-aria";
import { figmaLayers } from "./layer.ts";
import { NO_VARIABLES, restNode } from "./rest.ts";

export const FIGMA_API = "https://api.figma.com";

/** The plugin id of `plugins/figma/manifest.json`, whose plugin data holds the Weft source. */
export const PLUGIN_ID = "weft-development";

/** A frame's JSON is far below this; a larger response is refused before it is parsed. */
export const MAX_RESPONSE_BYTES = 50_000_000;

// Node ids per request for the main components, so the query stays a short URL.
const IDS_PER_REQUEST = 200;

const FILE_KEY = /^[0-9A-Za-z]{22,128}$/;
const NODE_ID = /^I?\d+[:-]\d+(?:;\d+[:-]\d+)*$/;

export class PullError extends Error {}

/** A file key and node id from a `figma.com` link, or from a bare file key and `--node`. */
export function parseTarget(
  target: string,
  node?: string,
): { fileKey: string; nodeId: string } | { error: string } {
  let fileKey = target;
  let nodeId = node;
  if (!FILE_KEY.test(target)) {
    let url: URL;
    try {
      url = new URL(target);
    } catch {
      return { error: `${target} is neither a figma.com link nor a file key` };
    }
    if (url.hostname !== "figma.com" && !url.hostname.endsWith(".figma.com"))
      return { error: `${url.hostname} is not figma.com` };
    const parts = url.pathname.split("/").filter((p) => p !== "");
    // A branch link names the branch's own key after `branch/`.
    fileKey = (parts[2] === "branch" ? parts[3] : parts[1]) ?? "";
    nodeId ??= url.searchParams.get("node-id") ?? undefined;
  }
  if (!FILE_KEY.test(fileKey)) return { error: `no file key in ${target}` };
  if (nodeId === undefined) return { error: "no node id: pass a link with node-id, or --node" };
  if (!NODE_ID.test(nodeId)) return { error: `${nodeId} is not a Figma node id` };
  return { fileKey, nodeId: nodeId.replaceAll("-", ":") };
}

export type PullOptions = ReadOptions & {
  fileKey: string;
  nodeId: string;
  /** A personal access token or OAuth token with the `file_content:read` scope. */
  token: string;
  pluginId?: string | undefined;
  fetch?: typeof fetch | undefined;
  /** Only tests point this elsewhere; the CLI always uses `FIGMA_API`. */
  api?: string | undefined;
  maxBytes?: number | undefined;
};

const redact = (text: string, token: string): string =>
  token === "" ? text : text.replaceAll(token, "[token]");

const STATUS: Record<number, string> = {
  403: "the token was refused: it is invalid or expired, or lacks the file_content:read scope",
  404: "the file was not found",
};

async function bodyOf(response: Response, maxBytes: number): Promise<string> {
  const chunks: Uint8Array[] = [];
  let size = 0;
  if (response.body !== null)
    for await (const chunk of response.body) {
      size += chunk.byteLength;
      if (size > maxBytes) throw new PullError(`the response is larger than ${maxBytes} bytes`);
      chunks.push(chunk);
    }
  return Buffer.concat(chunks).toString("utf8");
}

/** `GET /v1/files/:key/nodes` for these ids: the `nodes` map, by id. */
async function getNodes(
  options: PullOptions,
  ids: readonly string[],
  depth?: number,
): Promise<Record<string, unknown>> {
  const query = new URLSearchParams({
    ids: ids.join(","),
    plugin_data: options.pluginId ?? PLUGIN_ID,
  });
  if (depth !== undefined) query.set("depth", String(depth));
  const url = `${options.api ?? FIGMA_API}/v1/files/${encodeURIComponent(options.fileKey)}/nodes?${query}`;
  const response = await (options.fetch ?? fetch)(url, {
    headers: { "X-Figma-Token": options.token },
  });
  const text = await bodyOf(response, options.maxBytes ?? MAX_RESPONSE_BYTES);
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    json = undefined;
  }
  const body = typeof json === "object" && json !== null ? (json as Record<string, unknown>) : {};
  if (!response.ok) {
    // The server's own words are untrusted text for a terminal: no control characters, bounded.
    const err =
      typeof body["err"] === "string"
        ? `: ${body["err"].replace(/\p{Cc}/gu, " ").slice(0, 200)}`
        : "";
    const wait = Number(response.headers.get("retry-after") ?? Number.NaN);
    const reason =
      response.status === 429
        ? `rate limited${Number.isFinite(wait) ? `; retry after ${wait} s` : ""}`
        : (STATUS[response.status] ?? `HTTP ${response.status}`);
    throw new PullError(`${reason}${err}`);
  }
  const nodes = body["nodes"];
  if (typeof nodes !== "object" || nodes === null) throw new PullError("the response has no nodes");
  return nodes as Record<string, unknown>;
}

const documentOf = (entry: unknown): Record<string, unknown> | undefined => {
  const document = (entry as { document?: unknown } | null)?.document;
  return typeof document === "object" && document !== null && !Array.isArray(document)
    ? (document as Record<string, unknown>)
    : undefined;
};

/** The component ids of the instances below a node, in a bounded walk over the raw JSON. */
function componentIds(root: Record<string, unknown>): string[] {
  const ids = new Set<string>();
  const stack: unknown[] = [root];
  for (let seen = 0; stack.length > 0 && seen < MAX_NODES; seen++) {
    const node = stack.pop() as Record<string, unknown> | null;
    if (typeof node !== "object" || node === null) continue;
    if (node["type"] === "INSTANCE" && typeof node["componentId"] === "string")
      ids.add(node["componentId"]);
    if (Array.isArray(node["children"])) stack.push(...(node["children"] as unknown[]));
  }
  return [...ids].filter((id) => NODE_ID.test(id));
}

async function pullRead(options: PullOptions): Promise<ReadResult> {
  const nodes = await getNodes(options, [options.nodeId]);
  const root = documentOf(Object.hasOwn(nodes, options.nodeId) ? nodes[options.nodeId] : null);
  if (root === undefined) throw new PullError(`node ${options.nodeId} is not in the file`);
  // Instances need their main components: the catalog kind and the style live there.
  const components = new Map<string, Record<string, unknown>>();
  const ids = componentIds(root);
  for (let i = 0; i < ids.length; i += IDS_PER_REQUEST) {
    const found = await getNodes(options, ids.slice(i, i + IDS_PER_REQUEST), 1);
    for (const [id, entry] of Object.entries(found)) {
      const document = documentOf(entry);
      if (document !== undefined) components.set(id, document);
    }
  }
  const pluginId = options.pluginId ?? PLUGIN_ID;
  const layer = figmaLayers(NO_VARIABLES)(restNode(root, { pluginId, components }));
  return finishRead(await readLayers(layer, options), options);
}

/** Reads a frame through the REST API into a canonical, validated Weft document. */
export async function pullScreen(options: PullOptions): Promise<ReadResult> {
  try {
    return await pullRead(options);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new PullError(redact(message, options.token));
  }
}
