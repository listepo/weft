import type { Loss, TokenTree } from "./types.ts";

/** A token on its way to the tree: `path` is where it goes, `source` how its input named it. */
export type Token = { path: string[]; type: string; value: unknown; source: string };

const ALIAS = /^\{([^{}]+)\}$/;

/** The path an alias value points at (`{color.primary}` is `["color", "primary"]`), or `undefined`. */
export function aliasPath(value: unknown): string[] | undefined {
  const match = typeof value === "string" ? ALIAS.exec(value) : null;
  return match === null ? undefined : (match[1] as string).split(".");
}

/** A DTCG name: no dots or braces, and no `$` prefix, which the format keeps for its own members. */
export const isTokenName = (name: string): boolean =>
  name !== "" && !/[.{}]/.test(name) && !name.startsWith("$");

/**
 * Drops the aliases that point at nothing or loop, each as a loss, and gives the rest the type of
 * the token they end at, so every token of the tree carries its own `$type`. Removing one can orphan
 * another that pointed at it, hence the repetition until nothing changes.
 */
function settleAliases(tokens: Map<string, Token>, losses: Loss[]): void {
  for (let changed = true; changed;) {
    changed = false;
    for (const [key, token] of tokens) {
      const target = aliasPath(token.value);
      if (target === undefined) continue;
      const seen = new Set([key]);
      let end: Token | undefined = token;
      let reason = "";
      while (end !== undefined && aliasPath(end.value) !== undefined) {
        const next: string = (aliasPath(end.value) as string[]).join(".");
        if (seen.has(next)) {
          reason = "loops back to itself";
          end = undefined;
        } else {
          seen.add(next);
          end = tokens.get(next);
          if (end === undefined) reason = `points at ${next}, which is not a token of the result`;
        }
      }
      if (end === undefined) {
        losses.push({
          kind: "unresolved-alias",
          path: token.source,
          note: `The reference ${token.value as string} ${reason}; the token is left out.`,
        });
        tokens.delete(key);
        changed = true;
      } else if (token.type !== end.type) {
        token.type = end.type;
      }
    }
  }
}

/** The DTCG tree for a set of tokens. A token and a group cannot share a name, so the later one is left out. */
export function toTree(tokens: Token[], losses: Loss[]): TokenTree {
  const byPath = new Map<string, Token>();
  for (const token of tokens) byPath.set(token.path.join("."), token);
  settleAliases(byPath, losses);
  const root: TokenTree = {};
  for (const token of byPath.values()) {
    let group = root;
    const groups = token.path.slice(0, -1);
    const last = token.path.at(-1) as string;
    let blocked = false;
    for (const name of groups) {
      const next = group[name];
      if (next === undefined) group = group[name] = {} as TokenTree;
      else if (typeof next === "object" && next !== null && !("$value" in next))
        group = next as TokenTree;
      else {
        blocked = true;
        break;
      }
    }
    if (!blocked && group[last] === undefined)
      group[last] = { $type: token.type, $value: token.value };
    else {
      losses.push({
        kind: "invalid-name",
        path: token.source,
        note: `${token.path.join(".")} would be both a token and a group; the token is left out.`,
      });
    }
  }
  return root;
}
