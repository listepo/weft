import assert from "node:assert/strict";
import { describe, test } from "vitest";
import {
  type Crate,
  type Facts,
  type Project,
  type Task,
  referencedProjects,
  select,
} from "../select.ts";

// A small copy of the real workspace: the same task shapes (the inherited `test` inputs, the
// coarse `packages/**` and `crates/**` globs, the generated-module dependencies), fewer members.
const projects: Project[] = [
  { id: "root", source: ".", dependencies: [] },
  { id: "core", source: "packages/core", dependencies: ["root"] },
  { id: "catalog", source: "packages/catalog", dependencies: ["root", "core"] },
  { id: "mcp", source: "packages/mcp", dependencies: ["root", "core", "catalog"] },
  { id: "bench", source: "bench", dependencies: ["root", "core"] },
  { id: "shared", source: "plugins/shared", dependencies: ["root", "mcp"] },
  { id: "visual", source: "packages/visual", dependencies: ["root", "core"] },
];

const crates: Crate[] = [
  { name: "weft-core", dir: "crates/weft-core", dependsOn: [] },
  { name: "weft-catalog", dir: "crates/weft-catalog", dependsOn: ["weft-core"] },
  { name: "weft-swiftui", dir: "crates/weft-swiftui", dependsOn: ["weft-core", "weft-catalog"] },
  { name: "weft-cli", dir: "crates/weft-cli", dependsOn: ["weft-catalog", "weft-swiftui"] },
  { name: "weft-wasm", dir: "crates/weft-wasm", dependsOn: ["weft-core", "weft-catalog"] },
  { name: "weft-node", dir: "crates/weft-node", dependsOn: ["weft-core"] },
];

const shared = ["corpus/**/*", "examples/**/*", "schemas/**/*", "compat/**/*"];

const task = (target: string, over: Partial<Task>): Task => {
  const [project = "", name = ""] = target.split(":");
  return {
    target,
    project,
    name,
    globs: [],
    files: [],
    deps: [],
    vitest: false,
    ...over,
  };
};

const test_ = (id: string, over: Partial<Task> = {}): Task => {
  const source = projects.find((project) => project.id === id)?.source ?? id;
  return task(`${id}:test`, {
    globs: [`${source}/**/*`, "packages/**/*", ...shared, "crates/*/tests/fixtures/**/*"],
    files: ["SPEC.md", "AGENT-SPEC.md"],
    deps: ["root:wasm", "root:native"],
    vitest: true,
    ...over,
  });
};

const tasks: Task[] = [
  // The build tasks list the crates they are made of, not `crates/**/*` (T56).
  task("root:wasm", {
    globs: ["crates/weft-wasm/**/*", "crates/weft-core/**/*", "crates/weft-catalog/**/*"],
    files: ["packages/catalog/catalog.json"],
  }),
  task("root:native", {
    globs: ["crates/weft-node/**/*", "crates/weft-core/**/*", "crates/weft-catalog/**/*"],
    files: ["packages/catalog/catalog.json"],
  }),
  task("root:cli", {}),
  task("root:typecheck", {
    globs: ["packages/**/*", "plugins/**/*", "bench/**/*", "runtimes/**/*", "tooling/**/*"],
    deps: ["root:wasm"],
  }),
  task("root:lint", {
    globs: [
      "packages/**/*",
      "plugins/**/*",
      "bench/**/*",
      "runtimes/**/*",
      "compat/**/*",
      "*.json",
    ],
  }),
  task("root:rust-test", {
    globs: [
      "crates/**/*",
      "packages/catalog/examples/**/*",
      "examples/**/*",
      "schemas/**/*",
      "corpus/**/*",
    ],
    files: ["Cargo.toml", "Cargo.lock", "packages/catalog/catalog.json"],
  }),
  task("root:rust-lint", { globs: ["crates/**/*"], files: ["Cargo.toml", "Cargo.lock"] }),
  task("root:runtimes-node", {
    globs: ["runtimes/**/*", "packages/**/*", "!packages/**/node_modules/**", ...shared],
    deps: ["root:wasm"],
  }),
  task("root:runtimes", { deps: ["root:runtimes-node"] }),
  // Scripts running both engines: no `vitest` command to pass `--changed` to.
  test_("core", { vitest: false, deps: ["root:wasm", "root:native"] }),
  test_("catalog", { vitest: false, deps: ["root:wasm", "root:native"] }),
  test_("mcp"),
  test_("bench"),
  test_("shared", { globs: ["plugins/shared/**/*", "packages/**/*", "plugins/*/**/*"] }),
  test_("visual", {
    globs: ["packages/visual/**/*", "packages/**/*", "crates/**/*"],
    deps: ["root:wasm", "root:native", "root:cli"],
  }),
];

const noReads = { source: {}, own: {} };

const plan = (changed: string[], reads: Facts["reads"] = noReads) => {
  const result = select({ changed, projects, tasks, crates, reads });
  assert.equal(result.kind, "selected");
  return result.kind === "selected" ? result : assert.fail("full");
};

