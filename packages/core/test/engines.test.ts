// The two engines behind the core: the WebAssembly module and the native addon. The suites of
// this package run under both (`WEFT_ENGINE=wasm` and `WEFT_ENGINE=native`, see moon.yml); these
// tests check that the choice is honoured, that the addon and the module export the same names,
// and that an addon that is missing or broken never stops the core from loading.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, test } from "vitest";
import * as module from "../wasm-web/weft.js";
import { buildName, loadNative } from "../src/native.ts";
import { engine } from "../src/wasm.ts";

const addon = loadNative({ choice: "native", warn: () => {} });
const requested = process.env.WEFT_ENGINE;

/** The names a host can call: no glue internals, no memory management. */
function surface(exports: object): string[] {
  return Object.keys(exports)
    .filter((name) => !name.startsWith("_") && name !== "default" && name !== "initSync")
    .sort();
}

function methods(Catalog: { prototype: object }): string[] {
  return Object.getOwnPropertyNames(Catalog.prototype)
    .filter((name) => !name.startsWith("_") && name !== "constructor" && name !== "free")
    .sort();
}

describe("engine choice", () => {
  test("WEFT_ENGINE=native loads the addon, and fails here when it did not", () => {
    if (requested === "native") assert.equal(engine, "native");
    if (requested === "wasm") assert.equal(engine, "wasm");
  });

  test("an unknown WEFT_ENGINE warns and means auto", () => {
    const warnings: string[] = [];
    loadNative({ choice: "gpu", dir: new URL("file:///nowhere/"), warn: (m) => warnings.push(m) });
    assert.equal(warnings.length, 1);
    assert.match(warnings[0] ?? "", /WEFT_ENGINE=gpu/);
  });

  test("wasm never looks for the addon", () => {
    const warnings: string[] = [];
    assert.equal(loadNative({ choice: "wasm", warn: (m) => warnings.push(m) }), undefined);
    assert.deepEqual(warnings, []);
  });
});

describe("fallback to WebAssembly", () => {
  const build = buildName();
  const dir = mkdtempSync(join(tmpdir(), "weft-addon-"));
  const folder = pathToFileURL(`${dir}/`);

  test("a missing addon is silent under auto and a warning under native", () => {
    const warnings: string[] = [];
    const warn = (message: string) => warnings.push(message);
    assert.equal(loadNative({ choice: "auto", dir: folder, warn }), undefined);
    assert.deepEqual(warnings, []);
    assert.equal(loadNative({ choice: "native", dir: folder, warn }), undefined);
    assert.equal(warnings.length, 1);
    assert.match(warnings[0] ?? "", /using WebAssembly/);
  });

  test.skipIf(build === undefined)("a broken addon warns, whatever the choice", () => {
    writeFileSync(join(dir, `weft.${build}.node`), "this is not a shared library");
    for (const choice of ["auto", "native"]) {
      const warnings: string[] = [];
      const loaded = loadNative({ choice, dir: folder, warn: (m) => warnings.push(m) });
      assert.equal(loaded, undefined);
      assert.equal(warnings.length, 1);
      assert.match(warnings[0] ?? "", /cannot load .*weft\..*\.node, using WebAssembly/);
    }
    rmSync(dir, { recursive: true });
  });
});

describe("the addon and the module", () => {
  test.skipIf(addon === undefined)("export the same names", () => {
    const native = addon as typeof module;
    assert.deepEqual(surface(native), surface(module));
    assert.deepEqual(methods(native.Catalog), methods(module.Catalog));
  });

  test.skipIf(addon === undefined)("answer a call with the same text", () => {
    const native = addon as typeof module;
    // Under WEFT_ENGINE=native the core never instantiates the module.
    module.initSync({ module: readFileSync(new URL("../wasm-web/weft_bg.wasm", import.meta.url)) });
    const call = (engine: typeof module) => [
      engine.parse('<screen id="s"/>', "{}"),
      engine.canonicalize('{"weft":"0.1","root":{"kind":"screen"}}'),
      engine.didYouMean("buton", '["button","badge"]'),
    ];
    assert.deepEqual(call(native), call(module));
  });
});

describe("runtimes", () => {
  const script = new URL("./engine-smoke.ts", import.meta.url).pathname;

  function run(runtime: string, args: string[], choice: string) {
    return spawnSync(runtime, [...args, script], {
      encoding: "utf8",
      env: { ...process.env, WEFT_ENGINE: choice },
      timeout: 15_000,
    });
  }

  for (const choice of ["wasm", "native"]) {
    test.skipIf(choice === "native" && addon === undefined)(`Node runs on ${choice}`, () => {
      const result = run(process.execPath, [], choice);
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), {
        engine: choice,
        root: "screen",
        invalid: true,
      });
    });

    // Bun is optional: without it the test says so and passes nothing it did not run.
    const bun = spawnSync("bun", ["--version"], { encoding: "utf8" });
    const hasBun = bun.status === 0;
    if (!hasBun) console.warn("bun is not on PATH: the Bun engine tests are skipped");
    test.skipIf(!hasBun || (choice === "native" && addon === undefined))(
      `Bun runs on ${choice}`,
      () => {
        const result = run("bun", ["run"], choice);
        assert.equal(result.error, undefined, result.error?.message);
        assert.equal(result.status, 0, result.stderr);
        assert.deepEqual(JSON.parse(result.stdout), {
          engine: choice,
          root: "screen",
          invalid: true,
        });
      },
    );
  }
});
