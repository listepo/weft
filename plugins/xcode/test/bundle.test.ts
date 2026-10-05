// The artifact bundle that `binaryTarget(path:)` consumes: the manifest follows SwiftPM's schema
// and the one variant is a working Apple Silicon `weft`.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { bundle, run, skipReason, weftBinary } from "./helpers.ts";

describe.skipIf(skipReason)("artifact bundle", () => {
  test("info.json declares one arm64 macOS executable", () => {
    const info = JSON.parse(readFileSync(join(bundle, "info.json"), "utf8"));
    expect(info.schemaVersion).toBe("1.0");
    expect(Object.keys(info.artifacts)).toEqual(["weft"]);
    const weft = info.artifacts.weft;
    expect(weft.type).toBe("executable");
    expect(weft.variants).toEqual([
      { path: expect.stringMatching(/^weft-.+-macosx\/bin\/weft$/), supportedTriples: ["arm64-apple-macosx"] },
    ]);
    expect(weft.variants[0].path).toBe(`weft-${weft.version}-macosx/bin/weft`);
  });

  test("the variant is an arm64 executable of the same version", () => {
    const binary = weftBinary();
    expect(run("lipo", ["-archs", binary]).output.trim()).toBe("arm64");
    const info = JSON.parse(readFileSync(join(bundle, "info.json"), "utf8"));
    expect(run(binary, ["--version"]).output.trim()).toBe(`weft ${info.artifacts.weft.version}`);
  });
});
