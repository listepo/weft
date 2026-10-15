# WeftStudio

The desktop editor for Weft screens (T69). Milestone 1 is a native window per platform that shows
one `.weft` screen, rendered by `weft-slint` on a Slint surface from
[slint-bindings](https://github.com/pyrlyn/slint-bindings), and opens and saves it through the
native file dialogs. The document, the generation and the surface live in one Rust library,
`crates/weft-studio-ffi` (the `ws_*` editor calls and slint-bindings' `sb_*` surface); the shells
add only chrome and dialogs.

| Folder | What it is |
| --- | --- |
| `windows` | `WeftStudio.WinUI` (WinUI 3, .NET 10, unpackaged, x64 and ARM64) and `WeftStudio.Tests` (MSTest and FlaUI) |
| `scripts` | `slint-bindings.ts`: the slint-bindings checkout Cargo made of the pinned `rev`, for the shells' builds |
| `test` | Vitest drivers of `moon run studio:test`; each leg skips with a message on another OS or without its toolchain |

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
