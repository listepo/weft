// Renders a target into an iframe of its own: every target gets a fresh document with only the
// browser's default styles, the same host page a static page gets, so the screenshots differ only
// where the targets do. The iframe is 480 px wide and as tall as its content.
import { commands } from "vitest/browser";
import { createElement, type ComponentType } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { createComponent } from "solid-js";
import { render as renderSolid } from "solid-js/web";
import type { BaselineResult } from "../../src/baseline.ts";

const EMPTY =
  '<!doctype html><html lang="en"><head><meta charset="utf-8"></head><body></body></html>';

type Props = { data?: unknown };

const frames = () =>
  new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
  );

async function settle(frame: HTMLIFrameElement): Promise<void> {
  const doc = frame.contentDocument;
  if (doc === null) throw new Error(`#${frame.id} has no document`);
  await doc.fonts.ready;
  await frames();
  frame.style.height = `${doc.documentElement.scrollHeight}px`;
  await frames();
}

/** The empty page with `head` added to its head, such as a stylesheet a component reads. */
const withHead = (head: string) => EMPTY.replace("</head>", `${head}</head>`);

/** A fresh iframe `#id`, loaded with `html`; the previous one of that id is removed. */
async function frame(id: string, html = EMPTY): Promise<HTMLIFrameElement> {
  document.getElementById(id)?.remove();
  const el = document.createElement("iframe");
  el.id = id;
  el.style.cssText =
    "display:block;width:480px;height:100px;border:0;margin:0 0 16px;background:#fff";
  const loaded = new Promise((resolve) => el.addEventListener("load", resolve, { once: true }));
  el.srcdoc = html;
  document.body.append(el);
  await loaded;
  return el;
}

/** The container a component or markup goes in, inside the iframe's body. */
function host(el: HTMLIFrameElement): HTMLElement {
  const doc = el.contentDocument;
  if (doc === null) throw new Error(`#${el.id} has no document`);
  const div = doc.createElement("div");
  doc.body.append(div);
  return div;
}

/**
 * What a frosted-glass screen (T51) is shown over: a glass material blurs what is behind it, and
 * plain white has nothing to blur. Diagonal stripes in three colours turn into a visible smear
 * under a 16 to 24 px blur and stay sharp where no glass covers them; the dark scheme gets darker
 * stripes, so its tint is judged against a dark backdrop too.
 */
export const BACKDROP = `<style>
html { background: repeating-linear-gradient(135deg, #e11d48 0 28px, #2563eb 28px 56px, #facc15 56px 84px); }
body { padding: 24px; }
@media (prefers-color-scheme: dark) {
  html { background: repeating-linear-gradient(135deg, #881337 0 28px, #1e3a8a 28px 56px, #854d0e 56px 84px); }
}
</style>`;

/** A whole page, as the static HTML generator writes it, with `head` added to its head. */
export async function showPage(id: string, html: string, head = ""): Promise<void> {
  await settle(await frame(id, head === "" ? html : html.replace("</head>", `${head}</head>`)));
}

/** Server-rendered markup, as the reference renderer writes it. */
export async function showMarkup(id: string, markup: string, head = ""): Promise<HTMLElement> {
  const el = await frame(id, withHead(head));
  const div = host(el);
  div.innerHTML = markup;
  await settle(el);
  return div;
}

export async function showReact(
  id: string,
  component: ComponentType<Props>,
  props: Props,
  head = "",
) {
  const el = await frame(id, withHead(head));
  const div = host(el);
  flushSync(() => createRoot(div).render(createElement(component, props)));
  await settle(el);
}

export async function showSolid(
  id: string,
  component: (props: Props) => unknown,
  props: Props,
  head = "",
) {
  const el = await frame(id, withHead(head));
  renderSolid(() => createComponent(component as never, props), host(el));
  await settle(el);
}

/**
 * A generated Lit element. Custom elements are registered per window, so the iframe loads the
 * module itself; the module's URL is Vite's for a virtual module (`/@id/__x00__` is its `\0`).
 */
export async function showLit(
  id: string,
  name: string,
  props: Props,
  head = "",
): Promise<HTMLElement> {
  const el = await frame(id, withHead(head));
  const win = el.contentWindow;
  const doc = el.contentDocument;
  if (win === null || doc === null) throw new Error(`#${id} has no window`);
  const url = `/@id/__x00__virtual:weft-component/lit/${name}`;
  const ready = new Promise<void>((resolve, reject) => {
    win.addEventListener("weft-ready", () => resolve(), { once: true });
    win.addEventListener("unhandledrejection", (e) => reject(e.reason), { once: true });
  });
  const script = doc.createElement("script");
  script.type = "module";
  script.textContent = `import ${JSON.stringify(url)};
const host = document.createElement("div");
const screen = document.createElement("weft-screen");
screen.data = ${JSON.stringify(props.data ?? null)};
host.append(screen);
document.body.append(host);
await screen.updateComplete;
dispatchEvent(new Event("weft-ready"));`;
  doc.head.append(script);
  await ready;
  await settle(el);
  const screen = doc.querySelector<HTMLElement>("weft-screen");
  if (screen === null) throw new Error(`#${id} has no element`);
  return screen;
}

/** Re-measures `#id` after its content was changed in place. */
export async function refit(id: string): Promise<void> {
  const el = document.getElementById(id);
  if (!(el instanceof HTMLIFrameElement)) throw new Error(`no iframe #${id}`);
  await settle(el);
}

/**
 * Fails unless the element matches its reviewed baseline; skips where this platform has no
 * reviewed baselines at all (font rendering differs between operating systems).
 */
export async function expectBaseline(
  selector: string,
  name: string,
  skip: (note: string) => void,
): Promise<void> {
  const result: BaselineResult = await commands.matchScreenshot(selector, name);
  if (result.status === "missing") {
    if (result.reviewed) {
      throw new Error(`no baseline ${name}: take it with WEFT_UPDATE_SCREENSHOTS=1 and review it`);
    }
    skip(`no reviewed baselines for ${result.platform}`);
    return;
  }
  if (result.status === "differ") {
    const { differing, diff } = result.comparison;
    throw new Error(`${name}: ${differing} pixels differ from the baseline; see ${diff}`);
  }
}

/** Fails unless the two elements look the same. */
export async function expectSameLook(expected: string, actual: string, label: string) {
  const { differing, diff } = await commands.compareElements(expected, actual, label);
  if (differing > 0) throw new Error(`${label}: ${differing} pixels differ; see ${diff}`);
}
