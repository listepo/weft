# Evaluation

Translations: [Russian](../docs/ru/evaluation.md), [Ukrainian](../docs/uk/evaluation.md).

Report for T8, written 2026-10-09 from the runs already in [`test.md`](../test.md). It does not close T8. The final measurement is a three-sample edit run and a three-sample read run on the current harness (Rust core, T25 checkers, replies saved). Those runs have not been executed: no `ANTHROPIC_API_KEY` is set here, and no LM Studio server is listening.

## Criteria

Weft must reach at least 95% validity on the first try and at least 99% after one repair cycle. Success, output size and the gap to HTML, JSX and A2UI inform the continue/stop recommendation. Raw results must be in the repository. They are, under `bench/results/`.

## What was measured

| Run | Samples | Replies saved | Weft valid / after repair / success | Notes |
| --- | ---: | --- | --- | --- |
| 2026-10-03 edit, all 36 tasks, Sonnet 5.5, Opus 5.5, Haiku 4.5 | 1 | no | 100% / 100% / 100% | Before T25. Read run stopped after 39 of 288 requests (credit exhausted); nothing saved. |
| 2026-10-04 Bonsai 27B, login only | 1 | yes | edit 100% / 100% / 66.7%; read 100% answered | Smoke. The edit miss is `login.e2`. |

The October 3 edit file is `bench/results/edit-2026-10-03T20-19-03-199Z.json` (432 results: 3 models × 36 tasks × 4 formats). Every Weft result is valid and successful. No result in that file was repaired: every miss was a valid document that failed an assertion, so the repair prompt never fired.

Mean output tokens on that run, from the summary:

| Model | Weft | HTML | JSX | A2UI |
| --- | ---: | ---: | ---: | ---: |
| claude-sonnet-5-5 | 366 | 361 | 426 | 1004 |
| claude-opus-5-5 | 346 | 334 | 409 | 977 |
| claude-haiku-4-5-20251001 | 264 | 276 | 321 | 863 |

## Failures

Eight results failed, all of them baselines, all of them valid:

- `login.e1` JSX, Sonnet and Opus: the Remember me checkbox was not between the password field and Sign in.
- `search-results.e2` JSX (all three models) and HTML (Haiku): the empty-state copy was lost or not placed after "No results found".
- `error-state.e1` JSX and A2UI, Haiku: the Reload page button was not placed after Retry.

Weft had no failure in that file. The Bonsai smoke run did: `login.e2` changed `disabled="{!$.email}"` to `disabled="{!$.busy}"` and kept the `!`, so the button is disabled while `$.busy` is falsy. HTML and JSX on the same run got `{$.busy}` / `data.busy`. The markup validates. The repair cycle cannot see it. T28's readback turn is the follow-up for that class of miss; it is in the harness (`--readback`) and is not in any saved run yet.

## Recommendation

Continue. On the only full edit run, Weft is at 100% first-try validity and 100% after repair, above the 95% and 99% bars, with success at 100% against baselines at 94.4–100%, and with output closer to HTML than to A2UI. That is not a reason to stop.

It is also not a reason to close T8. The run has one sample, saved no replies, predates the T25 answer rule and the Rust core the final runs are supposed to use, and the matching read run never finished. Repeat the edit and read runs at three samples on the current harness, keep the replies, add the history rows, and revise this report. Until then the numeric bar is met only by the earlier measurement.
