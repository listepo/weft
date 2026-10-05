// What a Rust build is made of, read from `cargo metadata`: the workspace crates it compiles and the
// files their sources include from outside their own folders. The moon tasks that build the
// WebAssembly module and the native addon list both as inputs; tooling/test/inputs.test.ts compares
// the lists with these functions, so a dependency or an include added later cannot leave a task
// serving a stale cached build.
import { posix } from "node:path";

export interface Metadata {
  workspace_root: string;
  workspace_members: string[];
  packages: { id: string; name: string; manifest_path: string }[];
  resolve: {
    nodes: { id: string; deps: { pkg: string; dep_kinds: { kind: string | null }[] }[] }[];
  };
}

/**
 * Workspace crates that a build of `root` compiles, itself included, as workspace-relative folders.
 * Development dependencies are left out: a release build of the library never compiles them.
 */
export function crateFolders(metadata: Metadata, root: string): string[] {
  const members = new Set(metadata.workspace_members);
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const start = metadata.packages.find((pkg) => pkg.name === root);
  if (!start) {
    throw new Error(`no package named ${root}`);
  }
  const seen = new Set([start.id]);
  const queue = [start.id];
  for (let id = queue.pop(); id !== undefined; id = queue.pop()) {
    for (const dep of nodes.get(id)?.deps ?? []) {
      // A registry crate cannot depend on a path crate, so the walk stays inside the workspace.
      const built = dep.dep_kinds.some((kind) => kind.kind !== "dev");
      if (built && members.has(dep.pkg) && !seen.has(dep.pkg)) {
        seen.add(dep.pkg);
        queue.push(dep.pkg);
      }
    }
  }
  const folder = (manifest: string) =>
    posix.relative(metadata.workspace_root, posix.dirname(manifest));
  return metadata.packages
    .filter((pkg) => seen.has(pkg.id))
    .map((pkg) => folder(pkg.manifest_path))
    .sort();
}

/**
 * Files that one source file includes (`include_str!`, `include_bytes!`, `include!`) from outside
 * the crate folder, as workspace-relative paths. The macros resolve a path against the including file.
 */
export function externalIncludes(crateFolder: string, file: string, text: string): string[] {
  const found: string[] = [];
  for (const [, path = ""] of text.matchAll(/\binclude(?:_str|_bytes)?!\s*\(\s*"([^"]+)"/g)) {
    const resolved = posix.join(posix.dirname(file), path);
    if (!resolved.startsWith(`${crateFolder}/`)) {
      found.push(resolved);
    }
  }
  return found;
}
