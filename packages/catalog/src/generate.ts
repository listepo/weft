// Rewrites catalog.json from coreCatalog; run with `node src/generate.ts` in this package.
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { coreCatalog } from "./core.ts";

export const catalogJsonPath = new URL("../catalog.json", import.meta.url);

if (import.meta.main) {
  const file = fileURLToPath(catalogJsonPath);
  writeFileSync(file, `${JSON.stringify(coreCatalog, null, 2)}\n`);
  // The repository formatter owns the layout; formatting here keeps `pnpm run lint` green.
  execFileSync("pnpm", ["exec", "oxfmt", file], { stdio: "inherit" });
}
