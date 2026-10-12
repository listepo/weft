// What keeps a provider from taking a JSON Schema whole. The creator's rule for T12.3: such a
// provider is reported as unsupported, with these reasons, rather than sent a reduced schema.
// Anthropic's limits: https://platform.claude.com/docs/en/build-with-claude/structured-outputs
// (checked 2026-10-12; research.md §20).

type Json = Record<string, unknown>;

const isObject = (x: unknown): x is Json =>
  typeof x === "object" && x !== null && !Array.isArray(x);

function visit(x: unknown, f: (o: Json) => void): void {
  if (Array.isArray(x)) for (const y of x) visit(y, f);
  else if (isObject(x)) {
    f(x);
    for (const y of Object.values(x)) visit(y, f);
  }
}

const DEFS = "#/$defs/";

/** `$defs` entries that reach themselves through local `$ref`s. */
function recursiveDefs(schema: Json): string[] {
  const defs = isObject(schema.$defs) ? schema.$defs : {};
  const edges = new Map<string, string[]>();
  for (const [name, def] of Object.entries(defs)) {
    const out: string[] = [];
    visit(def, (o) => {
      if (typeof o.$ref === "string" && o.$ref.startsWith(DEFS))
        out.push(o.$ref.slice(DEFS.length));
    });
    edges.set(name, out);
  }
  return [...edges.keys()].filter((start) => {
    const seen = new Set<string>();
    const stack = [...(edges.get(start) ?? [])];
    while (stack.length > 0) {
      const at = stack.pop() as string;
      if (at === start) return true;
      if (seen.has(at)) continue;
      seen.add(at);
      stack.push(...(edges.get(at) ?? []));
    }
    return false;
  });
}

/** The Anthropic structured-output limits `schema` breaks; empty when it can be sent. */
export function anthropicSchemaProblems(schema: object): string[] {
  let optional = 0;
  let unions = 0;
  let bounds = 0;
  visit(schema, (o) => {
    if (isObject(o.properties)) {
      const required = Array.isArray(o.required) ? o.required : [];
      optional += Object.keys(o.properties).filter((k) => !required.includes(k)).length;
    }
    if (Array.isArray(o.anyOf) || Array.isArray(o.type)) unions++;
    if (
      ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "multipleOf"].some(
        (k) => k in o,
      )
    )
      bounds++;
  });
  const problems: string[] = [];
  const recursive = recursiveDefs(schema as Json);
  if (recursive.length > 0)
    problems.push(
      `recursive schemas are not supported (${recursive.length} $defs reach themselves)`,
    );
  if (bounds > 0) problems.push(`numerical constraints are not supported (${bounds} found)`);
  if (unions > 16) problems.push(`at most 16 union-typed parameters (${unions} found)`);
  if (optional > 24) problems.push(`at most 24 optional parameters (${optional} found)`);
  return problems;
}
