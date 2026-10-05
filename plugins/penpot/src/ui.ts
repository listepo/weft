// The plugin's UI, a page Penpot shows in an iframe (`@weft/design-plugin`). Penpot passes
// messages as they are, with no wrapper; only messages from Penpot's window are taken.
import { startUi } from "@weft/design-plugin";

startUi({
  send: (request) => parent.postMessage(request, "*"),
  listen: (onReply) => {
    window.addEventListener("message", (event: MessageEvent) => {
      if (event.source === parent) onReply(event.data);
    });
  },
});
