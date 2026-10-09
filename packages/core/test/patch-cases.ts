// Patch cases shared by patch.test.ts and the TypeScript–Rust differential check.
import type { ApplyOptions, Document } from "../src/index.ts";

/** What a host sets beside the catalog: who may write context, and whether anyone may. */
export type HostOptions = Pick<ApplyOptions, "author" | "context">;

export const AGENT = { by: "agent", name: "m" } as const;

export const ENTRY = {
  id: "n",
  kind: "decision",
  by: "agent",
  name: "m",
  for: "go",
  text: "Primary, because it is the one action.",
};

export const BASE = `<screen id="s" weft="0.2">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Sign-in for returning users.</entry>
    <entry id="reset-where" by="agent" for="email" kind="question" name="m" status="open">Dialog or screen?</entry>
  </context>
  <stack id="main">
    <field id="email" label="Email" type="email" value="{$.email}"/>
    <field id="pw" label="Password" type="password"/>
    <form id="f" on-submit="auth.submit">
      <button id="go" variant="primary" on-press="auth.submit">Sign in</button>
      <slot name="footer">
        <link id="reset" on-press="nav.reset">Forgot password?</link>
      </slot>
    </form>
    <list id="l">
      <item id="i1">One</item>
      <item id="i2">Two</item>
      <each id="e" as="x" in="{$.xs}">
        <item id="i3">Three</item>
      </each>
    </list>
    <dialog id="dlg" label="Sure?">
      <slot name="actions">
        <button id="ok">OK</button>
      </slot>
    </dialog>
    <x-acme-box id="box" role="group"/>
  </stack>
</screen>
`;

// One patch list per patch code, with the base and the host options when they matter; the
// registry test below keeps this table complete.
export const failures: Record<string, [unknown, (Document | undefined)?, HostOptions?]> = {
  W501: [[{ op: "explode" }]],
  W502: [[{ op: "remove", id: "emial" }]],
  W503: [[{ op: "set", id: "go", prop: "id", value: "x" }]],
  W504: [[{ op: "insert", parent: "main", slot: "footer", markup: '<text id="t">x</text>' }]],
  W505: [[{ op: "insert", parent: "main", index: 99, markup: '<text id="t">x</text>' }]],
  W506: [[{ op: "move", id: "f", parent: "go" }]],
  W507: [[{ op: "remove", id: "s" }]],
  W508: [[{ op: "insert", parent: "main", markup: "just text" }]],
  W509: [[{ op: "insert", parent: "main", markup: '<text id="go">x</text>' }]],
  W510: [[{ op: "add-context", entry: { ...ENTRY, id: "why" } }]],
  W511: [[{ op: "resolve-context", id: "reset-were" }]],
  W512: [[{ op: "add-context", entry: { ...ENTRY, by: "human" } }], undefined, { author: AGENT }],
};
