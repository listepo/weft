// The visual suite's own config: the web screenshots run in Vitest browser mode on Chromium
// through Playwright, the SwiftUI screenshots in Node. Without Chromium the web project is
// replaced by one skipped test that says how to install it, so the suite still runs.
import { existsSync } from "node:fs";
import { playwright } from "@vitest/browser-playwright";
import { chromium } from "playwright";
import { defineConfig, type TestProjectInlineConfiguration } from "vitest/config";
import { commands } from "./src/commands.ts";
import { weftScreens } from "./src/plugin.ts";

const hasChromium = existsSync(chromium.executablePath());

const web: TestProjectInlineConfiguration = hasChromium
  ? {
      plugins: [weftScreens()],
      // Found up front, so Vite does not reload the page halfway through to bundle a dependency
      // a compiled screen imports.
      optimizeDeps: {
        include: [
          "react",
          "react/jsx-runtime",
          "react-dom/client",
          "solid-js",
          "solid-js/web",
          "lit",
          "lit/static-html.js",
          "lit/directives/style-map.js",
        ],
      },
      test: {
        name: "web",
        include: ["test/web/**/*.test.ts"],
        browser: {
          enabled: true,
          headless: true,
          // Every host but the test server fails to resolve, so a screen's external image is a
          // broken image every time instead of whatever the network returns.
          provider: playwright({
            launchOptions: { args: ["--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost"] },
          }),
          instances: [{ browser: "chromium" }],
          // Large enough that every screen fits without scrolling the tester frame.
          viewport: { width: 1280, height: 2400 },
          commands,
          screenshotFailures: false,
        },
      },
    }
  : { test: { name: "web", include: ["test/no-chromium.test.ts"] } };

export default defineConfig({
  test: {
    // Two 60s shots (commands.ts). The projects run one after the other (moon.yml), so
    // this only has to cover a slow shot, not a simulator build beside it.
    testTimeout: 150_000,
    hookTimeout: 300_000,
    projects: [
      web,
      // The simulator helpers' unit tests need no Xcode, so they run beside the screenshots.
      {
        test: {
          name: "swiftui",
          include: ["test/swiftui.test.ts", "test/simulator.test.ts", "test/compare.test.ts"],
        },
      },
    ],
  },
});
