// Shared data contracts from SPEC.md §3, §5, §6.1 and §7. Every package builds on these;
// change them only together with the specification.
import { z } from "zod";

export const WEFT_VERSION = "0.2";

export const BindingSchema = z.strictObject({ bind: z.string(), not: z.literal(true).optional() });
export const TokenRefSchema = z.strictObject({ token: z.string() });
export const ValueSchema = z.union([
  z.string(),
  z.number(),
  z.boolean(),
  BindingSchema,
  TokenRefSchema,
]);

export type Binding = z.infer<typeof BindingSchema>;
export type TokenRef = z.infer<typeof TokenRefSchema>;
export type Value = z.infer<typeof ValueSchema>;

export type Node = {
  kind: string;
  id?: string | undefined;
  props?: Record<string, Value> | undefined;
  on?: Record<string, string> | undefined;
  slots?: Record<string, Child[]> | undefined;
  children?: Child[] | undefined;
};
export type Child = Node | string;

export const NodeSchema: z.ZodType<Node> = z.lazy(() =>
  z.strictObject({
    kind: z.string(),
    id: z.string().optional(),
    props: z.record(z.string(), ValueSchema).optional(),
    on: z.record(z.string(), z.string()).optional(),
    slots: z.record(z.string(), z.array(ChildSchema)).optional(),
    children: z.array(ChildSchema).optional(),
  }),
);
export const ChildSchema: z.ZodType<Child> = z.lazy(() => z.union([NodeSchema, z.string()]));

// A context entry (SPEC §2.3). Every member is a string here, so that validation, not the shape
// check, names a wrong kind, author or status (W203, W227).
export const EntrySchema = z.strictObject({
  id: z.string().optional(),
  kind: z.string(),
  by: z.string(),
  name: z.string(),
  for: z.string().optional(),
  status: z.string().optional(),
  text: z.string(),
});
export type Entry = z.infer<typeof EntrySchema>;

export const DocumentSchema = z.strictObject({
  weft: z.string(),
  context: z.array(EntrySchema).optional(),
  /** Inline fragments of a screen, by name (SPEC §10.7). Each value is the fragment node. */
  fragments: z.record(z.string(), NodeSchema).optional(),
  root: NodeSchema,
});
export type Document = z.infer<typeof DocumentSchema>;

export const ContentModelSchema = z.enum(["none", "text", "nodes", "mixed"]);

export const PropDefSchema = z.strictObject({
  description: z.string(),
  type: z.enum(["string", "number", "boolean", "enum", "token"]),
  values: z.array(z.string()).optional(),
  tokenType: z.string().optional(),
  min: z.number().optional(),
  max: z.number().optional(),
  integer: z.boolean().optional(),
  required: z.boolean().optional(),
  default: z.union([z.string(), z.number(), z.boolean()]).optional(),
  bindable: z.boolean().optional(),
  writable: z.boolean().optional(),
  // A string prop whose literal value is the id of an element of this kind (SPEC §6, W309).
  references: z.string().optional(),
});
export type PropDef = z.infer<typeof PropDefSchema>;

export const SlotDefSchema = z.strictObject({
  description: z.string(),
  allowedChildren: z.array(z.string()).optional(),
  required: z.boolean().optional(),
});
export type SlotDef = z.infer<typeof SlotDefSchema>;

export const ComponentDefSchema = z.strictObject({
  description: z.string(),
  role: z.string(),
  content: ContentModelSchema,
  allowedChildren: z.array(z.string()).optional(),
  allowedParents: z.array(z.string()).optional(),
  // SPEC §5.1 "needs label": the universal `label` attribute is mandatory for this component.
  requiresLabel: z.boolean().optional(),
  // The one kind the document root must be, and that may stand nowhere else (SPEC §6, W201, W312).
  root: z.boolean().optional(),
  props: z.record(z.string(), PropDefSchema).optional(),
  slots: z.record(z.string(), SlotDefSchema).optional(),
  states: z.array(z.string()).optional(),
  events: z.array(z.string()).optional(),
});
export type ComponentDef = z.infer<typeof ComponentDefSchema>;

export const CatalogSchema = z.strictObject({
  weft: z.string(),
  name: z.string(),
  version: z.string(),
  components: z.record(z.string(), ComponentDefSchema),
  /** A project's fragments by name (SPEC §10.7), each a document whose root is `fragment`. */
  fragments: z.record(z.string(), DocumentSchema).optional(),
});
export type Catalog = z.infer<typeof CatalogSchema>;

export const DiagnosticSchema = z.strictObject({
  code: z.string(),
  severity: z.enum(["error", "warning"]),
  message: z.string(),
  path: z.string(),
  line: z.number().optional(),
  column: z.number().optional(),
  expected: z.string().optional(),
  got: z.string().optional(),
  hint: z.string().optional(),
});
export type Diagnostic = z.infer<typeof DiagnosticSchema>;

const fragment = z.string().optional();
export const PatchSchema = z.discriminatedUnion("op", [
  z.strictObject({
    op: z.literal("set"),
    id: z.string(),
    fragment,
    prop: z.string(),
    value: ValueSchema.nullable(),
  }),
  z.strictObject({
    op: z.literal("insert"),
    parent: z.string(),
    fragment,
    slot: z.string().optional(),
    index: z.number().int().nonnegative().optional(),
    markup: z.string(),
  }),
  z.strictObject({ op: z.literal("remove"), id: z.string(), fragment }),
  z.strictObject({
    op: z.literal("move"),
    id: z.string(),
    fragment,
    parent: z.string(),
    slot: z.string().optional(),
    index: z.number().int().nonnegative().optional(),
  }),
  z.strictObject({ op: z.literal("add-fragment"), markup: z.string() }),
  z.strictObject({ op: z.literal("remove-fragment"), name: z.string() }),
  z.strictObject({ op: z.literal("add-context"), entry: EntrySchema }),
  z.strictObject({
    op: z.literal("set-context"),
    id: z.string(),
    field: z.enum(["text", "kind", "for"]),
    value: z.string().nullable(),
  }),
  z.strictObject({ op: z.literal("resolve-context"), id: z.string() }),
  z.strictObject({ op: z.literal("remove-context"), id: z.string() }),
]);
export type Patch = z.infer<typeof PatchSchema>;
