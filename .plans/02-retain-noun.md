# Plan 02 — retain the subject noun in the answer

Goal: a result should answer "2 things" — the number AND the noun. `I have 2
dogs and one dog is dead.` returned `difference is 1`; it should say
`difference is 1 dog`. Previously every non-keyword word died in `classify_word`'s
`_ => {}` arm, so the subject was unreachable.

Scope: transformer only (`src/transformer.rs`) + test/doc updates.

## Tasks
- [x] `Answer`: add `unit: Option<String>`; `to_sentence` appends it after the
      number/currency core.
- [x] `State`: add `units: Vec<String>` — count nouns collected during `embed`.
- [x] `embed` Number arm: when a `Number` is followed by a bare count noun
      (`is_count_noun` = alphabetic, not a keyword, not a count unit), push it.
- [x] `pick_noun`: choose a candidate already in the right grammatical number for
      the result; singularize (drop trailing `s`) when the value is exactly 1.
- [x] `parse`: thread `pick_noun(&state.units, value)` into `Answer.unit`.
- [x] tests: new `noun_retained_in_answer`; updated `natural_language_subtraction`
      (unit) + `natural_language_subtraction_dogs` (integration) expectations.
- [x] README: dogs table row, REPL example, test count.
- [x] `cargo check && cargo clippy && cargo test` clean. (26 unit + 9 integration = 35 tests; 0 warnings)

## Constraints honored
- Transformer-only change to logic; no new deps.
- Enums/structs reused; noun chosen from a `Vec` so word order (`2 dogs` vs
  `one dog`) does not decide singular vs plural — `value` does.
- No regression: count units (`3 time`) and currency prices are captured before
  the noun arm, so total-cost / arithmetic outputs are unchanged.
