// Loads the JSON fixture documents: `<name>.weft.json` with optional `<name>.data.json`.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import type { Document } from "@weft/core";

const dir = new URL("../fixtures/", import.meta.url);

export type Fixture = { name: string; document: Document; data: unknown };

export function fixtures(): Fixture[] {
  return readdirSync(dir)
    .filter((f) => f.endsWith(".weft.json"))
    .sort()
    .map((f) => {
      const name = f.slice(0, -".weft.json".length);
      const dataFile = new URL(`${name}.data.json`, dir);
      return {
        name,
        document: JSON.parse(readFileSync(new URL(f, dir), "utf8")) as Document,
        data: existsSync(dataFile) ? JSON.parse(readFileSync(dataFile, "utf8")) : {},
      };
    });
}
