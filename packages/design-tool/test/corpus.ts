// What the round-trip tests of every design tool read: the corpus screens and the default tokens.
// Shared so each tool package tests against the same inputs.
import { readdirSync, readFileSync } from "node:fs";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { parse, type Document } from "@weft/core";

export const tokens = loadTokens(
  JSON.parse(
    readFileSync(new URL("../../catalog/tokens/default.tokens.json", import.meta.url), "utf8"),
  ),
).tokens;

const corpus = new URL("../../../corpus/", import.meta.url);

export const corpusNames = readdirSync(corpus, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name)
  .sort();

export const corpusMarkup = (name: string): string =>
  readFileSync(new URL(`${name}/screen.weft`, corpus), "utf8");

/**
 * The context example of SPEC §2.3: entries about the screen, an element and an element in a
 * slot. Not a corpus screen, so the benchmark's screens stay without context.
 */
export const contextMarkup = `<screen id="login" label="Sign in" weft="0.2">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Returning users sign in with email and password.</entry>
    <entry id="submit-disabled" by="agent" for="go" kind="decision" name="claude-opus-5-5">Disabled until an email is typed, so auth.submit never gets an empty request.</entry>
    <entry id="reset-where" by="agent" for="reset" kind="question" name="claude-opus-5-5" status="open">Should reset open a dialog or a screen of its own?</entry>
  </context>
  <form id="f1" state="idle" on-submit="auth.submit">
    <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
    <button id="go" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
    <slot name="footer">
      <link id="reset" on-press="nav.reset">Forgot password?</link>
    </slot>
  </form>
</screen>
`;

export function parseStrict(markup: string): Document {
  const { document, diagnostics } = parse(markup, { catalog: coreCatalog, mode: "strict" });
  if (document === undefined || diagnostics.length > 0)
    throw new Error(`invalid test markup: ${JSON.stringify(diagnostics)}`);
  return document;
}
