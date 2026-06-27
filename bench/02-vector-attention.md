# Bench 02 — vector attention (hand-set latent space)

Hypothesis: replacing `attend`'s priority `if`-chain with a hand-set feature
vector `φ ∈ R^18` + linear attention `argmax(w·φ)` keeps results identical
(proven by tests) and stays in the same nanosecond class as the old short-circuit
chain — i.e. the "real latent space" is free at LatCal's scale.

## Setup (zero-dependency, std-only)
- Corpus: 9 representative queries (NL subtraction, tax, arithmetic, average,
  total-cost, unary, LaTeX root, single number).
- Full pipeline measured: `latex::expand → tokenize → embed → gate → attend → decode`.
- `cargo test vector_attention_bench --release -- --ignored --nocapture`
- Measured on the dev machine (Apple Silicon), release profile.

## Result
```
bench: 900000 parses in 326.15 ms = 362 ns/parse
```
≈ **2.76 million parses/sec**. The vector attention (10 classes × 18-feature dot
product = 180 multiply-adds) is negligible next to tokenization + the `Vec`
allocations in `embed`/`decode`.

## Correctness
- All 27 unit + 9 integration tests pass (`cargo test`), 0 regressions.
- New unit test `vector_attention_picks_the_right_class` asserts the linear
  scores reproduce the legacy priority, including the Arith-over-Sum tie-break
  on `I have 2 dogs and one is die` (5.04 vs 5.03).

## Conclusion
Adopt the vector attention. It makes the "latent space" / "attention" claims
literally true (real vectors + dot products) with no measurable cost and no new
dependencies or training. The old priority order survives as a tiny per-class
prior (the `Bias` feature) so behavior is bit-for-bit identical today.
