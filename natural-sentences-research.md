# Natural Human Sentences Under a Token Budget

Goal: write in **normal, grammatical sentences** while keeping tokens **as low as possible**. The opposite of caveman mode — keep grammar (articles, subjects, verbs), cut only the fat.

## Core idea

Caveman saves tokens by **breaking grammar**. That's hard to read and risky (see caveman's own "Auto-Clarity Exception"). The natural-but-lean path keeps the grammar skeleton readers parse fast, and trims only modifiers + meta-noise. Result: nearly the same token count, far higher clarity.

## Rules (keep grammar, cut fat)

1. **Complete sentences.** Subject + verb. No fragments. `The parser fails on empty input.` not `Parser fail empty input.`
2. **One idea per sentence.** Split long runs. Short sentences are cheap and clear.
3. **Active voice.** `We retry failed jobs.` (4) vs `Failed jobs are retried by the worker.` (7).
4. **Strong verb, not noun phrase.** `decide` not `make a decision`; `fix` not `apply a fix`.
5. **Cut filler.** Drop `very`, `really`, `basically`, `actually`, `just`, `simply`, `in order to` → `to`.
6. **Cut throat-clearing.** Don't narrate intent. Say the thing, not that you'll say it.
7. **Cut redundancy.** `free gift` → `gift`; `end result` → `result`; `past history` → `history`.
8. **Pronoun when antecedent clear.** Avoid repeating a long noun.
9. **Precise word over phrase.** `stale` beats `no longer current`.
10. **No meta-commentary.** Skip "It should be noted that…", "I think that…".

## Before / after

| Verbose | Natural + lean |
|---|---|
| In order to fix the bug, you should make a decision to update the token check. | To fix the bug, update the token check. |
| It should be noted that the worker is actually responsible for retrying failed jobs. | The worker retries failed jobs. |
| The reason why it fails is because the input is basically empty. | It fails on empty input. |
| Due to the fact that the cache is basically cold at startup, it is a good idea to warm it. | The cache is cold at startup, so we warm it. |
| For the purpose of debugging, what needs to be done is to enable verbose logging. | We enable verbose logging for debugging. |
| It is important to note that there are a large number of timeouts occurring on a regular basis. | Timeouts occur often. |
| As a result of the deploy being broken, a rollback of the change is required to be performed. | The deploy broke, so we roll back the change. |
| The reason for the slow queries is that the index is no longer current and out of date. | Slow queries come from a stale index. |
| In the event that the queue is full, what happens is that jobs are being dropped. | A full queue drops jobs. |

### When NOT to compress

Keep full grammar and full context where a fragment risks a misread or a harmful action.

- **Error messages.** Name the failure, the input, and the fix. `Cannot divide by zero in expression "1/0".` beats `bad input`.
- **Destructive or irreversible actions.** State the object and the effect. `This deletes the file config.toml and cannot be undone.` Keep every word.
- **Ordered multi-step procedures.** Each step stands alone with a subject, verb, and result. A dropped article can flip a sequence.

## Validation (POC)

The rules are measured, not assumed. `tests/nested_sentences.rs` generates 100 sentences — 10 topics × nesting depth 1–10 — in both verbose and lean form, and asserts the lean form wins at every depth.

Run it:

```
cargo test --test nested_sentences -- --nocapture
```

Measured averages:

| Depth | Saving |
|---|---|
| 1 | 64.7% |
| 5 | 71.4% |
| 10 | 68.8% |
| **Overall** | **67.2%** (7770 verbose tokens → 2550 lean tokens) |

The win holds flat as sentences grow. Depth 1 saves 64.7%; depth 10 saves 68.8%. Deeper nesting does not erode the leverage — the rules scale with complexity.

## Machine-generated output

LatCal answers in natural language. Its messages should read like a careful human wrote them: subject + verb, no filler, grammar kept. The same rules apply to log lines, status updates, and error text as to prose.

| Verbose machine output | Natural-lean output |
|---|---|
| It should be noted that the result of the calculation is basically equal to 11. | The result is 11. |
| The total amount, after summing all of the values, comes out to 60 dollars. | The total is 60$. |
| As a result of the multiplication, the product is actually 0.000001. | The product is 0.000001. |
| Due to the fact that the input was empty, the operation was unable to be completed. | The input is empty, so the operation fails. |

A machine that talks like a human is easier to scan and to trust. Drop the throat-clearing; keep the grammar.

## Pre-send checklist

Before sending a sentence or a machine reply, run it through this list.

- Every sentence has a subject and a verb.
- Active voice, not passive.
- One idea per sentence; split long runs.
- Strong verb, not a noun phrase (`decide`, not `make a decision`).
- No filler (`very`, `basically`, `actually`, `just`).
- `to`, not `in order to`.
- No throat-clearing; the sentence states the thing.
- No redundancy (`free gift` → `gift`).
- Precise word over phrase (`stale`, not `no longer current`).
- No meta-commentary ("It should be noted that…").
- Full grammar kept for errors, destructive actions, and ordered steps.

## Token cost reality

Most savings come from rules 5–7 (filler, throat-clearing, redundancy), **not** from dropping grammar. Keeping `the`/`a`/`is` costs almost nothing; keeping `basically`/`in order to`/`make a decision` costs a lot. So: protect grammar, attack the modifiers.

## TL;DR

Write like a careful human who hates waste. Full sentences, active voice, strong verbs, no filler. ~80% of caveman's savings with ~0% of its readability risk.
