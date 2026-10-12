# Exploderous project plan

## Direction

Build a Bomberman-inspired original in Rust, with correctness as a first-class goal. Choose explicit gameplay rules rather than assuming compatibility with a particular original game.

Deliver small, usable increments. Early headless increments are runnable and testable; the first human-playable milestone uses a basic TUI. Development is not a fixed linear roadmap: working capabilities accumulate and later priorities can change.

For gameplay vocabulary, see [GLOSSARY.md](../GLOSSARY.md). For the simulation boundary, see [ADR-0001](adr/0001-headless-deterministic-core.md).

## Reference

[Bomberman ’93 manual](https://dds.konami.com/games/manual/pcemini/en_Bomber93.pdf) is one reference, not an exhaustive specification:

- Printed pp. 6–7: Normal is single-player stage progression, requiring an exit and defeating all enemies.
- Printed pp. 12–13: Battle supports up to five human/computer-controlled participants, last-survivor rounds, draws on simultaneous elimination, and a preset round-win target.
- Printed p. 14: Versus is two humans on linked devices, essentially Battle without computer participants or arena selection.

Exploderous keeps Normal, Battle, and Versus as separate gameplay presets. Battle supports configurable arenas and human/AI participants; Versus is two humans in a fixed arena. Local versus network connection is independent of these presets. Detailed competitive rules and participant limits will be specified when those capabilities are introduced.

## Initial implementation slices

| Slice | Deliverable | Completion gate |
| --- | --- | --- |
| Rust skeleton | Minimal Cargo project and runnable placeholder executable | Build/run succeeds; CI runs formatting, Clippy, and tests |
| Movement and collision | Headless arena and tile-step movement | Rule tests cover boundaries and blocked movement |
| One-bomb mechanics | Placement, fuse, blasts, block destruction | Rule tests cover placement restrictions, timing, propagation, and lingering hazards |
| Complete first level | Exit objective, death, victory, explicit restart | A scripted successful playthrough completes the arena; tests cover death-before-victory and restoration on restart |
| Basic TUI | Terminal input/output and real-time pacing around the same core | Terminal play can complete and restart the level; exiting restores the terminal |

Each slice gets specific behavior and verification acceptance criteria before implementation. It is complete only when those criteria pass. Do not build empty advanced-testing harnesses into the skeleton.

Keep dependencies and project structure minimal. Introduce abstractions and optional build features only for concrete needs, not a speculative frontend or capability matrix.

## First level

One fixed Normal arena with a safe starting position and an exit hidden under a designated block. Destroy that block and reach the exit alive to win.

Initially omit enemies, power-ups, random arena generation, score, a lives counter, a level deadline, and pre-placed bombs. The player has bomb capacity one. Chain-reaction implementation and tests belong to a later multi-bomb slice.

Death or victory stops gameplay until explicit restart restores the initial arena and gameplay state.

### Simulation and action rules

- Real-time play uses fixed simulation ticks and discrete tile-step movement. The adapter controls pacing; rendering frequency does not define game time.
- Resolve movement, then placement, then explosions, then harm, then victory. Moving clear before an expiring bomb detonates can save the player, but only if the destination is outside harmful blast tiles.
- A newly placed bomb starts with its full fuse. A live blast can trigger it immediately.
- Walls, blocks, and bomb tiles block entry. The player may leave the bomb beneath them but cannot re-enter its tile while it remains undetonated.
- Failed movement or placement has no corresponding movement/placement effect; it does not stop simulation time or unrelated effects.
- A blast includes the bomb's tile and rays along four cardinal directions. Walls stop rays unharmed. A block is destroyed and stops the ray that reaches it.
- Blasts persist for a short, tunable duration. Entering a live blast is lethal, and live blasts can trigger bombs.
- Harm precedes victory: reaching the exit in a tick that kills the player is not a win.

Choose arena dimensions, blast reach, tick rate, movement cadence, fuse duration, and blast lifetime when specifying the relevant slice. Keep timing values tunable for playtesting. Define exact fuse/lifetime boundaries and input sampling before implementing timing behavior.

### Later multi-bomb rule

When multiple bombs become possible, a blast hitting another bomb triggers its detonation in the same tick. All blasts in an explosion phase use the obstacles present at the start of that phase: destroying a block does not open a path for another same-phase ray. Verify that bomb processing order cannot change blast reach.

This rule is agreed direction, not a requirement to implement chain reactions in the first level.

## Verification strategy

Correctness means conformance to the chosen rules and named invariants. Each check states its scope; passing a bounded proof does not establish whole-game correctness.

Example tests accompany each rule. Introduce advanced methods through explicit verification slices interleaved with gameplay work:

- **Deterministic simulation testing (DST):** once stepping the engine is meaningful, drive input sequences without real time or UI. Check gameplay invariants as well as reproducibility; replaying a bug consistently is not evidence that the rules are correct. Capture initial state, inputs, parameters, and any random seed needed to reproduce failures.
- **Kani:** introduce a scoped proof for a bounded critical rule, such as movement/collision. Record valid-state assumptions, explored bounds, the property asserted, and limitations.
- **Mutation testing:** once rule tests exercise meaningful logic, challenge their sensitivity. Define the assessed scope and review surviving mutants rather than treating a percentage as proof of correctness.

Each tool-introduction slice has an explicit pass criterion. Select the exact harness, tooling versions, CI placement, and practical runtime budget in that slice. The named techniques are planned project work, not indefinite optional follow-ups; further techniques may be added when useful.

## Later capabilities

After the first playable level, add gameplay features in small verified slices. Introduce local Battle and Versus before network play. Reprioritize these additions as the project develops rather than treating this document as a rigid schedule.

- Enemies, power-ups, additional arenas, and multi-bomb mechanics are later gameplay candidates.
- Evaluate network approaches when network work begins; do not commit to a topology, synchronization protocol, or rollback scheme now.
- Evaluate a graphical frontend, potentially Bevy, without making it the owner of gameplay rules.
- Evaluate browser support, potentially WebAssembly, when that target becomes concrete.

Future frontends and transports should reuse the same rules. Their detailed requirements and implementation choices remain deliberately open.
