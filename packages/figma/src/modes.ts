// Token modes as Figma variable modes (SPEC §10.3): each context of a resolver modifier is a mode
// of the library's collection, the default context its default mode, and the modes read back into
// a resolver document. Modes need a paid plan, so a mode the plan refuses is skipped and noted,
// never fatal: the default mode alone is the file Figma had before.
import type { Token, TokenModifier } from "@weft/catalog";
import {
  KEY,
  MAX_CONTEXTS,
  modeToken,
  readMark,
  resolverDocument,
  tokenColor,
  tokenPx,
} from "@weft/design-tool";
import type { FCollection, FigmaApi, FRGBA, FVariable } from "./api.ts";
import { dataOf } from "./data.ts";

export function variableValue(
  token: Token | undefined,
): { type: "FLOAT"; value: number } | { type: "COLOR"; value: FRGBA } | undefined {
  const px = tokenPx(token);
  if (px !== undefined) return { type: "FLOAT", value: px };
  const color = tokenColor(token);
  return color === undefined ? undefined : { type: "COLOR", value: color };
}

/**
 * Gives the collection a mode per context and every variable its value in each. A context
 * without the token, or with a value of another type, takes the default context's value.
 */
export function writeModes(
  collection: FCollection,
  variables: ReadonlyMap<string, FVariable>,
  modifier: TokenModifier,
  notes: string[],
): void {
  dataOf(collection).setPluginData(KEY.modifier, modifier.name);
  if (
    collection.modes.find((m) => m.modeId === collection.defaultModeId)?.name !== modifier.default
  )
    collection.renameMode(collection.defaultModeId, modifier.default);
  const modes: [string, ReadonlyMap<string, Token>][] = [];
  for (const [name, tokens] of [...modifier.contexts].slice(0, MAX_CONTEXTS)) {
    if (name === modifier.default) continue;
    let id = collection.modes.find(
      (m) => m.name === name && m.modeId !== collection.defaultModeId,
    )?.modeId;
    if (id === undefined) {
      try {
        id = collection.addMode(name);
      } catch (error) {
        notes.push(
          `The ${modifier.name} context "${name}" has no Figma mode: ${error instanceof Error ? error.message : String(error)}`,
        );
        continue;
      }
    }
    modes.push([id, tokens]);
  }
  for (const [path, variable] of variables) {
    const fallback = variable.valuesByMode[collection.defaultModeId];
    for (const [id, tokens] of modes) {
      const value = variableValue(tokens.get(path));
      if (value !== undefined && value.type === variable.resolvedType)
        variable.setValueForMode(id, value.value);
      else if (typeof fallback === "number" || isColor(fallback))
        variable.setValueForMode(id, fallback);
    }
  }
}

const isColor = (v: unknown): v is FRGBA =>
  typeof v === "object" && v !== null && "r" in v && "g" in v && "b" in v;

/**
 * The library collection's modes as a resolver document, when it has more than one. Each mode is
 * `tokens` with the values the file holds in that mode; the modifier is the one the build named,
 * else `mode`. A mode named like an earlier one, an alias and a malformed value are left out.
 */
export async function readModes(
  api: FigmaApi,
  tokens: ReadonlyMap<string, Token>,
): Promise<Record<string, unknown> | undefined> {
  const collection = (await api.variables.getLocalVariableCollectionsAsync()).find(
    (c) => readMark(dataOf(c), KEY.library) !== undefined,
  );
  if (collection === undefined || collection.modes.length < 2) return undefined;
  const variables = (await api.variables.getLocalVariablesAsync()).filter(
    (v) => v.variableCollectionId === collection.id,
  );
  const contexts = new Map<string, Map<string, Token>>();
  let fallback: string | undefined;
  for (const mode of collection.modes.slice(0, MAX_CONTEXTS)) {
    if (contexts.has(mode.name)) continue;
    const context = new Map(tokens);
    for (const v of variables) {
      const path = readMark(dataOf(v), KEY.token);
      if (path === undefined) continue;
      const token = modeToken(tokens.get(path), v.valuesByMode[mode.modeId]);
      if (token !== undefined) context.set(path, token);
    }
    contexts.set(mode.name, context);
    if (mode.modeId === collection.defaultModeId) fallback = mode.name;
  }
  const name = readMark(dataOf(collection), KEY.modifier) ?? "mode";
  const first = contexts.keys().next().value as string;
  return resolverDocument({ name, default: fallback ?? first, contexts });
}
