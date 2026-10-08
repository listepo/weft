// The figma-pull script: a frame of a Figma file in, a `.weft` screen and a loss table out. The
// file is the fake of `@weft/figma`, served as REST JSON on a local port; the script's requests to
// api.figma.com are sent there instead, so nothing leaves the machine.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { NAMESPACE } from "@weft/figma";
import { FIGMA_API, PLUGIN_ID } from "@weft/figma/pull";
import { afterAll, afterEach, beforeAll, describe, test } from "vitest";
import { built, corpusMarkup } from "../../../packages/figma/test/helpers.ts";
import {
  FILE_KEY,
  serveFile,
  TOKEN,
  type FakeRest,
} from "../../../packages/figma/test/rest-server.ts";
import { main, type Host } from "../scripts/figma-pull.ts";

let dir: string;
let n = 0;
const work = (): string => mkdtempSync(join(dir, `${(n += 1)}-`));
beforeAll(() => {
  dir = mkdtempSync(join(tmpdir(), "weft-figma-pull-"));
});
afterAll(() => rmSync(dir, { recursive: true, force: true }));

let server: FakeRest | undefined;
afterEach(async () => {
  await server?.close();
  server = undefined;
});

/**
 * The login screen in a served fake file; the link that names its frame. With `writer`, the file
 * is one built before T14.2, its Weft source in the private plugin data of that plugin id.
 */
async function login(writer?: string): Promise<{ link: string }> {
  const { figma, frame } = await built(corpusMarkup("login"));
  if (writer !== undefined) figma.privateDataFor(NAMESPACE);
  const served = await serveFile(figma, writer ?? PLUGIN_ID);
  server = served;
  return {
    link: `https://www.figma.com/design/${FILE_KEY}/Login?node-id=${frame.id.replace(":", "-")}`,
  };
}

async function run(
  argv: string[],
  cwd: string,
  env: Record<string, string> = { FIGMA_TOKEN: TOKEN },
) {
  const out: string[] = [];
  const err: string[] = [];
  const asked: string[] = [];
  const host: Host = {
    env,
    cwd,
    fetch: (url, init) => {
      asked.push(String(url));
      const local = String(url).replace(FIGMA_API, (server as FakeRest).api);
      return fetch(local, init);
    },
  };
  const code = await main(argv, { stdout: (t) => out.push(t), stderr: (t) => err.push(t) }, host);
  return { code, stdout: out.join(""), stderr: err.join(""), asked };
}

describe("a pulled frame", () => {
  test("is written as <screen id>.weft in the working directory, byte-identical, with no losses", async () => {
    const { link } = await login();
    const cwd = work();
    const result = await run([link, "--no-project"], cwd);
    assert.equal(result.code, 0, result.stderr);
    assert.equal(readFileSync(join(cwd, "login.weft"), "utf8"), corpusMarkup("login"));
    assert.match(result.stdout, /^Wrote .*login\.weft\n\nImport losses:\n\nNo losses\.\n$/);
    // Every request went to api.figma.com, which the test routed to the fake.
    assert.ok(result.asked.every((url) => url.startsWith(`${FIGMA_API}/v1/files/`)));
  });

  test("an existing file is kept unless --force, and an output path can be named", async () => {
    const { link } = await login();
    const cwd = work();
    const target = join(cwd, "out.weft");
    assert.equal((await run([link, target, "--no-project"], cwd)).code, 0);
    const again = await run([link, target, "--no-project"], cwd);
    assert.equal(again.code, 2);
    assert.match(again.stderr, /--force/);
    assert.equal((await run([link, target, "--no-project", "--force"], cwd)).code, 0);
  });
});

describe("import.figma in weft.json", () => {
  test("outDir places the screen and pluginId names the plugin whose private data an old file holds", async () => {
    const { link } = await login("1234567890");
    const cwd = work();
    writeFileSync(
      join(cwd, "weft.json"),
      JSON.stringify({ import: { figma: { outDir: "screens", pluginId: "1234567890" } } }),
    );
    const result = await run([link], cwd);
    assert.equal(result.code, 0, result.stderr);
    assert.equal(readFileSync(join(cwd, "screens/login.weft"), "utf8"), corpusMarkup("login"));
  });

  test("--plugin-id overrides the project's pluginId", async () => {
    const { link } = await login("1234567890");
    const cwd = work();
    writeFileSync(join(cwd, "weft.json"), JSON.stringify({ import: { figma: { pluginId: "1" } } }));
    const result = await run([link, "--plugin-id", "1234567890"], cwd);
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /No losses/);
  });
});

describe("failures", () => {
  test("no FIGMA_TOKEN", async () => {
    const { link } = await login();
    const result = await run([link, "--no-project"], work(), {});
    assert.equal(result.code, 2);
    assert.match(result.stderr, /set FIGMA_TOKEN/);
    assert.deepEqual(result.asked, []);
  });

  test("a refused token is reported without the token", async () => {
    const { link } = await login();
    const wrong = "figd_wrong-secret-value";
    const result = await run([link, "--no-project"], work(), { FIGMA_TOKEN: wrong });
    assert.equal(result.code, 2);
    assert.match(result.stderr, /token was refused/);
    assert.ok(!result.stderr.includes(wrong) && !result.stdout.includes(wrong));
  });

  test("a link that is not Figma's", async () => {
    const result = await run([`https://example.com/design/${FILE_KEY}/x?node-id=1-2`], work());
    assert.equal(result.code, 2);
    assert.match(result.stderr, /example\.com is not figma\.com/);
    assert.deepEqual(result.asked, []);
  });

  test("no target, or a flag the script does not know", async () => {
    assert.equal((await run([], work())).code, 2);
    assert.equal((await run([FILE_KEY, "--token", "x"], work())).code, 2);
  });
});
