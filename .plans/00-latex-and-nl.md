# Plan 00 — LaTeX subset + richer natural language

Goal: make LatCal handle a real arithmetic subset of LaTeX and more natural
human language, e.g. `I have 2 dogs and one is die` -> `difference is 1`.

## Tasks
- [x] add `src/latex.rs` preprocessor (`expand`): `\frac{}{}`, `\sqrt[k]{}`,
      `\times`, `\cdot`, `\div`, `\pi`, `\$, \%, \{, \}`, `\, \; \: \!`,
      `\left( \right)`. Emits forms the tokenizer already understands +
      `root(k,n)` for roots.
- [x] tokenizer: `Token::Root { index, radicand }` + `root_token` parser.
- [x] transformer: `Latent::Root`, `State.roots`, embed/attend/decode arms.
- [x] NL vocabulary: extend `NL_SUB` (die/died/lost/gone/...) so the dogs
      example computes; update the stale rejection test.
- [x] `lib.rs`: `pub mod latex;`.
- [x] tests: unit + integration for LaTeX and the dogs example.
- [x] README: Dioxus 0.7+, LaTeX section, expanded vocab, test count.
- [x] `cargo check && cargo clippy && cargo test` clean. (22 unit + 9 integration = 31 tests; 0 warnings)

## Constraints honored
- Zero dependencies kept (no criterion bench; LaTeX rewrite is O(n) string work).
- Enums over hard-codes (`Token::Root`, `Latent::Root`).
- Const vocab arrays.
