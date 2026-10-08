// `weft figma pull`: a frame built by the plugin, read back through the REST API. The file is the
// in-memory fake, served as REST JSON by a local HTTP server (`rest-server.ts`).
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { serialize } from "@weft/core";
import { afterEach, describe, test } from "vitest";
import { parseTarget, pullScreen, PullError, type PullOptions } from "../src/pull.ts";
import type { FakeFrame, FakeText } from "./fake-figma.ts";
import { built, corpusMarkup, corpusNames, tokens } from "./helpers.ts";
import { FILE_KEY, serveFile, TOKEN, type FakeRest } from "./rest-server.ts";

let open: FakeRest | undefined;
afterEach(async () => {
  await open?.close();
  open = undefined;
});

async function served(markup: string) {
  const { figma, frame } = await built(markup);
  open = await serveFile(figma);
  const options: PullOptions = {
    catalog: coreCatalog,
    tokens,
    fileKey: FILE_KEY,
    nodeId: frame.id,
    token: TOKEN,
    api: open.api,
  };
  return { figma, frame, server: open, options };
}

for (const name of corpusNames) {
  describe(`corpus ${name} pulled`, () => {
    test("comes back byte-identical", async () => {
      const markup = corpusMarkup(name);
      const { options } = await served(markup);
      const result = await pullScreen(options);
      assert.deepEqual(result.losses, []);
      assert.deepEqual(result.diagnostics, []);
      assert.equal(serialize(result.document), markup);
    });
  });
}

describe("the requests", () => {
  test("carry the token in X-Figma-Token and ask for shared plugin data, then the main components", async () => {
    const { server, options, frame } = await served(corpusMarkup("login"));
    await pullScreen(options);
    assert.equal(server.requests.length, 2);
    const [first, second] = server.requests as [URL, URL];
    assert.equal(first.pathname, `/v1/files/${FILE_KEY}/nodes`);
    assert.equal(first.searchParams.get("ids"), frame.id);
    assert.equal(first.searchParams.get("plugin_data"), "shared");
    assert.equal(second.searchParams.get("depth"), "1");
    // The token travels only in the header.
    assert.ok(!first.href.includes(TOKEN));
  });
});

describe("a designer's edit", () => {
  test("comes back as a Weft change", async () => {
    const { figma, frame, options } = await served(corpusMarkup("login"));
    figma.find<FakeText>(figma.find(frame, "button#submit"), "text").characters = "Log in";
    const result = await pullScreen(options);
    const markup = serialize(result.document);
    assert.ok(markup.includes(">Log in</button>"), markup);
    assert.deepEqual(result.losses, []);
  });

  test("a turned layer is a visual edit, read from REST radians", async () => {
    const markup = corpusMarkup("tilt");
    const { figma, frame, options } = await served(markup);
    figma.find<FakeFrame>(frame, "section#front").rotation = 30;
    const result = await pullScreen(options);
    assert.deepEqual(
      result.losses.map((l) => l.kind),
      ["tokens"],
    );
    assert.equal(serialize(result.document), markup);
  });
});

describe("shared plugin data", () => {
  test("is the only plugin data asked for, and without it the layers are foreign", async () => {
    const { figma, server, options } = await served(corpusMarkup("login"));
    for (const holder of figma.everyDataHolderFor()) holder.shared.clear();
    const result = await pullScreen(options);
    assert.equal(server.requests[0]?.searchParams.get("plugin_data"), "shared");
    assert.ok(result.losses.some((l) => l.kind === "ids"));
  });
});

describe("failures", () => {
  const failing = async (
    status: number,
    body: string,
    headers?: Record<string, string>,
  ): Promise<PullError> => {
    const { server, options } = await served(corpusMarkup("login"));
    server.fail = { status, body, ...(headers === undefined ? {} : { headers }) };
    const error = await pullScreen(options).then(
      () => assert.fail("the pull should fail"),
      (e: unknown) => e,
    );
    assert.ok(error instanceof PullError);
    assert.ok(!error.message.includes(TOKEN), error.message);
    return error;
  };

  test("a refused token says why", async () => {
    const error = await failing(403, '{"status":403,"err":"Invalid token"}');
    assert.match(error.message, /token was refused.*file_content:read scope: Invalid token$/);
  });

  test("a missing file", async () => {
    assert.match((await failing(404, "{}")).message, /file was not found/);
  });

  test("rate limiting gives the wait", async () => {
    const error = await failing(429, "{}", { "retry-after": "30" });
    assert.match(error.message, /rate limited; retry after 30 s/);
  });

  test("the server's words lose their control characters", async () => {
    const error = await failing(400, JSON.stringify({ err: "bad\u001b[31m ids" }));
    assert.match(error.message, /: bad \[31m ids$/);
  });

  test("an error that quotes the token has it redacted", async () => {
    const error = await failing(400, JSON.stringify({ err: `token ${TOKEN} is odd` }));
    assert.match(error.message, /token \[token\] is odd/);
  });

  test("a node that is not in the file", async () => {
    const { options } = await served(corpusMarkup("login"));
    await assert.rejects(
      pullScreen({ ...options, nodeId: "9:999" }),
      /node 9:999 is not in the file/,
    );
  });

  test("a response larger than the bound is refused", async () => {
    const { options } = await served(corpusMarkup("login"));
    await assert.rejects(pullScreen({ ...options, maxBytes: 100 }), /larger than 100 bytes/);
  });

  test("an unreachable server is an error without the token", async () => {
    const { options } = await served(corpusMarkup("login"));
    await assert.rejects(pullScreen({ ...options, api: "http://127.0.0.1:1" }), PullError);
  });

  test("JSON that is not a file response", async () => {
    const { server, options } = await served(corpusMarkup("login"));
    server.fail = { status: 200, body: "[1,2]" };
    await assert.rejects(pullScreen(options), /has no nodes/);
  });
});

describe("parseTarget", () => {
  test("a design link with node-id", () => {
    assert.deepEqual(parseTarget(`https://www.figma.com/design/${FILE_KEY}/Login?node-id=12-34`), {
      fileKey: FILE_KEY,
      nodeId: "12:34",
    });
  });

  test("a branch link names the branch key", () => {
    const branch = "ZyXwVuTsRqPoNmLkJiHgFe";
    const url = `https://figma.com/design/${FILE_KEY}/branch/${branch}/Login?node-id=1-2`;
    assert.deepEqual(parseTarget(url), { fileKey: branch, nodeId: "1:2" });
  });

  test("a bare file key with --node", () => {
    assert.deepEqual(parseTarget(FILE_KEY, "3:4"), { fileKey: FILE_KEY, nodeId: "3:4" });
  });

  test("--node wins over the link's node-id", () => {
    const url = `https://figma.com/design/${FILE_KEY}/x?node-id=1-2`;
    assert.deepEqual(parseTarget(url, "5:6"), { fileKey: FILE_KEY, nodeId: "5:6" });
  });

  for (const [what, target, node] of [
    ["another host", `https://figma.com.evil.test/design/${FILE_KEY}/x?node-id=1-2`, undefined],
    ["no node id", `https://figma.com/design/${FILE_KEY}/x`, undefined],
    ["a node id that is not one", FILE_KEY, "1:2&ids=3"],
    ["a key that is not one", "../../etc", "1:2"],
    ["a link without a key", "https://figma.com/design/", "1:2"],
  ] as const)
    test(`refuses ${what}`, () => {
      assert.ok("error" in parseTarget(target, node));
    });
});
