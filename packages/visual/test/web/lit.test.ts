// What the Lit element does once it is on the page: the properties it takes (`data`, `actions`,
// `onChange`) are the ones the React and SolidJS components take, and a click or an input reaches
// them the same way.
import { expect, test } from "vitest";
import { screens } from "virtual:weft-screens";
import { showLit } from "./stage.ts";

type Screen = HTMLElement & {
  updateComplete: Promise<boolean>;
  data: unknown;
  actions: Record<string, (event: unknown) => void>;
  onChange: (path: string, value: unknown) => void;
};

const dataOf = (name: string) => screens.find((s) => s.name === name)?.data;

test("a pressed button reports its action", async () => {
  const el = (await showLit("lit-press", "confirm-dialog", {
    data: dataOf("confirm-dialog"),
  })) as Screen;
  const events: unknown[] = [];
  el.actions = { "dialog.open": (event) => events.push(event) };
  await el.updateComplete;
  el.querySelector<HTMLElement>('[data-weft-id="delete-file"]')?.click();
  expect(events).toEqual([{ id: "delete-file", action: "dialog.open" }]);
});

test("a typed value is written to its path, then the screen is drawn from the new data", async () => {
  const el = (await showLit("lit-type", "booking", { data: dataOf("booking") })) as Screen;
  const writes: unknown[] = [];
  el.onChange = (path, value) => writes.push([path, value]);
  await el.updateComplete;
  const input = el.querySelector<HTMLInputElement>('[data-weft-id="city"]');
  if (input === null) throw new Error("no city field");
  input.value = "Lisbon";
  input.dispatchEvent(new Event("input", { bubbles: true }));
  expect(writes).toEqual([["$.trip.city", "Lisbon"]]);

  el.data = { ...(dataOf("booking") as object), trip: { city: "Porto" } };
  await el.updateComplete;
  expect(input.value).toBe("Porto");
});
