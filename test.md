# Quality test

Translations: [Russian](docs/ru/test.md), [Ukrainian](docs/uk/test.md).

The quality test measures how reliably language models read and write Weft, compared with three formats a model could be asked to use instead: semantic HTML, a React JSX component and A2UI JSON. It is the evidence behind the continue/stop decision of T8 and the regression check for every later change to the format, the primer or the core. The harness lives in `bench/`.

## What is measured

**Corpus.** Twelve reference screens in `corpus/` (login, signup, settings, data table, tabs, confirm dialog, wizard step, search results, todo list, profile, menu, error state). Each screen is written in all four formats with the same content, structure, states, bindings and actions, and comes with its data model. `corpus/README.md` explains how the formats were written.

**Tasks.** `corpus/tasks.json` holds 60 tasks, five per screen:

- 36 **edit** tasks: a change in plain words ("Add a 'Remember me' checkbox bound to `$.remember` below the password field"). The model returns the whole updated document.
- 24 **question** tasks: a question about the screen ("Which action does the 'Forgot password?' link trigger?"). The model answers with one `ANSWER:` line.

**Prompt.** One user message: a short primer for the format, the data model, the screen in that format, and the task. No system prompt, no examples beyond the primer. Primer sizes are in `bench/REPORT.md`.

**Checks.**

- *Valid*: the edit reply parses in its format with no errors (Weft: the core validator in strict mode; HTML, JSX, A2UI: their parsers in `bench/src/adapters/`).
- *Success*: the reply is valid, the requested change is there (assertions in the task, checked on a neutral tree shared by all formats, including order and bindings), and nothing else was lost.
- *Repair*: when an edit reply is invalid, the model gets one more prompt with its reply and the validator's diagnostics, and nothing else. The same rule applies to every format. *Valid after repair* and *success after repair* count the second reply.
- *Readback* (optional, `node bench/src/run.ts edit --readback`): after an edit reply is valid, on the first try or after the repair, the model gets one more prompt. That prompt lists the bindings, flags, events and loops that changed, in plain words, and nothing else. A negated binding reads `true while $.busy is falsy (NOT $.busy)`; `!` is NOT. The sentences come from the neutral tree, so every format gets the same wording. *Valid after readback* and *success after readback* count that reply. Runs without the flag do not send it, and their table has no readback columns, so the rows below stay comparable.
- *Answer*: the last `ANSWER:` line, compared case- and whitespace-insensitively. An action named in the format's own spelling (`press:nav.reset` in HTML, `actions.nav.reset()` in JSX) counts as the action.
- *Output tokens*: the visible size of the first reply as reported by the provider; reasoning tokens are excluded.

**Samples.** Every task runs three times per format and model. Rates are the mean over all samples; the range is the lowest and highest single-sample rate.

**Pass criteria (T8).** Weft must reach at least 95% validity on the first try and at least 99% after one repair cycle. Success, output size and the gap to the baselines inform the recommendation.

**Token cost of the formats.** `run.ts tokens` measures the corpus itself in bytes and tokens (an offline proxy tokenizer, plus Anthropic's counter when a key is set). Results are in `bench/REPORT.md`.

## How to run

```bash
# Local model through LM Studio's OpenAI-compatible server (lms server start; lms load <model>)
node bench/src/run.ts edit --provider openai --model prism-ml/bonsai-27b
node bench/src/run.ts read --provider openai --model prism-ml/bonsai-27b

# Anthropic models (ANTHROPIC_API_KEY); --batch halves the price, results arrive within hours
node bench/src/run.ts edit [--batch]
node bench/src/run.ts read [--batch]

# Size of the corpus in every format
node bench/src/run.ts tokens

# Re-check saved replies with the current checkers, no model calls
node bench/src/run.ts rescore bench/results/<run>.json
```

Options: `--screens a,b`, `--formats weft,html,jsx,a2ui`, `--samples N` (default 3), `--concurrency N` (default 4), `--base-url` for another OpenAI-compatible server, `--readback` for the binding readback turn on an edit run. Each run writes `bench/results/<mode>-<time>.json` (every reply verbatim with its verdict) and a summary `.md`. A run that fails half-way still saves what it finished.

## Limitations

- The corpus is small (12 screens, 60 tasks) and written by the format's authors.
- Edit success is judged by the task's assertions; a change the assertions do not cover is not checked.
- One prompt shape; no system prompt, no tool calling, no few-shot examples. The readback turn is a second user message, and only when `--readback` is set.
- Runs before the history row of 2026-10-04 have one sample, no saved replies and no action-spelling rule.

## History

Every kept run is recorded here, newest last; see `AGENTS.md`. Weft columns: first-try valid / valid after repair / success after repair. Baselines: success after repair for HTML / JSX / A2UI.

| Date | Commit | Provider / model | Run | Samples | Weft | Baselines | Results | Notes |
| --- | --- | --- | --- | ---: | --- | --- | --- | --- |
| 2026-10-03 | 645afb7 | Anthropic / claude-sonnet-5-5 | edit, login only | 1 | 100 / 100 / 100% | 100 / 66.7 / 100% | `edit-2026-10-03T19-51-24-086Z` | Trial run |
| 2026-10-03 | 55b3159 | Anthropic / claude-sonnet-5-5 | edit, all | 1 | 100 / 100 / 100% | 100 / 94.4 / 100% | `edit-2026-10-03T20-19-03-199Z` | No replies saved |
| 2026-10-03 | 55b3159 | Anthropic / claude-opus-5-5 | edit, all | 1 | 100 / 100 / 100% | 100 / 94.4 / 100% | same file | No replies saved |
| 2026-10-03 | 55b3159 | Anthropic / claude-haiku-4-5-20251001 | edit, all | 1 | 100 / 100 / 100% | 97.2 / 94.4 / 97.2% | same file | No replies saved |
| 2026-10-03 | 55b3159 | Anthropic / claude-sonnet-5-5 | read | — | — | — | none | Stopped after 39 of 288 requests: credit balance exhausted; nothing saved. Most baseline misses were action spellings, fixed in T25 |
| 2026-10-04 | 6c36b24 | LM Studio / prism-ml/bonsai-27b | read, login only | 1 | 100% answered | 100 / 100 / 100% | `read-2026-10-04T19-35-27-209Z` | Smoke run; HTML and JSX answered in their own action spelling |
| 2026-10-04 | a9e189a | LM Studio / prism-ml/bonsai-27b | edit, login only | 1 | 100 / 100 / 66.7% | 100 / 100 / 100% | `edit-2026-10-04T19-44-58-547Z` | Smoke run; the Weft miss is `login.e2`: the model bound `disabled` to `{!$.busy}` instead of `{$.busy}` |
| 2026-10-09 | 51b77bc | LM Studio / prism-ml/bonsai-27b | edit, login only, `--readback` | 3 | 88.9 / 100 / 88.9% | 88.9 / 55.6 / 100% | `edit-2026-10-09T09-08-11-689Z` | T28 rerun. Weft success after readback 100% (88.9% after repair): `login.e2` sample 1 bound `disabled` to `{!$.busy}` again, valid, so repair did not fire; the readback turn fixed it. Baselines after readback: 88.9 / 55.6 / 100%, unchanged |
