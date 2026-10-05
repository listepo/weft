// The narrow API of `src/api.ts` must be a true subset of Penpot's plugin API: the official types
// are assigned to it here, so `tsc` fails if the subset drifts.
import assert from "node:assert/strict";
import type {
  Board,
  Group,
  LibraryComponent,
  LibraryVariantComponent,
  Page,
  Penpot,
  Rectangle,
  Shape,
  Text,
  Token,
  TokenCatalog,
  TokenSet,
  VariantContainer,
  Variants,
} from "@penpot/plugin-types";
import { test } from "vitest";
import type {
  PBoard,
  PenpotApi,
  PGroup,
  PLibraryComponent,
  PPage,
  PRectangle,
  PShape,
  PText,
  PToken,
  PTokenCatalog,
  PTokenSet,
  PVariantComponent,
  PVariantContainer,
  PVariants,
} from "../src/api.ts";

type Assignable<To, From extends To> = [To, From];

export type Checks = [
  Assignable<PenpotApi, Penpot>,
  Assignable<PShape, Shape>,
  Assignable<PBoard, Board>,
  Assignable<PVariantContainer, VariantContainer>,
  Assignable<PText, Text>,
  Assignable<PGroup, Group>,
  Assignable<PRectangle, Rectangle>,
  Assignable<PPage, Page>,
  Assignable<PLibraryComponent, LibraryComponent>,
  Assignable<PVariantComponent, LibraryVariantComponent>,
  Assignable<PVariants, Variants>,
  Assignable<PToken, Token>,
  Assignable<PTokenSet, TokenSet>,
  Assignable<PTokenCatalog, TokenCatalog>,
];

test("the narrow API is checked by the type checker", () => {
  // The assertions above run in `tsc`; this keeps the file a valid Vitest suite.
  assert.ok(true);
});
