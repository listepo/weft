# WeftStudio

The desktop editor for Weft screens (T69). Milestone 1 is a native window per platform that shows
one `.weft` screen, rendered by `weft-slint` on a Slint surface from
[slint-bindings](https://github.com/pyrlyn/slint-bindings), and opens and saves it through the
native file dialogs. The document, the generation and the surface live in one Rust library,
`crates/weft-studio-ffi` (the `ws_*` editor calls and slint-bindings' `sb_*` surface); the shells
add only chrome and dialogs.

| Folder | What it is |
| --- | --- |
| `macos` | WeftStudio for macOS (SwiftUI and AppKit, macOS 14, arm64): the app, its Swift Testing unit tests and an XCUITest, generated into an Xcode project by XcodeGen |
| `windows` | `WeftStudio.WinUI` (WinUI 3, .NET 10, unpackaged, x64 and ARM64) and `WeftStudio.Tests` (MSTest and FlaUI) |
| `scripts` | `slint-bindings.ts`: the slint-bindings checkout Cargo made of the pinned `rev`, for the shells' builds |
| `test` | Vitest drivers of `moon run studio:test`; each leg skips with a message on another OS or without its toolchain |

## macOS

Needs Apple Silicon with Xcode, XcodeGen and the Rust toolchain of `mise.toml`. From the repository
root:

```bash
node studio/macos/scripts/xcodegen.ts   # writes studio/macos/WeftStudio.xcodeproj (gitignored)
cd studio/macos && xcodebuild -scheme WeftStudio -destination 'platform=macOS,arch=arm64' test
```

The generator passes the slint-bindings checkout to XcodeGen as `SLINT_BINDINGS_DIR`, which names
its Swift package. A pre-build phase (`scripts/build-ffi.sh`) runs `cargo rustc -p weft-studio-ffi
--crate-type staticlib --target aarch64-apple-darwin` (`--release` for Release) and links the
library; the app imports its header through `CWeftStudio/module.modulemap`. The app is signed ad
hoc and unsandboxed in M1 (T69.1 decision 5), and it imports the `dev.weft.screen` file type that
the Xcode Source Editor Extension exports.

`WeftStudio -WeftStudioScreen <path>` opens the screen in the first window: a `-key value` pair
lands in the argument domain of `UserDefaults`, while a bare path would be an open-document
request that SwiftUI ignores. `WEFT_STUDIO_OPEN_PATH` and `WEFT_STUDIO_SAVE_PATH` in the
environment answer the open and save panels, which is how the UI test saves without a panel.

`studio/test/macos.test.ts` runs XcodeGen, `xcodebuild build` and `xcodebuild test`: the document
model against the real library with a fake picker, and the UI test that opens `corpus/login`,
finds the window titled `screen.weft`, checks that the surface is not one colour, and saves it
byte-identical with Save As.

### By hand

- File › Open… (⌘O) shows the open panel filtered to `.weft`; every corpus screen opens and renders.
- File › Save As… (⇧⌘S) shows the save panel with the file's name; an unedited screen saves byte-identical, and the window takes the new name.
- File › Save (⌘S) writes the screen back to its file.
- ⌘O and ⌘S work while the Slint surface has the focus.
- A file with a syntax error shows an alert with the diagnostics and keeps the open screen.
- On a Retina display the screen is sharp; resizing the window live redraws it without tearing, and moving it to a display with another scale redraws it sharply.

## Windows

Needs Windows with the .NET 10 SDK and the Rust toolchain of `mise.toml`. From the repository root:

```powershell
node studio/scripts/slint-bindings.ts --msbuild   # writes studio/windows/SlintBindings.g.props
dotnet build studio/windows/WeftStudio.WinUI -p:Platform=x64 -p:RuntimeIdentifier=win-x64
cd studio/windows; dotnet test --project WeftStudio.Tests
```

The build runs `cargo rustc -p weft-studio-ffi --crate-type cdylib` for the platform's target
(`--release` for Release) and copies `weft_studio_ffi.dll` next to the app. slint-bindings'
`SlintPanel` imports `slint_bindings_ffi`; the app sends those imports to `weft_studio_ffi.dll`,
which carries the same `sb_*` calls, so the editor and the surface share one Slint.

`WeftStudio.exe [screen.weft] [--pick <path>]` opens the screen at launch; `--pick` answers every
file dialog with `<path>`, which is how the smoke test saves without a dialog.

`studio/test/windows.test.ts` runs both steps; the `studio-windows` job of `.github/workflows/ci.yml`
runs it on `windows-latest` with the editor crates' tests and keeps the window capture as an
artifact.

### By hand

The smoke test answers the dialogs itself, so these need a person on a Windows machine:

- File > Open… (Ctrl+O) shows the system picker filtered to `.weft`; every corpus screen opens and renders.
- File > Save As… (Ctrl+Shift+S) shows the save picker with the file's name; an unedited screen saves byte-identical.
- Ctrl+O and Ctrl+S work while the Slint panel has the focus.
- A file with a syntax error shows a dialog with the diagnostics and keeps the open screen.
- Resizing the window and moving it to a display with another scale redraws the screen sharply.
