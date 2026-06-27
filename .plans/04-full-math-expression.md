# Plan: Full Math Expression Evaluation

## Goal
Evaluate pure arithmetic expressions with operator precedence, parentheses, and unary minus, coexisting with the existing NL classifier (so `3 + 4 × 2 = 11`, `(1+2)*3 = 9`, `-3 + 2 = -1`).

## Tasks
- [x] 0. Add `src/expr.rs`: recursive-descent evaluator `evaluate(&str) -> Option<f64>` with its own lexer (numbers, `+ - * / × ÷`, parens). Returns `None` on any non-arithmetic token or leftover/incomplete input. Unit tests for precedence, parens, unary minus, left-assoc, and rejection cases.
- [x] 1. Wire into `Calculator::parse`: attempt `expr::evaluate` first; if `Some` AND the expression contains ≥1 operator, return `Answer { label: "result" }`. Else fall through to existing NL flow. Keep existing tests green.
- [x] 2. Specialize label for single-op expressions: exactly one binary op on two numbers → `sum`/`difference`/`product`/`quotient`; otherwise `result`. Tests.
- [x] 3. Integration tests in `tests/`: precedence, parens, unary minus, left-assoc division, and non-stealing (`sin(x)`, letters, `3 +` trailing) fall through to NL/Unknown.
- [x] 4. `cargo check && cargo clippy --all-targets -- -D warnings` + full test suite; note behavior change in README if any.

## Notes
- The expr path self-gates: the lexer rejects any letter/non-arithmetic char, so NL inputs (`20% of 50`, `3 times 4`, `$20`, `5 plus 3`) fall through unchanged.
- Bare number with no operator → NOT claimed by expr (keep NL `Single` behavior).
- Functions (`sin`/`cos`/`sqrt`/`log`, constants `pi`/`e`) are explicitly out of scope here — future plan.
- `Answer.label` is `&'static str`; use existing `"result"` constant-style label.
