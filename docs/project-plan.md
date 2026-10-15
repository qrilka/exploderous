# Exploderous project plan

## Direction

Build an original Bomberman-inspired game in Rust, treating correctness as a first-class goal. Choose explicit gameplay rules rather than assuming compatibility with a particular original game. Deliver usable capabilities incrementally; priorities may change as the project develops.

The plan is a roadmap, not a feature specification or issue-status tracker. Detailed scope, decisions, and acceptance gates belong in GitHub spec issues and implementation tickets; lasting gameplay rules belong in the [game-state design](design/game-state.md), and consequential rationale belongs in ADRs. See the [issue workflow](agents/issue-tracker.md).

For gameplay vocabulary, see [GLOSSARY.md](../GLOSSARY.md). The [simulation ADR](adr/0001-headless-deterministic-core.md) records the headless deterministic core decision. The [representation ADR](adr/0002-grid-terrain-and-positioned-objects.md) and game-state design define the current simulation model.

## Roadmap

1. **Expand gameplay in small verified increments.** Enemies, power-ups, additional arenas, and multi-bomb mechanics are candidates, not a fixed schedule. Reprioritize as the project develops.
2. **Add local competitive play before network play.** Introduce Battle and Versus before choosing network topology or synchronization behavior.
3. **Evaluate other frontends and targets when concrete.** A graphical frontend, potentially Bevy, and browser support, potentially WebAssembly, remain options. Keep gameplay rules independent of presentation and transport.

The [Bomberman ’93 manual](https://dds.konami.com/games/manual/pcemini/en_Bomber93.pdf) is one reference, not an exhaustive specification. Exploderous treats Normal, Battle, and Versus as separate gameplay presets; detailed mode rules and participant limits are settled when those capabilities are planned.

## Verification strategy

Correctness means conformance to the chosen rules and named invariants. Add example tests with each behavior, and make every bounded check state its scope; a bounded proof does not establish whole-game correctness.

Introduce verification work alongside gameplay, through explicit issues with pass criteria:

- **Deterministic simulation testing (DST):** drive input sequences without real time or UI. Check gameplay invariants as well as reproducibility; capture initial state, inputs, parameters, and any random seed needed to reproduce failures.
- **Kani:** use scoped proofs for bounded critical rules. Select and pin a Kani version, check compiler compatibility, and record valid-state assumptions, explored bounds, asserted properties, and limitations. The [corridor-entry proof task #7](https://github.com/qrilka/exploderous/issues/7) proposes bounded verification of the movement progress property. The [representation research](research/kani-representation-constraints.md) is evidence, not a completed compatibility check.
- **Mutation testing:** once rule tests exercise meaningful logic, assess whether they detect deliberate faults. Define the assessed scope and review surviving mutants; a percentage is not proof of correctness.

Tool versions, harness details, CI placement, and runtime budgets are decided in the relevant issue. These techniques are planned project work, not indefinite optional follow-ups.
