# Plan 03 — real latent vector + linear attention (no training)

Goal: make the "latent space" and "attention" claims literally true instead of
metaphorical — a real vector `φ ∈ R^18` per input, and operation selection by
dot-product attention `argmax(w·φ)` — with **zero training** (hand-set features
and weights). Option A from the design discussion.

Scope: transformer only (`src/transformer.rs`) + bench summary + README.

## Tasks
- [x] extract the inline `attend` arms into `try_average` / `try_total_cost` /
      `try_unary` / `try_root` / `try_arith` / `try_sum` / `try_single` (each
      returns `Option<Latent>` — `None` = inapplicable = attention mask).
- [x] `Feature` enum (18 dims) + `features(&State) -> [f64; 18]` — the real
      latent vector (flags, counts, multiplicity, a constant Bias).
- [x] `OpClass` enum (10 classes) + `WEIGHTS: [[f64; 18]; 10]` const — hand-set
      affinity rows; the Bias column carries legacy priority as a tiny prior.
- [x] `score(class, φ) = w·φ`; `attend` now collects applicable candidates and
      takes the `argmax` of `score` (real linear attention with a validity mask).
- [x] tests: `vector_attention_picks_the_right_class` + an `#[ignore]` zero-dep
      timing bench `vector_attention_bench` (std::time only).
- [x] bench summary in `bench/02-vector-attention.md`: **362 ns/parse** (release),
      identical results to the old priority chain.
- [x] README: attend bullet now describes real vector + linear attention; test count 36.
- [x] `cargo check && cargo clippy --all-targets && cargo test` clean.
      (27 unit + 9 integration = 36 tests; 0 warnings)

## Constraints honored
- No training, no new deps (std-only bench via std::time — no criterion).
- Enums over hard-codes (`OpClass`, `Feature`); named `const WEIGHTS`.
- Behavior bit-for-bit identical: the hand-set weights reproduce every legacy
  result, incl. the Arith-over-Sum tie-break (5.04 vs 5.03).

## Notes
- The old priority order survives as a tiny per-class prior (Bias feature), so
  the change is behavior-preserving today and the affinities can grow to carry
  more weight as the vocabulary expands (path to genuine generalization).
