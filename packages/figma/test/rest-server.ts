// A fake of the Figma REST API's `GET /v1/files/:key/nodes` over a fake file: each node is written
// as the REST API documents it (https://developers.figma.com/docs/rest-api/file-node-types/,
// checked 2026-10-08), with fields at their documented default left out and rotation in radians,
// and served by a local HTTP server, so the pull runs its real `fetch` with no network.
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import type { FPaint } from "../src/api.ts";
import { FakeInstance, type FakeFigma, type FakeNode, type FakePage } from "./fake-figma.ts";

export const FILE_KEY = "AbCdEfGhIjKlMnOpQrStUv";
export const TOKEN = "figd_test-token-0123456789";

type Json = Record<string, unknown>;

const paints = (list: readonly FPaint[] | symbol): unknown[] =>
  typeof list === "symbol"
    ? []
    : list.map((p) =>
        p.type === "SOLID"
          ? {
              type: "SOLID",
              color: { ...p.color, a: 1 },
              ...(p.opacity === undefined || p.opacity === 1 ? {} : { opacity: p.opacity }),
              ...(p.boundVariables?.color === undefined
                ? {}
                : { boundVariables: { color: p.boundVariables.color } }),
            }
          : { type: p.type },
      );

/** Leaves out the members whose value is the REST default. */
function withoutDefaults(fields: Json, defaults: Json): Json {
  return Object.fromEntries(
    Object.entries(fields).filter(([k, v]) => JSON.stringify(v) !== JSON.stringify(defaults[k])),
  );
}

const LAYOUT_DEFAULTS: Json = {
  layoutMode: "NONE",
  counterAxisAlignItems: "MIN",
  itemSpacing: 0,
  paddingLeft: 0,
  paddingRight: 0,
  paddingTop: 0,
  paddingBottom: 0,
  gridRowGap: 0,
  fills: [],
  strokes: [],
  effects: [],
  rotation: 0,
  cornerRadius: 0,
};

/** The REST JSON of a fake node; shared plugin data when the request asked for it. */
export function restJson(node: FakeNode, shared: boolean): Json {
  const json: Json = {
    id: node.id,
    name: node.name,
    type: node.type,
    ...(node.visible ? {} : { visible: false }),
    absoluteBoundingBox: { x: node.x, y: node.y, width: node.width, height: node.height },
  };
  if (shared && node.shared.size > 0)
    json["sharedPluginData"] = Object.fromEntries(
      [...node.shared].map(([namespace, entries]) => [namespace, Object.fromEntries(entries)]),
    );
  if (node.type === "TEXT") {
    json["characters"] = node.characters;
    json["fills"] = paints(node.fills);
  } else if ("layoutMode" in node) {
    Object.assign(
      json,
      withoutDefaults(
        {
          layoutMode: node.layoutMode,
          layoutWrap: node.layoutWrap,
          counterAxisAlignItems: node.counterAxisAlignItems,
          itemSpacing: node.itemSpacing,
          paddingLeft: node.paddingLeft,
          paddingRight: node.paddingRight,
          paddingTop: node.paddingTop,
          paddingBottom: node.paddingBottom,
          ...(node.layoutMode === "GRID"
            ? { gridColumnCount: node.gridColumnCount, gridRowGap: node.gridRowGap }
            : {}),
          fills: paints(node.fills),
          strokes: paints(node.strokes),
          effects: node.effects,
          rotation: (node.rotation * Math.PI) / 180,
          cornerRadius: node.cornerRadius,
        },
        LAYOUT_DEFAULTS,
      ),
    );
    if (Object.keys(node.boundVariables).length > 0) json["boundVariables"] = node.boundVariables;
    if (node instanceof FakeInstance) {
      json["componentId"] = node.main.id;
      json["componentProperties"] = node.componentProperties;
    }
  } else if ("fills" in node) {
    json["fills"] = paints(node.fills);
  }
  if ("children" in node && node.children !== undefined)
    json["children"] = node.children.map((c) => restJson(c, shared));
  return json;
}

/** Every node of the fake file by id. */
function index(figma: FakeFigma): Map<string, FakeNode> {
  const out = new Map<string, FakeNode>();
  const walk = (n: FakeNode | FakePage) => {
    if (n.type !== "PAGE") out.set(n.id, n);
    for (const c of ("children" in n ? n.children : undefined) ?? []) walk(c);
  };
  for (const page of figma.root.children) walk(page);
  return out;
}

export type FakeRest = {
  api: string;
  requests: URL[];
  /** Replies with this status and body instead, while set. */
  fail?: { status: number; body: string; headers?: Record<string, string> } | undefined;
  close(): Promise<void>;
};

/** Serves the fake file; a request without the right token gets a 403, as the real API does. */
export async function serveFile(figma: FakeFigma): Promise<FakeRest> {
  const nodes = index(figma);
  const state: Omit<FakeRest, "api" | "close"> = { requests: [] };
  const server: Server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://localhost");
    state.requests.push(url);
    const reply = (status: number, body: string, headers: Record<string, string> = {}) => {
      response.writeHead(status, { "content-type": "application/json", ...headers });
      response.end(body);
    };
    if (state.fail !== undefined)
      return reply(state.fail.status, state.fail.body, state.fail.headers);
    if (request.headers["x-figma-token"] !== TOKEN)
      return reply(403, JSON.stringify({ status: 403, err: "Invalid token" }));
    if (url.pathname !== `/v1/files/${FILE_KEY}/nodes`)
      return reply(404, JSON.stringify({ status: 404, err: "Not found" }));
    const shared = (url.searchParams.get("plugin_data") ?? "").split(",").includes("shared");
    const entries = (url.searchParams.get("ids") ?? "").split(",").map((id) => {
      const node = nodes.get(id);
      return [id, node === undefined ? null : { document: restJson(node, shared) }];
    });
    reply(200, JSON.stringify({ name: "fake", nodes: Object.fromEntries(entries) }));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  return Object.assign(state, {
    api: `http://127.0.0.1:${port}`,
    close: () => new Promise<void>((resolve) => server.close(() => resolve())),
  });
}