describe("a docs-only change", () => {
  test("runs nothing", () => {
    const result = plan(["README.md", "plan.md", "docs/guide.md"]);
    assert.deepEqual(result.narrowed, []);
    assert.deepEqual(result.whole, []);
    assert.deepEqual(result.rust, { tests: null, clippy: [], fmt: false });
  });

  test("a note inside a package reaches that package's suite but not the linters", () => {
    const result = plan(["packages/mcp/NOTES.md"]);
    assert.deepEqual(result.whole, ["mcp:test", "shared:test"]);
  });

  test("still reaches the suites that read the specification", () => {
    assert.deepEqual(plan(["SPEC.md"]).narrowed, []);
    assert.deepEqual(plan(["SPEC.md"]).whole, [
      "bench:test",
      "catalog:test",
      "core:test",
      "mcp:test",
      "shared:test",
      "visual:test",
    ]);
  });
});

describe("a change in a leaf crate", () => {
  const result = plan(["crates/weft-swiftui/src/lib.rs"]);

  test("tests the crate and what depends on it", () => {
    assert.deepEqual(result.rust.tests, { filterset: "rdeps(=weft-swiftui)" });
    assert.deepEqual(result.rust.clippy, ["weft-cli", "weft-swiftui"]);
    assert.equal(result.rust.fmt, true);
  });

  test("reaches only the TypeScript that runs the CLI", () => {
    assert.deepEqual(result.whole, ["visual:test"]);
    assert.deepEqual(result.narrowed, []);
  });
});

describe("a change in weft-core", () => {
  const result = plan(["crates/weft-core/src/validate.rs"]);

  test("reaches every crate", () => {
    assert.deepEqual(result.rust.tests, { filterset: "rdeps(=weft-core)" });
    assert.deepEqual(result.rust.clippy, crates.map((crate) => crate.name).sort());
  });

  test("reaches every TypeScript check through the generated module", () => {
    assert.deepEqual(result.whole, [
      "bench:test",
      "catalog:test",
      "core:test",
      "mcp:test",
      "root:runtimes",
      "root:typecheck",
      "shared:test",
      "visual:test",
    ]);
    assert.deepEqual(result.narrowed, []);
  });

  test("combines several crates into one filterset", () => {
    const both = plan(["crates/weft-node/src/lib.rs", "crates/weft-swiftui/src/lib.rs"]);
    assert.deepEqual(both.rust.tests, { filterset: "rdeps(=weft-node) | rdeps(=weft-swiftui)" });
  });
});

describe("a change in a binding stub", () => {
  test("the WebAssembly stub reaches every suite that loads the module", () => {
    const result = plan(["crates/weft-wasm/src/lib.rs"]);
    assert.deepEqual(result.rust.tests, { filterset: "rdeps(=weft-wasm)" });
    assert.ok(
      ["core:test", "mcp:test", "visual:test", "root:typecheck"].every((t) =>
        result.whole.includes(t),
      ),
    );
  });

  test("a crate the module is not built from leaves the module and its suites alone", () => {
    const result = plan(["crates/weft-swiftui/src/lib.rs"]);
    assert.ok(!result.whole.includes("core:test") && !result.whole.includes("root:typecheck"));
    assert.ok(!result.narrowed.includes("mcp:test"));
  });

  test("a crate the module is built from rebuilds it and reaches its suites", () => {
    const result = plan(["crates/weft-catalog/src/lib.rs"]);
    assert.ok(
      ["core:test", "catalog:test", "root:typecheck"].every((t) => result.whole.includes(t)),
    );
  });

  test("the native addon reaches every suite, which all depend on it", () => {
    const result = plan(["crates/weft-node/src/lib.rs"]);
    assert.ok(
      ["catalog:test", "core:test", "mcp:test", "visual:test"].every((t) =>
        result.whole.includes(t),
      ),
    );
    assert.ok(!result.whole.includes("root:typecheck"));
  });
});

describe("a TypeScript-only change", () => {
  const result = plan(["packages/mcp/src/server.ts"]);

  test("runs the package, its dependents and the static checks", () => {
    assert.deepEqual(result.narrowed, ["mcp:test", "shared:test"]);
    assert.deepEqual(result.whole, ["root:lint", "root:typecheck"]);
  });

  test("touches no Rust", () => {
    assert.deepEqual(result.rust, { tests: null, clippy: [], fmt: false });
  });

  test("a change in core reaches its dependents and the runtime legs", () => {
    const core = plan(["packages/core/src/validate.ts"]);
    assert.deepEqual(core.narrowed, ["bench:test", "mcp:test", "shared:test", "visual:test"]);
    assert.deepEqual(core.whole, [
      "catalog:test",
      "core:test",
      "root:lint",
      "root:runtimes",
      "root:typecheck",
    ]);
  });

  test("a package that nothing imports leaves the runtime legs out", () => {
    assert.ok(!result.whole.includes("root:runtimes"));
  });
});

