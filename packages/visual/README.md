# @weft/visual

Screenshot tests for every corpus screen (`corpus/`), on every target that draws pixels. Private; nothing imports it.

- **Web** (`test/web/`, Vitest browser mode on Playwright's Chromium): the generated React and SolidJS components, the reference renderer and the static HTML page each render the screen's `data.json` in an iframe of their own. Each screen is compared with its reviewed baseline, across targets (SolidJS and the reference renderer must look exactly like React, with the same accessibility tree), and across round trips through the HTML, React and SolidJS importers and the Figma and Penpot fakes.
- **Light and dark** (`test/web/appearance.test.ts`): the example project's review screen, whose tokens come from a resolver with a light and a dark theme, under each `prefers-color-scheme`. The static page and React with the `weft css-tokens` stylesheet are compared with their baselines (`example-review`, `example-review-dark`), and the reference renderer drawn with the same theme must look like React. The SwiftUI suite takes the same screen in both appearances.
- **SwiftUI** (`test/swiftui.test.ts`, Node): the generated views, each showing its screen's `data.json` as the model's `sample`, are built into one app and screenshotted in the iOS Simulator, then compared with their reviewed baselines.
- **Mutations**: the same comparisons must fail for a screen nudged by one pixel (one point in SwiftUI) or with one word changed.

Known differences are listed in the tests with the reason, as `test.fails`: a listed difference that goes away fails the suite until its entry is removed.

## Running

`moon run visual:test` builds the WebAssembly core and the `weft` CLI first; the static page, the convention importers and the SwiftUI generator are reached through the CLI.

Without Chromium (`pnpm exec playwright install chromium`), or without macOS, Xcode and the pinned simulator (iPhone 17, iOS 27.0; see `test/swiftui.test.ts`), those screenshots are skipped with a message saying why.

## Baselines

Baselines live in `baselines/<platform>-<arch>/` (`web/`, `html/`, `swiftui/`), because fonts render differently on each operating system. A platform without a baselines folder skips the baseline comparisons; the comparisons across targets and round trips still run.

A failed comparison writes `diffs/<label>.diff.png` with the expected and actual images beside it (`diffs/` is not committed).

To take new baselines, run the suite with `WEFT_UPDATE_SCREENSHOTS=1`, then look at every changed image before committing it.
