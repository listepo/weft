// The narrow API of `src/api.ts` must be a true subset of the official Plugin API: the real
// `figma` object and its nodes are assigned to it here, so `tsc` fails if the subset drifts.
import assert from "node:assert/strict";
import type {
  ComponentNode,
  ComponentSetNode,
  FrameNode,
  InstanceNode,
  PageNode,
  PluginAPI,
  RectangleNode,
  SectionNode,
  SceneNode,
  TextNode,
  Variable,
  VariableCollection,
} from "@figma/plugin-typings/plugin-api-standalone.js";
import { test } from "vitest";
import type {
  FCollection,
  FComponent,
  FComponentSet,
  FFrame,
  FigmaApi,
  FInstance,
  FNode,
  FPage,
  FRectangle,
  FSection,
  FText,
  FVariable,
} from "../src/api.ts";

type Assignable<To, From extends To> = [To, From];

export type Checks = [
  Assignable<FigmaApi, PluginAPI>,
  Assignable<FNode, SceneNode>,
  Assignable<FFrame, FrameNode>,
  Assignable<FComponent, ComponentNode>,
  Assignable<FComponentSet, ComponentSetNode>,
  Assignable<FInstance, InstanceNode>,
  Assignable<FText, TextNode>,
  Assignable<FRectangle, RectangleNode>,
  Assignable<FSection, SectionNode>,
  Assignable<FPage, PageNode>,
  Assignable<FVariable, Variable>,
  Assignable<FCollection, VariableCollection>,
];

test("the narrow API is checked by the type checker", () => {
  // The assertions above run in `tsc`; this keeps the file a valid Vitest suite.
  assert.ok(true);
});
