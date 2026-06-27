# Plan: More Math Operators (Plan 07)

## Goal
Extend the expression evaluator with power (`^`), implicit multiplication, and more functions (`abs`, `floor`, `ceil`, `asin`, `acos`, `atan`) — still pure Rust, still self-gating.

## Tasks
- [x] 0. Restructure grammar + add `^` power operator.
- [x] 1. Add `abs`/`floor`/`ceil`/`asin`/`acos`/`atan`.
- [x] 2. Add implicit multiplication (`2(3)`, `2 pi`, `(2)(3)`).
- [x] 3. Integration tests + end-to-end via `Calculator::parse`.
- [x] 4. clippy + full suite; README.

## Notes
- Grammar after task 0: `expr → term (('+'|'-') term)*`; `term → unary (('*'|'/') unary)*`; `unary → ('+'|'-') unary | power`; `power → primary ('^' unary)?`; `primary → num | const | func '(' expr ')' | '(' expr ')'`.
- Implicit mult (task 2) triggers when the next token is a primary-starter (`Num`/`Const`/`Func`/`LParen`); right operand is `unary`, so `2(-3)` works but `2 -3` stays subtraction (handled at `expr`).
- `^` is right-assoc with unary exponent so `2 ^ -1` and `2 ^ 3 ^ 2` work.
- Skip `%` modulo (conflicts with percent semantics). Skip `|x|` notation (tokenizing `|` is ambiguous) — `abs()` covers it.
- Lexer stays strict: only the expanded identifier set is accepted; everything else → `None` → NL.
