# Exploderous project plan

## Direction

Build a Bomberman-inspired original in Rust, with correctness as a first-class goal. Choose explicit gameplay rules rather than assuming compatibility with a particular original game.

Deliver small, usable increments. Early headless increments are runnable and testable; the first human-playable milestone uses a basic TUI. Development is not a fixed linear roadmap: working capabilities accumulate and later priorities can change.

For gameplay vocabulary, see [GLOSSARY.md](../GLOSSARY.md). For the simulation seam, see [ADR-0001](adr/0001-headless-deterministic-core.md). The [game-state design](design/game-state.md) and [ADR-0002](adr/0002-grid-terrain-and-positioned-objects.md) define terrain/object separation, precise movement, simulation state, and verification implications.

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
| Movement and collision | Headless arena and integer sub-cell movement with an explicit character footprint | Rule tests cover body bounds, clear-path progress, and whole-path stopping at obstacles; compare bounded scenarios against an independent movement reference |
| Movement TUI playtest | Keyboard input, visible sub-cell movement, and real-time pacing around the movement core | In a compatible terminal, a human can navigate, stop, turn, reset, and quit; normal exit and handled failures restore the terminal |
| One-bomb mechanics | Placement, fuse, blasts, block destruction; extend the existing TUI to exercise them | Rule tests cover placement restrictions, timing, propagation, and lingering hazards |
| Complete first level | Exit objective, death, victory, explicit restart, integrated into the TUI | Scripted and terminal play can complete and restart the arena; tests cover death-before-victory and restoration on restart |

Each slice gets specific behavior and verification acceptance criteria before implementation. It is complete only when those criteria pass. Do not build empty advanced-testing harnesses into the skeleton.

Keep dependencies and project structure minimal. Introduce abstractions and optional build features only for concrete needs, not a speculative frontend or capability matrix.

### Next slice: movement and collision

Implement terrain, validated rules/scenario construction, one human-controlled character, whole-game tick movement, read-only observations, and complete cloning. Use character records/identities that support adding characters without a representation redesign.

Do not scaffold dormant enemy variants, bombs, blast/item collections, an exit field, AI/RNG state, networking, or generic adapters in this slice. Those parts of the full design are implemented with their corresponding gameplay features.

