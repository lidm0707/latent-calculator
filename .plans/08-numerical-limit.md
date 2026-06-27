# Plan: Numerical limit by closed-form substitution

## Goal
Stop blanket-rejecting `lim`. Let LatCal evaluate a limit `lim VAR -> POINT of EXPR`
the only honest way a modelless numeric calculator can: substitute `POINT` for `VAR`
in `EXPR` and run the arithmetic evaluator. Singular limits (e.g. `sin(x)/x` at 0 →
`0/0`) stay honestly rejected as `NotMath`. Symbolic differentiation, integration,
and equation solving remain out of scope.

## Why substitution (not symbolic limits)
LatCal has no CAS. The only thing it *can* compute for `lim VAR -> POINT of EXPR` is
`EXPR` evaluated at `POINT`. That is correct for every continuous expression and is
exactly what the user's forward-difference example evaluates to:
`(sin(0.001) - sin(0)) / 0.001 ≈ 0.9999998` (≈ cos(0)). True symbolic limits,
removable singularities, and L'Hôpital are explicitly out of scope and rejected.

## Tasks
- [x] 0. Expose `expr::is_known_ident` so the limit parser can reject a variable
      that collides with a known function/constant (`lim e->0 of e`, `lim sin->0`).
- [x] 1. `parse_limit_form` — structural parse of `lim`/`limit`, `VAR`, `->`/`→`,
      `POINT`, `of`, `EXPR`. Returns borrowed slices; `None` on any mismatch.
- [x] 2. `substitute_var` — word-boundary replacement of `VAR` with `(POINT)` so
      operator precedence is preserved and `x` inside `exp`/`0.001` is untouched.
- [x] 3. `try_limit` — substitute → `expr::evaluate`; `Some(Ok(v))` on success,
      `Some(Err(NotMath))` on a matched-but-unevaluable limit (0/0, ln(0), ...),
      `None` for non-limit input (fall through to the normal pipeline).
- [x] 4. Wire into `Calculator::parse` right after `latex::expand`, before the expr
      fast path. Label the answer `limit`.
- [x] 5. Tests: substitution cases (`lim x->0 of sin(x)` → 0, `lim x->2 of x^2` → 4,
      `lim x->0 of 1` → 1, the forward-difference ≈ 1), arrow/keyword variants
      (`limit`, `→`, signed point), and rejections (`sin(x)/x`, `1/x`, var collides
      with `e`, malformed `lim x->0`).
- [x] 6. Update the existing `reject_dont_guess_on_unsolvable_expressions` test:
      `lim x->0 of 1` is no longer a rejection (it is now `limit is 1`); replaced
      with genuinely singular `lim x->0 of 1/x` and `lim x->0 of sin(x)/x`.
- [x] 7. README + pipeline diagram update; kept `derivative`/`integral` in the
      reject list (still out of scope).

## Notes
- `CALCULUS_WORDS` is deliberately left untouched: it still contains `lim`/`limit`.
  It is only reached when `try_limit` returns `None` (malformed limit syntax like
  `lim x->0` with no `of`), where `looks_structured` honestly rejects. This keeps
  the existing rejection path intact for non-limit calculus words
  (`derivative`, `integral`, `dx`, `dy`).
- The variable collision guard is the only new public surface on `expr`
  (`is_known_ident`). The lexer's own identifier match is unchanged, so the
  non-stealing guarantee for NL input (`5 plus 3`, `2 dogs`) is unaffected.
- `substitute_var` is O(n) and allocates one `String`; borrowing throughout
  `parse_limit_form` keeps the pre-evaluation work allocation-free.
