import { defineConfig } from "vite";

// Penpot fetches the manifest and plugin code cross-origin from its own site, and Vite's default
// CORS allows only localhost origins.
export default defineConfig({
  preview: { port: 4400, strictPort: true, cors: true },
});
