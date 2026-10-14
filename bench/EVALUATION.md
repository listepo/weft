# Evaluation

Translations: [Russian](../docs/ru/evaluation.md), [Ukrainian](../docs/uk/evaluation.md).

Report for T8, written 2026-10-09 from the runs already in [`test.md`](../test.md). It does not close T8. The final measurement is a three-sample edit run and a three-sample read run on the current harness (Rust core, T25 checkers, replies saved). On Anthropic those runs have not been executed: they are paid and wait on the creator's budget (estimate below). On LM Studio (`prism-ml/bonsai-27b`) neither has finished. The edit run takes about 3.4 minutes per sample here, roughly a day for 432 samples, so it is to be run in parts: Ctrl-C saves what finished and `--resume` continues it. A read run on 2026-10-13 reached 190 of 288 samples in about three hours and was lost when the disk filled up: the run stopped and the partial results could not be written, so there is no file and no history row.

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

## Pending: the Anthropic runs

The three-sample edit and read runs on Sonnet 5.5, Opus 5.5 and Haiku 4.5 are paid and wait on the creator's budget. Nothing has been sent.

Commands, from the repository root with `ANTHROPIC_API_KEY` set:

```bash
node bench/src/run.ts edit --estimate   # no calls: requests and proxy input tokens
node bench/src/run.ts edit --batch
node bench/src/run.ts read --batch
# if a run stops (quota, outage), continue it without paying for what it saved:
node bench/src/run.ts edit --batch --resume bench/results/edit-<stamp>.json
```

The local runs cost nothing but time (about three hours for read, a day for edit):

```bash
node bench/src/run.ts read --provider openai --model prism-ml/bonsai-27b
node bench/src/run.ts edit --provider openai --model prism-ml/bonsai-27b
# after Ctrl-C:
node bench/src/run.ts edit --provider openai --model prism-ml/bonsai-27b --resume bench/results/edit-<stamp>.json
```

Estimate, checked 2026-10-13:

- **Requests.** Edit: 36 tasks × 4 formats × 3 samples = 432 first-turn requests per model, 1296 in all. Read: 24 × 4 × 3 = 288 per model, 864 in all. Repair turns come on top; the October 3 run sent none.
- **Input.** `run.ts --estimate` counts 321,393 proxy tokens per model for the edit run and 210,510 for the read run (gpt-tokenizer o200k_base). Claude's tokenizer is not the proxy. Anthropic's models overview puts 1M tokens at about 555k words on the current tokenizer (introduced with Opus 4.7) against about 750k words before it ([models overview](https://platform.claude.com/docs/en/about-claude/models/overview), checked 2026-10-13), up to 1.35× as many tokens, so the estimate multiplies input by 1.35 for every model, Haiku 4.5 included: 433,881 edit and 284,189 read tokens per model.
- **Output.** Edit: the October 3 run's mean output tokens per model and format (Anthropic-reported, thinking included) × 108 requests: Sonnet 232,956, Opus 223,128, Haiku 186,192. Read: no full read run has finished on Anthropic, so 300 tokens per answer is assumed (86,400 per model).
- **Prices** (USD per million tokens, input / output; [pricing](https://platform.claude.com/docs/en/about-claude/pricing), checked 2026-10-13): Opus 5.5 $4 / $20, Sonnet 5.5 $2 / $10, Haiku 4.5 $1 / $5. The Batch API halves both ($2 / $10, $1 / $5, $0.50 / $2.50).

| Model | Edit | Read | Total | With `--batch` |
| --- | ---: | ---: | ---: | ---: |
| claude-sonnet-5-5 | $3.20 | $1.43 | $4.63 | $2.32 |
| claude-opus-5-5 | $6.20 | $2.86 | $9.06 | $4.53 |
| claude-haiku-4-5-20251001 | $1.36 | $0.72 | $2.08 | $1.04 |
| **all three** | | | **$15.77** | **$7.89** |

A budget of $20 (or $10 with `--batch`) leaves room for repair turns and longer thinking than on October 3.

## Constrained generation (T12.3)

The question: does canonical JSON held to the catalog's document schema beat free markup on validity and edit success, and what does it cost in output tokens? Three formats answer it: `weft` (markup), `weft-json` (free canonical JSON) and `weft-json-constrained` (the same, decoded under the schema). The method is in [`test.md`](../test.md).

Status, 2026-10-12: the harness is covered by a mocked-provider test, and one local trial has been run (below). Anthropic cannot take the schema.

- **Anthropic: unsupported for the constrained format.** Its structured outputs reject recursive schemas, numeric bounds, more than 16 union-typed and more than 24 optional parameters per request. The core document schema has 36 self-reaching `$defs`, 6 bounds, 132 `anyOf` and 416 optional properties (`research.md` §20), so `run.ts` refuses the format on Anthropic before any call. Markup against free JSON still runs there.
- **LM Studio:** takes the schema as `response_format` and holds the reply to it (below).

### Trial: Bonsai 27B, login screen

LM Studio, `prism-ml/bonsai-27b`, the three login edit tasks, three samples each, commit `7caefe55`; raw results in `bench/results/edit-2026-10-12T18-31-58-922Z.*`, history row in `test.md`.

| Format | Valid | Valid after repair | Success | Success after repair | Mean output tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| `weft` | 88.9% | 88.9% | 55.6% | 55.6% | 240 |
| `weft-json` | 100.0% | 100.0% | 100.0% | 100.0% | 648 |
| `weft-json-constrained` | 66.7% | 66.7% | 33.3% | 33.3% | 380 |

- **The schema was accepted and obeyed.** Every constrained reply was JSON that passed the schema (checked afterwards with Ajv 8.20, draft 2020-12); nothing was rejected or partly ignored. So did every free JSON reply. The invalid ones (all three `login.e2` samples) break rules no JSON Schema states: an id used twice (W301) and a submit button outside its form (W313). Repair did not fix them.
- **The grammar admits nonsense.** All three `login.e1` constrained samples dropped the checkbox's `$.remember` binding and filled it with schema-legal but meaningless props (`perspective`, `rotate-x`); valid, but the edit failed.
- **Free JSON won this trial.** Markup lost on one invalid event name in `login.e1` and on a `login.e2` answer that did not bind `disabled` to `$.busy` in any sample.
- **Confounds.** Under the schema the model writes no thinking (the grammar holds it from the first token), while the free formats think first; their reasoning tokens are excluded from the means. The free JSON was pretty-printed and the constrained JSON compact, which accounts for most of the token gap between the two.
- **Reading.** Nine samples per format on one screen and one local model. Constrained decoding guarantees shape, not validity, and here it cost success. It is not evidence for constrained output as the default path; a broader run, on a provider that takes the schema, would be needed to say more.
