# Plan: Climb Toward "Real Calculator" (Functions + Honest Rejection)

## Goal
Make LatCal act like a real calculator: (B) support common functions and constants, and (A) stop guessing on input it cannot handle (reject don't-guess). Together these close the two GOAT-blocking gaps from the review.

## Tasks
- [x] 0. Add constants (pi, e).
- [x] 1. Add functions (sin/cos/tan/sqrt/log/ln/exp).
- [x] 2. Refine expr gate (lone const/func evaluates; bare number stays NL).
- [x] 3. Noise-gate tightening (reject don't-guess).
- [x] 4. Validate + README update.

## Notes
- Order matters: functions first (B), then the gate (A). Once `sin`/`cos`/… are supported, the unsupported-reject list is just calculus/symbolic terms.
- Lexer must stay strict: accept ONLY the known identifier set; every other letter sequence → `lex` returns `None` → NL path. This is what protects `5 plus 3`, `double 15`, `2 dogs` from being stolen.
- `bin_ops == 0` alone is no longer "bare number" — a lone `pi` or `sqrt(2)` has 0 binary ops but is a real result. Gate on `(bin_ops >= 1 || has_func || has_const)`.
- Risk: task 3 changes plausibility behavior. Validate against the FULL suite; keep the rule conservative (targeted keywords + stop lone-number-from-soup), not a broad noise ratio that could break grammar cases.
- Still out of scope (be honest in README): free variables/symbolic math, limits/derivatives/integrals, equation solving.