describe("narrowing to related tests", () => {
  test("applies only when every cause is an import", () => {
    assert.ok(plan(["packages/mcp/test/server.test.ts"]).narrowed.includes("mcp:test"));
    const data = plan(["packages/mcp/test/fixtures/a.json"]);
    assert.ok(data.whole.includes("mcp:test"));
    assert.ok(!data.narrowed.includes("mcp:test"));
  });

  test("never applies to a script task, which takes no arguments", () => {
    const result = plan(["packages/core/src/validate.ts"]);
    assert.ok(result.whole.includes("core:test"));
  });

  test("a changed data file next to a changed module runs the project whole", () => {
    const result = plan(["packages/mcp/src/a.ts", "packages/mcp/src/a.json"]);
    assert.ok(result.whole.includes("mcp:test"));
  });
});

describe("files the suites read", () => {
  test("a shared fixture reaches every suite whole and every Rust test", () => {
    const result = plan(["corpus/inbox.json"]);
    assert.deepEqual(result.narrowed, []);
    assert.equal(result.rust.tests, "all");
    assert.ok(result.whole.includes("root:runtimes"));
    assert.deepEqual(result.rust.clippy, []);
  });

  test("the catalog embedded in the crates rebuilds the module", () => {
    const result = plan(["packages/catalog/catalog.json"]);
    assert.equal(result.rust.tests, "all");
    assert.ok(result.whole.includes("core:test"));
    assert.ok(result.whole.includes("root:typecheck"));
  });

  test("a fixture inside a crate reaches the TypeScript suites that compare against it", () => {
    const result = plan(["crates/weft-core/tests/fixtures/a.json"]);
    assert.ok(result.whole.includes("bench:test"));
  });
});

describe("paths in sources", () => {
  const reads: Facts["reads"] = {
    own: { bench: ["mcp"], visual: ["shared"] },
    source: { core: ["catalog"] },
  };

  test("a test that reads a sibling by path runs whole when the sibling changes", () => {
    const result = plan(["packages/mcp/src/server.ts"], reads);
    assert.ok(result.whole.includes("bench:test"));
    assert.ok(!result.narrowed.includes("bench:test"));
  });

  test("a source that reads a sibling by path reaches the importers", () => {
    // bench imports core but not catalog, whose files core's sources read.
    const result = plan(["packages/catalog/src/index.ts"], reads);
    assert.ok(result.whole.includes("bench:test"));
  });

  test("the paths a test names do not reach the importers of that project", () => {
    const result = plan(["plugins/shared/src/index.ts"], reads);
    assert.ok(result.whole.includes("visual:test"));
    assert.ok(![...result.narrowed, ...result.whole].includes("bench:test"));
  });

  test("reads are one step, not transitive", () => {
    const result = plan(["packages/mcp/src/x.ts"], { own: { bench: ["mcp"] }, source: {} });
    assert.ok(!result.whole.includes("visual:test") && !result.narrowed.includes("visual:test"));
  });
});

describe("referencedProjects", () => {
  test("finds a path from the workspace root, a plugin path and a relative sibling", () => {
    const text = [
      'readFileSync("packages/catalog/catalog.json")',
      "resolve(here, '../../mcp/src/primer.ts')",
      'join(root, "plugins/shared")',
      'import "@weft/core"',
    ].join("\n");
    assert.deepEqual(referencedProjects(text, projects).sort(), ["catalog", "mcp", "shared"]);
  });

  test("ignores a name that is only part of a longer one", () => {
    assert.deepEqual(referencedProjects('"packages/core-extra/x"', projects), []);
  });
});

describe("the fallback to the full check", () => {
  const reasons: [string, string][] = [
    ["pnpm-lock.yaml", "lockfile"],
    ["Cargo.lock", "lockfile"],
    ["mise.toml", "toolchain"],
    [".moon/tasks/all.yml", "moon"],
    ["moon.yml", "root tasks"],
    ["pnpm-workspace.yaml", "workspace"],
    ["Cargo.toml", "workspace"],
    ["package.json", "workspace"],
    ["tsconfig.json", "TypeScript"],
    ["vitest.config.ts", "Vitest"],
    ["clippy.toml", "clippy"],
  ];
  for (const [file, word] of reasons) {
    test(`${file} changed`, () => {
      const result = select({
        changed: ["README.md", file],
        projects,
        tasks,
        crates,
        reads: noReads,
      });
      assert.equal(result.kind, "full");
      assert.ok(
        result.kind === "full" && result.reason.includes(file) && result.reason.includes(word),
      );
    });
  }

  test("a file under crates/ that belongs to no crate", () => {
    const result = select({
      changed: ["crates/new/src/lib.rs"],
      projects,
      tasks,
      crates,
      reads: noReads,
    });
    assert.deepEqual(result, {
      kind: "full",
      reason: "crates/new/src/lib.rs changed and belongs to no crate",
    });
  });

  test("a package.json inside a package is not a workspace change", () => {
    assert.equal(
      select({ changed: ["packages/mcp/package.json"], projects, tasks, crates, reads: noReads })
        .kind,
      "selected",
    );
  });
});
