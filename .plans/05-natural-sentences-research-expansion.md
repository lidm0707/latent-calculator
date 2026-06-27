# Plan: Expand Natural-Sentences Research

## Goal
Grow `natural-sentences-research.md` with validation evidence, machine-output guidance, richer examples, and a copy-paste checklist.

## Tasks
- [x] 0. Add a "Validation (POC)" section summarizing the `tests/nested_sentences.rs` results: ~67% token savings, holding flat across nesting depth 1–10 (depth 1 = 64.7%, depth 10 = 68.8%).
- [x] 1. Add a "Machine-generated output" section: apply natural-lean to tool/agent output (e.g. the calculator's own answer sentences, log lines), with before/after.
- [x] 2. Expand the before/after table with 6+ more real examples and an edge-case note (when NOT to compress: errors, destructive ops, ordered steps).
- [x] 3. Add a "Pre-send checklist" derived from the 10 rules (copy-paste friendly).

## Notes
- Keep the doc itself natural-lean — it should exemplify its own rules.
- Cross-link to `tests/nested_sentences.rs` so the evidence is reproducible.
