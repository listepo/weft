# @weft/visual

Screenshot tests for every corpus screen (`corpus/`), on every target that draws pixels. Private; nothing imports it.

- **Web** (`test/web/`, Vitest browser mode on Playwright's Chromium): the generated React and SolidJS components, the reference renderer and the static HTML page each render the screen's `data.json` in an iframe of their own; the components and the reference renderer get the `weft css-tokens` stylesheet in their head, as a host gives it, and the static page must look exactly like React with the same accessibility tree. Each screen is compared with its reviewed baseline, across targets (SolidJS and the reference renderer must look exactly like React, with the same accessibility tree), and across round trips through the HTML, React and SolidJS importers and the Figma and Penpot fakes.
- **Light and dark** (`test/web/appearance.test.ts`): the example project's review screen, whose tokens come from a resolver with a light and a dark theme, under each `prefers-color-scheme`. The static page and React with the `weft css-tokens` stylesheet are compared with their baselines (`example-review`, `example-review-dark`), and the reference renderer drawn with the same theme must look like React. The SwiftUI suite takes the same screen in both appearances.
- **SwiftUI** (`test/swiftui.test.ts`, Node): the generated views, each showing its screen's `data.json` as the model's `sample`, are built into one app and screenshotted in the iOS Simulator, then compared with their reviewed baselines.
- **Mutations**: the same comparisons must fail for a screen nudged by one pixel (one point in SwiftUI) or with one word changed.

Known differences are listed in the tests with the reason, as `test.fails`: a listed difference that goes away fails the suite until its entry is removed.

## Running

`moon run visual:test` builds the WebAssembly core and the `weft` CLI first; the static page, the convention importers and the SwiftUI generator are reached through the CLI.

Without Chromium (`pnpm exec playwright install chromium`), or without macOS, Xcode and the pinned simulator (iPhone 17, iOS 27.0; see `test/swiftui.test.ts`), those screenshots are skipped with a message saying why.

### Simulator sharing

The SwiftUI suite installs one app id on one iOS Simulator, so two runs on the same device would overwrite each other's app and screenshots and fail with false pixel diffs. `WEFT_SIMULATOR` chooses how runs from different worktrees avoid that:

- `shared` (default): every run uses the same simulator and takes a machine-wide lock for the whole suite, so runs take turns. The lock is `~/Library/Caches/weft-visual/simulator.lock` (the simulators belong to the user, and `TMPDIR` differs between sessions, so a temp folder could give two runs two locks). A run that finds it held prints the holder's process id and waits up to 30 minutes, then fails and names the lock file. A lock whose process is gone is taken over.
- `own`: each worktree gets a simulator of its own, the same device type and runtime, created with `xcrun simctl create` on the first run and reused afterwards. It is named `weft-visual-<worktree folder>`, e.g. `weft-visual-weft-t46`. Runs in different worktrees never share state and do not wait for each other; two runs of one worktree still take turns, through `~/Library/Caches/weft-visual/<device name>.lock`. Each device takes a few hundred megabytes once booted, and a fresh one is slower on its first run.

```bash
WEFT_SIMULATOR=own moon run visual:test

xcrun simctl list devices | grep weft-visual-   # the devices the suite created
xcrun simctl delete weft-visual-weft-t46        # one of them, by name
```

The suite never deletes a device: remove the device of a worktree when you remove the worktree. Shared mode ignores the `weft-visual-` devices.

## Baselines

Baselines live in `baselines/<platform>-<arch>/` (`web/`, `html/`, `swiftui/`), because fonts render differently on each operating system. A platform without a baselines folder skips the baseline comparisons; the comparisons across targets and round trips still run.

A failed comparison writes `diffs/<label>.diff.png` with the expected and actual images beside it (`diffs/` is not committed).

To take new baselines, run the suite with `WEFT_UPDATE_SCREENSHOTS=1`, then look at every changed image before committing it.
