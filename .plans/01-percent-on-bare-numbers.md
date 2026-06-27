# Plan 01 — percent operations on bare numbers

Goal: tax/vat/discount must work without a currency symbol and without a `%`,
and recover numbers the strict tokenizer dropped (thousands separators, glued
keywords). Transcript bugs: `10 tax 7%` returned `10`, `90 tax 8.8%` returned
`90`, `iphone 30211 vat 10%` returned `30211`, `10 tax 7` was Unknown,
`90tax 8.8%` was Unknown, `iphone 30,211 vat 10%` was Unknown.

Scope: transformer only (`src/transformer.rs`). Tokenizer untouched.

## Tasks
- [x] `try_percent_price`: base falls back to `s.numbers[0]` when `s.prices`
      is empty; rate falls back to `s.numbers[rate_idx]` when `s.percents` is
      empty (`rate_idx = 1` for bare base, `0` when a price carries it).
- [x] `classify_word`: recover a leading number from a word via `loose_number`
      (`30,211` -> 30211, `90tax` -> 90 + `tax`), guarded by `is_known_suffix`
      so garbage like `2@` stays noise.
- [x] helpers `loose_number`, `is_known_suffix`.
- [x] tests: `tax_and_discount_on_bare_numbers`, `tax_without_percent_symbol`,
      `thousands_separators_and_glued_keyword`.
- [x] `cargo check && cargo clippy && cargo test` clean. (25 unit + 9 integration = 34 tests; 0 warnings)

## Constraints honored
- Transformer-only change; zero new deps; enums reused (`PercentDir`).
- Const vocab arrays untouched; `is_count_unit`/`is_math_keyword` reused.
- No regression: `has_sum` is not an anchor, so noise-rejection tests hold.
