// Diagnostic cases shared by diagnostics.test.ts and the TypeScript–Rust differential check.
import type { DiagnosticCode, ValidateOptions } from "../src/index.ts";
import { tokens } from "./catalog.ts";

export const screen = (inner: string, weft = "0.1") =>
  `<screen id="s" weft="${weft}">${inner}</screen>`;
export const doc = (child: unknown) => ({
  weft: "0.1",
  root: { kind: "screen", id: "s", children: [child] },
});

export type Case = {
  markup?: string;
  json?: unknown;
  options?: Pick<ValidateOptions, "tokens" | "actions">;
};

// The W5xx patch codes need a patch list, not a document; patch.test.ts has their cases. The
// W6xx import codes need an importer; packages/from-aria tests them. W315 and W316 need a data
// schema (data.test.ts), and the W7xx project codes a project (packages/catalog project tests).
export type DocumentCode = Exclude<
  DiagnosticCode,
  `W5${string}` | `W6${string}` | `W7${string}` | "W315" | "W316"
>;
export const cases: Record<DocumentCode, Case> = {
  W101: { markup: screen('<text id="t" tone="muted"value="x"/>') },
  W102: { markup: `<?xml version="1.0"?>${screen("")}` },
  W103: { markup: `<!DOCTYPE screen>${screen("")}` },
  W104: { markup: screen('<text id="t"><![CDATA[x]]></text>') },
  W105: { markup: screen('<Button id="b">x</Button>') },
  W106: { markup: screen("<text id='t'>x</text>") },
  W107: { markup: screen('<field id="f" label="L" required/>') },
  W108: { markup: screen('<text id="t" tone="muted" tone="danger">x</text>') },
  W109: { markup: screen('<form id="f"><text id="t">x</form></text>') },
  W110: { markup: '<screen id="s" weft="0.1"><form id="f">' },
  W111: { markup: `${screen("")}</form>` },
  W112: { markup: screen('<text id="t">a &nbsp; b</text>') },
  W113: { markup: screen('<text id="t" text="a<b"/>') },
  W114: { markup: `hello ${screen("")}` },
  W115: { markup: screen("<!-- a -- b -->") },
  W116: { markup: screen('<text id="t" text="{oops}"/>') },
  W117: { markup: screen(`${'<stack id="x">'.repeat(300)}${"</stack>".repeat(300)}`) },
  W118: { markup: screen('<form id="f"><slot/></form>') },
  W119: {
    markup: screen(
      '<form id="f"><slot name="footer"><text id="a">a</text></slot><slot name="footer"><text id="b">b</text></slot></form>',
    ),
  },
  W200: { json: { weft: "0.1", root: { kind: "screen", id: "s", children: [{ id: 1 }] } } },
  W201: { markup: '<form id="f" weft="0.1"/>' },
  W202: { markup: screen("<text>x</text>") },
  W203: { markup: screen('<button id="b" variant="primay">x</button>') },
  W204: { markup: screen('<heading id="h" level="two">x</heading>') },
  W205: { markup: screen('<heading id="h">x</heading>') },
  W206: { markup: screen('<text id="t" on-press="a.b">x</text>') },
  W207: { markup: screen('<form id="f"><slot name="header"><text id="t">x</text></slot></form>') },
  W208: { markup: screen('<dialog id="d" label="L"><text id="t">x</text></dialog>') },
  W209: { markup: screen('<button id="b" role="link">x</button>') },
  W210: { markup: screen('<x-acme-map id="m"/>') },
  W211: { markup: screen('<x-acme-map id="m" role="mapp"/>') },
  W212: { markup: screen('<text id="1t">x</text>') },
  W213: { markup: screen('<text id="t" text="Hello {$.name}"/>') },
  W214: { markup: screen('<text id="t" text="{$..a}"/>') },
  W215: { markup: screen('<stack id="st" gap="{token.}"/>') },
  W216: { markup: screen('<button id="b" on-press="Bad Action">x</button>') },
  W217: { markup: screen('<x-acme-map id="m" role="{$.role}"/>') },
  W218: { markup: screen('<field id="f" label="L" value="{!$.v}"/>') },
  W219: { markup: screen("", "one") },
  W220: { markup: screen('<x-map id="m" role="img"/>') },
  W221: { json: doc({ kind: "text", id: "t", children: [`a${String.fromCharCode(1)}`] }) },
  W222: {
    markup: screen(
      '<list id="l"><each id="e" as="item" in="$.items"><item id="i">x</item></each></list>',
    ),
  },
  W223: { json: doc({ kind: "text", id: "t", props: { "on-press": "a.b" } }) },
  W224: { markup: screen('<heading id="h" level="7">x</heading>') },
  W301: { markup: screen('<text id="t">a</text><text id="t">b</text>') },
  W302: { markup: screen('<list id="l"><text id="t">x</text></list>') },
  W303: { markup: screen('<item id="i">x</item>') },
  W304: { markup: screen('<button id="b"><text id="t">x</text></button>') },
  W305: { markup: screen('<text id="t" text="{$todo.title}"/>') },
  W306: { markup: screen('<stack id="st" gap="{token.space.xl}"/>'), options: { tokens } },
  W307: { markup: screen('<stack id="st" gap="{token.color.accent}"/>'), options: { tokens } },
  W308: {
    markup: screen('<button id="b" on-press="auth.sumbit">x</button>'),
    options: { actions: ["auth.submit"] },
  },
  W309: { markup: screen('<tabs id="tb" selected="nope"><tab id="a" label="A"/></tabs>') },
  W310: { markup: screen('<link id="l" text="{$.cta}" on-press="a.b">y</link>') },
  W311: {
    markup: screen(
      '<list id="l"><each id="e1" as="row" in="{$.rows}"><each id="e2" as="row" in="{$row.items}"><item id="i">x</item></each></each></list>',
    ),
  },
  W312: { markup: screen('<screen id="inner" weft="0.1"/>') },
  W313: { markup: screen('<button id="b" submit="true">Send</button>') },
  W314: { markup: screen('<list id="l"><each id="e" as="row" in="{$.rows}"/></list>') },
  W401: { markup: screen('<fancy id="f"/>') },
  W402: { markup: screen('<text id="t" colour="red">x</text>') },
  W403: { markup: screen("", "0.2") },
  W404: { markup: screen("", "1.0") },
};
