import type { Diagnostic } from "@weft/core";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";

export const text = (value: string): CallToolResult => ({
  content: [{ type: "text", text: value }],
});

export const failure = (value: string): CallToolResult => ({
  content: [{ type: "text", text: value }],
  isError: true,
});

/** Diagnostics as compact JSON; a flood is cut so one bad call cannot fill the model's context. */
export function diagnosticsText(diagnostics: readonly Diagnostic[], limit: number): string {
  const shown = diagnostics.slice(0, limit);
  const omitted = diagnostics.length - shown.length;
  return JSON.stringify(omitted > 0 ? { diagnostics: shown, omitted } : { diagnostics: shown });
}

/** The tool contract is "never throw": whatever escapes a handler becomes an error result. */
export function guarded<A>(run: (args: A) => CallToolResult): (args: A) => CallToolResult {
  return (args) => {
    try {
      return run(args);
    } catch (error) {
      return failure(`Internal error: ${error instanceof Error ? error.message : String(error)}`);
    }
  };
}
