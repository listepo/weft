// The Pages deploy is how Penpot installs the plugin: only from the URL of its manifest.json.
// This suite reads the workflow offline and pins the deploy shape, so a change that un-pins an
// action, drops the gate, or publishes something other than dist/ fails here and not in CI.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, test } from "vitest";

const workflow = readFileSync(
  fileURLToPath(new URL("../../../.github/workflows/ci.yml", import.meta.url)),
  "utf8",
);

/** The lines of one job, from its key to the next job or the end of the file. */
function job(name: string): string {
  const lines = workflow.split("\n");
  const start = lines.findIndex((line) => line === `  ${name}:`);
  assert.notEqual(start, -1, `job ${name} is missing from .github/workflows/ci.yml`);
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (/^  [\w-]+:/.test(line) || /^[^\s#]/.test(line)) {
      end = i;
      break;
    }
  }
  return lines.slice(start, end).join("\n");
}

const uses = [...workflow.matchAll(/^\s*(?:-\s+)?uses:\s*(\S+)/gm)].map((match) => match[1] ?? "");

describe("the Pages deploy", () => {
  test("runs only on a push to main, after the gate", () => {
    const deploy = job("deploy-pages");
    assert.match(deploy, /^\s*needs:\s*gate$/m);
    assert.match(
      deploy,
      /^\s*if:\s*github\.event_name == 'push' && github\.ref == 'refs\/heads\/main'$/m,
    );
  });

  test("builds the Penpot plugin", () => {
    assert.match(job("deploy-pages"), /^\s*(?:-\s+)?run:\s*moon run penpot-plugin:build$/m);
  });

  test("publishes exactly plugins/penpot/dist, under the penpot/ subpath", () => {
    const deploy = job("deploy-pages");
    assert.match(deploy, /cp -R plugins\/penpot\/dist\/\. site\/penpot\//);
    assert.match(deploy, /^\s*path:\s*site$/m);
    assert.doesNotMatch(deploy, /cp -R plugins\/penpot\/(?!dist)/);
    assert.doesNotMatch(deploy, /^\s*path:\s*(\.|plugins|plugins\/penpot)\s*$/m);
  });

  test("pins every action to a commit SHA", () => {
    assert.ok(uses.length >= 4, `expected the gate's and the deploy's actions, saw ${uses.length}`);
    for (const use of uses) assert.match(use, /^[\w.-]+\/[\w./-]+@[0-9a-f]{40}$/, use);
  });

  test("grants pages and id-token to the deploy job only, and does not race a deploy", () => {
    const deploy = job("deploy-pages");
    assert.match(deploy, /^\s*pages:\s*write$/m);
    assert.match(deploy, /^\s*id-token:\s*write$/m);
    assert.doesNotMatch(job("gate"), /^\s*(pages|id-token):\s*write$/m);
    assert.match(deploy, /^\s*group:\s*github-pages$/m);
    assert.match(deploy, /^\s*cancel-in-progress:\s*false$/m);
    assert.match(deploy, /^\s*name:\s*github-pages$/m);
  });

  test("checks manifest.json, plugin.js and ui.html from the Pages URL", () => {
    const deploy = job("deploy-pages");
    assert.match(deploy, /https:\/\/listepo\.github\.io\/weft\/penpot/);
    for (const file of ["manifest.json", "plugin.js", "ui.html"])
      assert.match(deploy, new RegExp(`check ${file.replace(".", "\\.")} `));
  });
});
