// Design Tokens loader for the DTCG 2025.10 format
// (https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/).
//
// Supported subset: groups and tokens (`$value`), `$type` on a token or inherited from the nearest
// ancestor group, the `$root` token name, and whole-value aliases written `{group.token}`
// (chains allowed; a token without its own `$type` takes the type of the token it aliases).
// Other `$`-properties (`$description`, `$extensions`, `$deprecated`) are ignored.
// Not supported: `$extends`, JSON-pointer `$ref`, the resolver module, and aliases nested inside
// composite values - such an object is kept as-is, so its inner aliases stay unresolved.
// Bad input never throws: it is reported as problems and the offending tokens are left out.

export type Token = { type: string; value: unknown };

export type TokenProblem = {
  code: "T001" | "T002" | "T003" | "T004" | "T005" | "T006";
  path: string;
  message: string;
};

type Raw = { type: string | undefined; value: unknown };

const ALIAS = /^\{([^{}]+)\}$/;
const FORBIDDEN_IN_NAME = /[{}.]/;

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

const aliasTarget = (value: unknown): string | undefined =>
  typeof value === "string" ? ALIAS.exec(value)?.[1] : undefined;

export function loadTokens(json: unknown): {
  tokens: Map<string, Token>;
  problems: TokenProblem[];
} {
  const problems: TokenProblem[] = [];
  const raw = new Map<string, Raw>();

  if (!isObject(json)) {
    problems.push({ code: "T001", path: "", message: "A tokens file must be a JSON object." });
    return { tokens: new Map(), problems };
  }

  const walk = (node: Record<string, unknown>, path: string[], inherited: string | undefined) => {
    const own = node["$type"];
    if (own !== undefined && typeof own !== "string") {
      problems.push({
        code: "T006",
        path: path.join("."),
        message: "`$type` must be a string.",
      });
    }
    const type = typeof own === "string" ? own : inherited;
    for (const [name, child] of Object.entries(node)) {
      // `$root` is the only `$` name that is a token; the rest are format properties.
      if (name.startsWith("$") && name !== "$root") continue;
      const childPath = [...path, name];
      if (FORBIDDEN_IN_NAME.test(name)) {
        problems.push({
          code: "T002",
          path: childPath.join("."),
          message: `Name "${name}" must not contain "{", "}" or ".".`,
        });
        continue;
      }
      if (!isObject(child)) {
        problems.push({
          code: "T006",
          path: childPath.join("."),
          message: "A token or group must be a JSON object.",
        });
      } else if ("$value" in child) {
        const t = child["$type"];
        raw.set(childPath.join("."), {
          type: typeof t === "string" ? t : type,
          value: child["$value"],
        });
      } else {
        walk(child, childPath, type);
      }
    }
  };
  walk(json, [], undefined);

  const tokens = new Map<string, Token>();
  // A path maps to its resolution, or to the problem code explaining why it has none.
  const state = new Map<string, Token | "failed" | "visiting">();

  const resolve = (path: string, chain: string[]): Token | "failed" => {
    const known = state.get(path);
    if (known === "visiting") {
      const cycle = [...chain.slice(chain.indexOf(path)), path].join(" -> ");
      problems.push({
        code: "T005",
        path,
        message: `Alias cycle: ${cycle}.`,
      });
      return "failed";
    }
    if (known !== undefined) return known;

    const token = raw.get(path);
    if (token === undefined) return "failed";
    state.set(path, "visiting");

    let result: Token | "failed";
    const target = aliasTarget(token.value);
    if (target === undefined) {
      result = token.type === undefined ? "failed" : { type: token.type, value: token.value };
      if (result === "failed") {
        problems.push({
          code: "T003",
          path,
          message: "Token has no `$type` and none is inherited from a group.",
        });
      }
    } else if (!raw.has(target)) {
      problems.push({
        code: "T004",
        path,
        message: `Alias {${target}} points at a token that does not exist.`,
      });
      result = "failed";
    } else {
      const inner = resolve(target, [...chain, path]);
      if (inner === "failed") {
        // The root cause was reported where it was found; this token is only a casualty.
        result = "failed";
        if (!problems.some((p) => p.path === path)) {
          problems.push({
            code: "T004",
            path,
            message: `Alias {${target}} cannot be resolved.`,
          });
        }
      } else {
        result = { type: token.type ?? inner.type, value: inner.value };
      }
    }
    state.set(path, result);
    return result;
  };

  for (const path of raw.keys()) {
    const resolved = resolve(path, []);
    if (resolved !== "failed") tokens.set(path, resolved);
  }
  return { tokens, problems };
}

export function tokenTypes(tokens: Map<string, Token>): Map<string, string> {
  return new Map([...tokens].map(([path, token]) => [path, token.type]));
}
