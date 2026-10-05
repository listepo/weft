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

/** A whole page, as the static HTML generator writes it. */
export async function showPage(id: string, html: string): Promise<void> {
  await settle(await frame(id, html));
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
