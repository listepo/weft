# Roadmap

Approved work that is not in `plan.md` yet. It covers what the research plan (`research.md`) set out and the prototype has not delivered. Stages 0–7 of that plan are done (`done.md`); stage 8, the evaluation with real models, is T8 in `plan.md`. Each item keeps its id when it moves to `plan.md`.

### T69.1f. Windows CI job for the editor

Approved as T69.1 decision 4. T73's workflow (`.github/workflows/ci.yml`) runs on macOS and Linux only; a Windows job beside them runs `cargo nextest run -p weft-studio -p weft-studio-ffi` and the Windows leg of `studio:test` (`dotnet build` and `dotnet test` of `studio/windows`), so the WinUI shell of T69.1e is built and tested by something other than a hand run.
