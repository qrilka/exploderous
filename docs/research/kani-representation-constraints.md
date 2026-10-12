# Research: Kani representation constraints

## Summary
Kani supports heap-backed `Vec` operations and fixed arrays; lack of `kani::any::<Vec<_>>()` is a symbolic-construction limitation, not a ban on vectors. Available evidence does not establish which representation is cheapest for Exploderous; bounded harnesses can exercise actual production routines without requiring a separate game model.

Context: `rust-toolchain.toml` pins Rust **1.98.0**; `Cargo.toml` selects **edition 2024**. The project plan calls for a scoped bounded critical-rule proof and leaves arena dimensions/tool versions/runtime budget open.

## Findings — verified claims
1. **Claim:** `[T; N]` supports `kani::any()` when `T` supports `Arbitrary`; `Vec` needs explicitly bounded construction. Official examples construct containers from symbolic entries and demonstrate vector `clone`, `push`, `pop`, indexing and reversal using experimental `bounded_any::<_, 16>()`. **Sources:** [Symbolic values](https://model-checking.github.io/kani/tutorial-nondeterministic-variables.html), [Bounded arbitrary](https://model-checking.github.io/kani/reference/bounded_arbitrary.html). **Support:** direct evidence. **Confidence:** high. Experimental bounded derivation currently uses one bound for all bounded fields.

2. **Claim:** Functional support does not imply cheap proofs. The symbolic-values tutorial warns that nondeterministic-size heap structures scale poorly. **Sources:** Symbolic values, above. **Support:** direct evidence. **Confidence:** high for the documented warning, not a project-specific performance prediction. Its illustrative timings are not a controlled array-versus-vector comparison. **Researcher inference:** symbolic fixed length versus symbolic variable length must be distinguished from heap versus inline storage; arrays still expose symbolic cells and production loops.

3. **Claim:** Harnesses can bound inputs and call the actual routine. The loop tutorial calls `initialize_prefix` with a bounded array/slice and symbolic length constrained by `assume`. Insufficient unwind bounds fail unwinding assertions; default unwinding may not terminate. **Source:** [Loop unwinding](https://model-checking.github.io/kani/tutorial-loop-unwinding.html). **Support:** direct evidence. **Confidence:** high. Bounds must accommodate implementation/control-flow details, not merely arena cell count. **Researcher inference:** small legal arenas can exercise shared production movement/blast routines if their API permits those sizes; this proves only the specified states, actions and dimensions, not larger arenas or unlimited ticks. Symbolic type validity alone does not enforce gameplay invariants.

4. **Claim:** Kani uses its own compiler/toolchain. Driver source selects release Cargo from Kani's installation, or its build-time toolchain in development, and documents pointing `RUSTC` at `kani-compiler`. Current upstream manifests use edition 2024 and a separately pinned nightly. **Sources:** [Driver selection](https://github.com/model-checking/kani/blob/main/kani-driver/src/session.rs), [Manifest](https://github.com/model-checking/kani/blob/main/Cargo.toml), [Toolchain](https://github.com/model-checking/kani/blob/main/rust-toolchain.toml). **Support:** direct evidence. **Confidence:** high for inspected upstream source. **Researcher inference:** the repository's 1.98.0 pin does not force Kani to verify using that compiler; edition support does not establish compatibility with every API/feature/dependency used under 1.98.0.

## Contradictions
Search summaries disagreed with fetched upstream toolchain contents; upstream source was retained. No substantive representation contradiction found.

## Missing evidence / runnable questions
No selected Kani release, compatibility run, or representative proof-cost comparison exists in this investigation. Compare equivalent production operations with identical symbolic cells, dimensions, assumptions and properties: array versus fixed-length Vec, then variable-length Vec; measure runtime/memory and confirm unwinding assertions. Separately compile/prove the edition-2024 crate with a recorded Kani/compiler version.

## Sources / validation
Kept: six primary links above. Deprioritized: search-generated summaries and tutorial timings as benchmarks. `source_check` could not semantically validate claims (one returned no passages); original sources were inspected manually. Upstream `main` is mutable. No installation, commands or proofs were run; no blanket representation recommendation follows.
