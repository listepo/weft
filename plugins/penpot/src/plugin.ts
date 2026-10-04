// The plugin's sandbox code, run by Penpot in an SES compartment. It only wires the `penpot`
// global to the handler in @weft/penpot; parsing and serializing happen in the UI (`ui.ts`).
import type {} from "@penpot/plugin-types";
import { coreCatalog } from "@weft/catalog";
import { handleRequest } from "@weft/penpot";

// Resolved against the folder of the manifest, where the build puts the page.
penpot.ui.open("Weft", "ui.html", { width: 400, height: 560 });

penpot.ui.onMessage<unknown>(async (message) => {
  const reply = await handleRequest(penpot, penpot.selection, message, { catalog: coreCatalog });
  if (reply.type === "built") {
    const shape = penpot.currentPage?.getShapeById(reply.id);
    if (shape !== null && shape !== undefined) {
      penpot.selection = [shape];
      penpot.viewport.zoomIntoView([shape]);
    }
  }
  penpot.ui.sendMessage(reply);
});