The [movement slice spec](https://github.com/qrilka/exploderous/issues/1) resolves the initial movement parameters and defines concrete validation/error acceptance criteria. Implement its worked movement cases, rejected-input state preservation, replay/cloning checks, and bounded comparison against the independent movement reference through the shared whole-game interface. Advanced-tool setup remains separate explicit verification work.

### Following slice: movement TUI playtest

After the movement slice, build a minimal TUI around the same validated construction, tick-input, and read-only observation interface. This is a movement playground, not yet a complete Normal level:

- One fixed classic arena with corridors, walls, and a few blocks; one character.
- Real-time pacing at the movement spec's nominal tick rate, independent of rendering. The frontend supplies tick frames; the core remains the sole owner of movement and collision.
- A display that makes sub-cell movement visible, plus a small readout of position, movement distance, and footprint side length. Display rounding must not change simulation coordinates.
- Direction controls, reset by constructing a fresh game, clean exit, and safe terminal-resize handling. Clear held-input state on reset.
- No bombs, objectives, arena editor, random generation, menus, or generic frontend framework. Add later mechanics to this TUI as their core slices arrive.

Require [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) support, including event-type reporting and reporting all keys as escape codes. This supplies press/repeat/release events even for unmodified letter keys. Require the capabilities, not a particular terminal application; any intermediary such as a terminal multiplexer must preserve them. Negotiate and verify the required enhancements at startup. Unsupported environments get an actionable error and terminal cleanup, not an OS-key-repeat fallback. Movement follows held-key state derived from press/release events; autorepeat does not control simulation speed. Restore the previous keyboard mode and other terminal state on normal exit and handled failures.

Before implementation, the TUI slice spec must settle held-key conflicts, display resolution and minimum size, resize/focus behavior, pacing/catch-up policy, and the test seam. Its human-playtest gate complements automated rule checks; scoped Kani and mutation-testing work remains interleaved as described below.

## First level

One fixed Normal arena with a safe starting position and an exit hidden under a designated block. Destroy that block and reach its cell with the character's centre alive to win.

Initially omit enemies, power-ups, random arena generation, score, a lives counter, a level deadline, and pre-placed bombs. The player has bomb capacity one. Chain-reaction implementation and tests belong to a later multi-bomb slice.

Death or victory stops gameplay until explicit restart restores the initial arena and gameplay state.

### Simulation and action rules

- Real-time play uses fixed simulation ticks and integer sub-cell movement. The adapter controls pacing; rendering frequency and TUI display precision do not define game time or collision geometry.
- The initial character has a square footprint smaller than a cell. Cardinal movement checks the whole traveled path and stops at the first obstacle; exact edge contact is allowed. There is no automatic sideways alignment or corner sliding initially.
- Resolve movement, then placement, then explosions, then harm, then victory. Moving clear before an expiring bomb detonates can save the player if the final footprint is clear of its new blast; crossing an already-live blast remains harmful.
- Bomb placement uses the character's centre cell without snapping its position. A newly placed bomb starts with its full fuse; a live blast can trigger it immediately.
- Walls, blocks, and bomb cells block movement. Passage permission for a newly placed bomb lasts until the placing character's whole footprint clears that cell. After permission ends, re-entry is blocked while the bomb remains undetonated.
- Blocked movement may stop partway along the requested path. Failed placement has no placement effect; neither stops simulation time or unrelated effects. Malformed input frames, in contrast, are rejected before any state advancement.
- A blast includes the bomb's cell and rays along four cardinal directions. Walls stop rays unharmed. A block is destroyed and stops the ray that reaches it.
- Each blast has fixed cell coverage and its own tunable lifetime. Body overlap with live blasts is lethal, including along the movement path; exact edge contact is safe. Overlapping blasts expire independently, and live blasts can trigger bombs.
- Harm precedes victory: reaching the exit in a tick that kills the player is not a win.

Choose blast reach, fuse duration, blast lifetime, and their exact phase boundaries in the bomb slice. Keep gameplay parameters tunable for playtesting; define input sampling before implementing real-time pacing.

### Later multi-bomb rule

When multiple bombs become possible, a blast hitting another bomb triggers its detonation in the same tick. All blasts in an explosion phase use the obstacles present at the start of that phase: destroying a block does not open a path for another same-phase ray. Verify that bomb processing order cannot change blast reach.

This rule is agreed direction, not a requirement to implement chain reactions in the first level.

## Verification strategy

Correctness means conformance to the chosen rules and named invariants. Each check states its scope; passing a bounded proof does not establish whole-game correctness.

Example tests accompany each rule. Introduce advanced methods through explicit verification slices interleaved with gameplay work:

- **Deterministic simulation testing (DST):** once stepping the engine is meaningful, drive input sequences without real time or UI. Check gameplay invariants as well as reproducibility; replaying a bug consistently is not evidence that the rules are correct. Capture initial state, inputs, parameters, and any random seed needed to reproduce failures.
- **Kani:** introduce a scoped proof for a bounded critical rule, such as movement/collision. Select and pin a Kani version and verify compiler compatibility; the repository's Rust pin does not select Kani's compiler. Record valid-state assumptions, explored bounds, the property asserted, and limitations. Use the [representation research](research/kani-representation-constraints.md) as evidence, not a completed compatibility check.
- **Mutation testing:** once rule tests exercise meaningful logic, challenge their sensitivity. Define the assessed scope and review surviving mutants rather than treating a percentage as proof of correctness.

Each tool-introduction slice has an explicit pass criterion. Select the exact harness, tooling versions, CI placement, and practical runtime budget in that slice. The named techniques are planned project work, not indefinite optional follow-ups; further techniques may be added when useful.

## Later capabilities

After the first playable level, add gameplay features in small verified slices. Introduce local Battle and Versus before network play. Reprioritize these additions as the project develops rather than treating this document as a rigid schedule. Each slice resolves its relevant [open design decisions](design/game-state.md#open-design-decisions) and records concrete rules and completion gates before implementation.

- Enemies, power-ups, additional arenas, and multi-bomb mechanics are later gameplay candidates.
- Evaluate network approaches when network work begins; do not commit to a topology, synchronization protocol, or rollback scheme now.
- Evaluate a graphical frontend, potentially Bevy, without making it the owner of gameplay rules.
- Evaluate browser support, potentially WebAssembly, when that target becomes concrete.

Future frontends and transports should reuse the same rules. Their detailed requirements and implementation choices remain deliberately open.
