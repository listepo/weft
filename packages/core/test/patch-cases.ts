// Patch cases shared by patch.test.ts and the TypeScript–Rust differential check.
import type { Document } from "../src/index.ts";

export const BASE = `<screen id="s" weft="0.1">
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

// One patch list per patch code; the registry test below keeps this table complete.
export const failures: Record<string, [unknown, Document?]> = {
  W501: [[{ op: "explode" }]],
  W502: [[{ op: "remove", id: "emial" }]],
  W503: [[{ op: "set", id: "go", prop: "id", value: "x" }]],
  W504: [[{ op: "insert", parent: "main", slot: "footer", markup: '<text id="t">x</text>' }]],
  W505: [[{ op: "insert", parent: "main", index: 99, markup: '<text id="t">x</text>' }]],
  W506: [[{ op: "move", id: "f", parent: "go" }]],
  W507: [[{ op: "remove", id: "s" }]],
  W508: [[{ op: "insert", parent: "main", markup: "just text" }]],
  W509: [[{ op: "insert", parent: "main", markup: '<text id="go">x</text>' }]],
};
