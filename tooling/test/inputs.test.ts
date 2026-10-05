import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, matchesGlob } from "node:path";
import { describe, test } from "vitest";
import { type Metadata, crateFolders, externalIncludes } from "../closure.ts";

// A workspace of five crates: `app` depends on `lib` and, at build time only, on `gen`; `lib`
// depends on `util` for its tests only; `other` depends on `app` and so is not part of its build.
const metadata: Metadata = {
  workspace_root: "/w",
  workspace_members: ["app", "lib", "gen", "util", "other"],
  packages: ["app", "lib", "gen", "util", "other"].map((name) => ({
    id: name,
    name,
    manifest_path: `/w/crates/${name}/Cargo.toml`,
  })),
  resolve: {
    nodes: [
      {
        id: "app",
        deps: [
          { pkg: "lib", dep_kinds: [{ kind: null }] },
          { pkg: "gen", dep_kinds: [{ kind: "build" }] },
          { pkg: "serde", dep_kinds: [{ kind: null }] },
        ],
      },
      { id: "lib", deps: [{ pkg: "util", dep_kinds: [{ kind: "dev" }] }] },
      { id: "other", deps: [{ pkg: "app", dep_kinds: [{ kind: null }] }] },
    ],
  },
};

describe("crateFolders", () => {
  test("follows normal and build dependencies and skips development ones", () => {
    assert.deepEqual(crateFolders(metadata, "app"), ["crates/app", "crates/gen", "crates/lib"]);
  });

  test("leaves out the crates that depend on the root", () => {
    assert.deepEqual(crateFolders(metadata, "lib"), ["crates/lib"]);
  });

  test("keeps a crate reached both as a development and as a normal dependency", () => {
    const both = structuredClone(metadata);
    both.resolve.nodes[1]?.deps.push({ pkg: "util", dep_kinds: [{ kind: null }] });
    assert.deepEqual(crateFolders(both, "lib"), ["crates/lib", "crates/util"]);
  });

  test("rejects a root that is not a package", () => {
    assert.throws(() => crateFolders(metadata, "missing"), /no package named missing/);
  });
});

describe("externalIncludes", () => {
  const file = "crates/lib/src/core.rs";

  test("resolves a path against the including file", () => {
    const text = 'const A: &str = include_str!("../../../packages/catalog/catalog.json");';
    assert.deepEqual(externalIncludes("crates/lib", file, text), ["packages/catalog/catalog.json"]);
  });

  test("ignores files inside the crate", () => {
    const text = 'include_str!("helpers.swift"); include_bytes!("../tests/fixtures/a.bin");';
    assert.deepEqual(externalIncludes("crates/lib", file, text), []);
  });

  test("finds every form of the macro", () => {
    const text = 'include!("../../x/a.rs"); include_bytes! ( "../../x/b.bin" );';
    assert.deepEqual(externalIncludes("crates/lib", file, text), [
      "crates/x/a.rs",
      "crates/x/b.bin",
    ]);
  });
});

// The two tasks, the crate each builds from, and the build itself: every crate the build compiles
// is an input, no other crate is, and so is every file those crates include from elsewhere.
describe("the declared inputs of the Rust builds", () => {
  const read = (command: string, args: string[]) =>
    execFileSync(command, args, {
      encoding: "utf8",
      maxBuffer: 1 << 28,
      stdio: ["ignore", "pipe", "pipe"],
    });
  const real = JSON.parse(
    read("cargo", ["metadata", "--format-version", "1", "--all-features", "--locked"]),
  ) as Metadata;
  const tasks = (
    JSON.parse(read("moon", ["query", "tasks"])) as {
      tasks: Record<
        string,
        Record<
          string,
          { inputFiles?: Record<string, unknown>; inputGlobs?: Record<string, unknown> }
        >
      >;
    }
  ).tasks;

  // `--all-features` joins the builds with and without the `web` feature of weft-wasm.
  for (const [target, root] of [
    ["wasm", "weft-wasm"],
    ["native", "weft-node"],
  ] as const) {
    const declared = tasks.root?.[target];
    const folders = crateFolders(real, root);
    const globs = Object.keys(declared?.inputGlobs ?? {});
    const files = Object.keys(declared?.inputFiles ?? {});

    test(`root:${target} lists the crates ${root} is built from, and no other`, () => {
      assert.ok(declared, `root:${target} is not a moon task`);
      assert.deepEqual(
        globs.filter((glob) => glob.startsWith("crates/")).sort(),
        folders.map((folder) => `${folder}/**/*`),
      );
      for (const file of ["Cargo.toml", "Cargo.lock"]) {
        assert.ok(files.includes(file), `${file} is not an input of root:${target}`);
      }
    });

    test(`root:${target} lists the files its crates include from outside their folders`, () => {
      const covered = (path: string) =>
        files.includes(path) || globs.some((glob) => matchesGlob(path, glob));
      for (const folder of folders) {
        const sources = [join(folder, "build.rs")]
          .filter((path) => existsSync(join(real.workspace_root, path)))
          .concat(
            readdirSync(join(real.workspace_root, folder, "src"), { recursive: true })
              .map(String)
              .filter((name) => name.endsWith(".rs"))
              .map((name) => join(folder, "src", name)),
          );
        for (const source of sources) {
          const text = readFileSync(join(real.workspace_root, source), "utf8");
          for (const path of externalIncludes(folder, source, text)) {
            assert.ok(
              covered(path),
              `${source} includes ${path}, which root:${target} does not list`,
            );
          }
        }
      }
    });
  }
});
