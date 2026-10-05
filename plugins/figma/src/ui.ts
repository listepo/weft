// The plugin's UI, a browser page Figma shows in an iframe (`@weft/design-plugin`). Figma wraps
// each message in `pluginMessage` both ways.
import { startUi } from "@weft/design-plugin";

startUi({
  send: (pluginMessage) => parent.postMessage({ pluginMessage }, "*"),
  listen: (onReply) => {
    window.onmessage = (event: MessageEvent) =>
      onReply((event.data as { pluginMessage?: unknown } | null)?.pluginMessage);
  },
});
