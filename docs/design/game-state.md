# Game state and simulation design

Agreed representation and behavioral contract for the headless core. This includes the approved generated level for Normal mode with Buroru and Fire-up; approval does not imply those capabilities are already implemented. The [README](../../README.md) describes delivered capabilities. See the [project plan](../project-plan.md), [glossary](../../GLOSSARY.md), and [representation ADR](../adr/0002-grid-terrain-and-positioned-objects.md).

Characters use integer sub-cell positions and movement. Rust sketches below explain data relationships rather than complete declarations. Names such as `Cell`, `Tick`, and identity types denote distinct concepts. The [movement slice spec](https://github.com/qrilka/exploderous/issues/1) defines the movement scale, defaults, limits, and validation/error contract. [Turn assistance](https://github.com/qrilka/exploderous/issues/5) deliberately revises its initial no-alignment rule and Cartesian verification matrix as specified below; remaining choices are listed under [Open design decisions](#open-design-decisions).

## Terrain grid

An arena is a finite rectangle. Width and height are chosen at construction and remain fixed during that game; different games can use different dimensions.

```rust
// Illustrative representation.
enum Terrain {
    Floor,
    Wall,
    Block,
}

struct Arena {
    width: u32,
    height: u32,
    terrain: Vec<Terrain>,
}
```

- **Floor:** terrain that permits movement and blast propagation, subject to other objects and rules.
- **Wall:** indestructible terrain that blocks movement and blasts. This represents the reference's non-blowable boulders.
- **Block:** destructible terrain that blocks movement; a blast destroys it and stops that ray.

The vector stores rows consecutively. `Arena` owns the checked conversion between a `CellCoord` and its row-major index (`row * width + column`), plus terrain lookup and iteration. Callers do not reproduce this arithmetic. The vector length must equal the validated cell count.

Characters, bombs, blasts, items, and the exit do not replace terrain values. For example, a block and a concealed exit can share a cell. Destroying the block changes terrain to floor without erasing the exit.

The outside of the arena is solid. A surrounding ring of wall cells is not required by storage; small test arenas may have floor at their edges.

### Classic arena layout

Classic arenas use an odd-sized playable interior with wall pillars at every other row and column: with zero-based interior coordinates, both coordinates are odd. The other interior cells form corridors and may contain floor or blocks. This is not a checkerboard of isolated floor cells.

For example, `#` denotes a wall pillar and `.` denotes floor; the exterior boundary is not stored in this grid:

```text
.....
.#.#.
.....
.#.#.
.....
```

Odd interior dimensions let the alternating pattern begin and end with corridors. This is a classic layout rule, not a universal terrain-storage invariant. General scenario construction also accepts valid even-sized rectangles and explicit wall rings. A frontend may draw a separate exterior boundary without adding terrain cells or shifting gameplay coordinates.

### Generated level for Normal mode

The generated level for Normal mode stores a **13×13 playable Arena**, coordinates 0 through 12, with no outer wall-cell ring. The outside remains solid. The TUI draws its exterior boundary separately and distinguishes it visually from interior wall pillars.

Generation produces a validated Scenario with:

- Fixed classic pillars and a target Block density of about 40% among eligible non-pillar Cells, excluding reserved starting spaces. Density is a tuning value, not an exact count invariant.
- One human-controlled bomber at the centre of `(0, 0)`. Cells `(0, 0)`, `(1, 0)`, `(2, 0)`, `(0, 1)`, and `(0, 2)` are reserved Floor, providing escape room for the first Bomb.
- Exactly three Buroru on distinct Floor Cells. Each starts at least six corridor steps from the player, measured along cardinal routes treating Walls as impassable and ignoring Blocks. Each has at least one adjacent Floor Cell.
- Enemy pockets separated by Blocks are permitted. All non-Wall areas connect after removing Blocks; pockets do not imply an initially open route to the player.
- One Fire-up and one Exit under two different Blocks.

Generation belongs to headless gameplay rather than the renderer. Given the same generation parameters and random state, it produces the same Scenario. Starting the application or restarting requests fresh generation; restart does not restore the previous generated Scenario. Fresh generation advances generation random state rather than reseeding every restart with the same value. Random generation may coincidentally repeat a layout; freshness means a new generation, not a promise of global uniqueness. Invalid or impossible generation parameters must fail explicitly rather than loop indefinitely.

## Characters and precise positions

See [Character representation and movement](character-movement.md#characters-and-precise-positions).

### Turn assistance

See [Turn assistance](character-movement.md#turn-assistance).

### Buroru movement and Character contact

See [Buroru movement and Character contact](character-movement.md#buroru-movement-and-character-contact).

## Bombs and escape permissions

See [Bombs and escape permissions](bombs-and-blasts.md#bombs-and-escape-permissions).

## Blasts across cells and ticks

See [Blasts across Cells and ticks](bombs-and-blasts.md#blasts-across-cells-and-ticks).

### Tick phases and deadline boundaries

See [Tick phases and deadline boundaries](bombs-and-blasts.md#tick-phases-and-deadline-boundaries).

## Items and exit coexist with terrain

See [Items and Exit](bombs-and-blasts.md#items-and-exit-coexist-with-terrain).

## Level outcomes and restart

Normal mode has one life per run. Lethal Blast or player–enemy centre contact ends the run in Defeat. A living player's centre on the uncovered Exit with no surviving enemies ends it in Victory. Harm precedes victory even if both conditions arise during the same tick.

Either outcome freezes the complete simulation, including tick progression, movement, placement, and bomb/blast deadlines. A frontend stops any catch-up batch immediately when the outcome occurs. The TUI displays defeat in a centered modal over the Arena with readable restart/quit instructions. The outcome view accepts only restart or quit, not movement, placement, or active-play help.

Restart in Normal mode creates a newly generated Scenario under the selected rules, rather than replaying the previous one. The fresh Game begins at tick zero with its new terrain, covered Exit/Fire-up, player/enemy starting state, active lifecycle, initial blast reach, and available Bomb capacity. It removes old Bombs, Blasts, escape permissions, consumed/destroyed Items, and AI state. Frontends clear held directions, pending placement, and elapsed-time backlog; no old action or paused time leaks into the new run.

Constructing a Game twice from one explicit Scenario still reproduces that Scenario. Deterministic scenario replay and the frontend's regenerate-on-restart action have different purposes.

## Complete authoritative state

The complete simulation state owns:

- Rules/parameters and resource limits used by that game.
- Current tick and lifecycle/outcome state.
- Arena dimensions and current terrain.
- Character identities, positions, kind-specific state, and control source.
- Bombs, live blast effects, items, and the optional exit.
- Bomb-escape permissions.
- AI policy state, complete random-generator state, and identity allocation state where applicable.
- The Scenario definition needed to reproduce a particular run. A session starting fresh generated runs also retains generation random state, separately from the active Game's AI state.

An immutable Scenario definition and mutable current terrain have different roles; they are not competing copies of live state. Likewise, an initial random seed alone cannot replace the current generator state after random choices have occurred. Cloning an active Game reproduces its future, whereas a restart in Normal mode advances session generation state to request a fresh Scenario.

## Simulation interface and invariants

Use one small shared interface for frontends and checks:

1. Construct validated rules and a validated scenario, then instantiate a game with its starting tick.
2. Advance the entire game by exactly one tick with an input frame.
3. Inspect a read-only observation.
4. Clone complete in-memory simulation state for an independent continuation.

Scenario validation binds starting object placement to the scenario's rules. `Game::new(scenario, initial_tick)` takes ownership of those validated values without replacing rules or repeating validation; only the game stores and advances the tick. See [Scenario and Game](../../GLOSSARY.md) for the domain distinction.

Keep mutable storage private. Scenario data may be authored externally, but only validated construction creates live state. Tests use that same construction/step interface; no privileged setters that bypass invariants. Valid scenarios need not come with a full replay proving reachability.

Validate dimensions, cell-count arithmetic/storage length, coordinate ranges and footprint placement, rule parameters, unique identities and references, and explicit resource ceilings. Kind-specific relationships are part of the validation contract. Invalid input must be reported, not turned into indexing panics or silently wrapped arithmetic. Initial movement ceilings are defined in the movement spec; limits for later objects remain open. The Kani harness's smaller bounds are not the game's maximum size.

A `CharacterCommand` carries a character identity, optional movement direction, and a bomb-placement flag together. One frame contains at most one such command per addressed character. Missing human input means no action; an empty valid frame still advances time unless the tick counter is exhausted. The movement spec defines atomic rejection at that numeric limit. Externally submitted commands target human-controlled characters. NPC decisions run within the deterministic simulation using explicit AI/random state.

Reject duplicate commands, unknown identities, and external commands targeting NPCs before any tick, AI, or random-state advancement. A valid blocked action is ordinary gameplay: movement may stop partway, a failed placement has no placement effect, and time still advances. Input enumeration order must not affect results. Processing order and random consumption must be explicit rather than accidental collection order.

Cloning captures all state affecting future behavior, not just what is drawn. Same initial state, rules, and future input frames must yield the same complete state for the same implementation. Saved-file formats, cross-version replay, network authentication/synchronization, and rollback protocols are outside this representation contract.

A TUI may approximate positions or draw multiple terminal columns per cell. It cannot quantize simulation positions or make terminal rendering the clock. The shared observation interface is read-only state, not an event stream.

## Testing implications

All methods exercise production construction and movement routines; there is no reduced production engine solely for verification.

| Method | Intended evidence |
| --- | --- |
| Coordinate conversions | Cell-boundary positions, negative/out-of-Arena positions, cell centres, and row-major access on non-square arenas |
| Worked examples | Each direction, exact contact, off-centre corridor entry, inclusive actual-correction-limit boundaries, shared correction/forward budget, partial stopping, crossed obstacles, borders, blocked and free paths in assisted and strict modes |
| Validation/error tests | Bad dimensions/lengths/positions/parameters, limits and arithmetic edges, duplicate identities/commands, and unchanged complete state on rejected frames |
| Corridor-entry progress | The bounded property below, exercised from both sides and rotated through all four directions, without predicting intermediate positions |
| Independent movement reference | On small scripted sequences in both modes, advance each candidate correction/forward path one world unit at a time using independent overlap geometry and compare the resulting position to production movement |
| Deterministic sequence tests | Check invariants throughout action sequences; capture initial state, parameters, inputs, and generator seed for reproduction |
| Clone/continuation tests | Clone at intermediate ticks, apply identical suffixes and compare complete state; independently branch without sharing mutable state |
| Kani | Scoped bounded proofs of collision/bounds safety, exact clear-path progress, and maximal legal stopping using actual routines |
| Mutation testing | Challenge direction/edge comparisons, stopping/progress checks, and validation sensitivity; review survivors in the stated scope |

The reference must not share production collision helpers whose defect could make both algorithms agree incorrectly. Use hand-worked examples and properties as well: agreement between two implementations is not a complete specification.

Safety alone is insufficient: a character that never moves stays out of walls. Require correct movement on clear paths and stopping without an unnecessary gap on blocked paths. Check that unrelated state is preserved while the tick and intended state advance.

### Bounded corridor-entry property

At the canonical 192-unit Footprint and 16-unit movement distance, author a clear one-Cell corridor reached from an open approach. Start at entrance contact, with the centre 96 units before the mouth and ordinary forward movement blocked by sideways placement. The entire correction route and the corridor ahead are unobstructed. For every required correction from 1 through 128 world units, from either side and in every commanded direction, repeated commands must carry the centre strictly past the mouth within 15 ticks. This is entry, not guaranteed centering or the whole Footprint clearing the entrance.

The bound follows from the geometry: at most 128 correction units plus 97 forward units to cross the mouth is 225 units, requiring at most 15 canonical ticks, fewer than 256/16. Arbitrary approach distances, other parameters, and blocked routes are outside this particular progress bound and retain their separate worked checks.

Check the complete Footprint along both observed path segments with independent overlap geometry and require total per-tick travel at most 16. Do not calculate expected intermediate positions by repeating assistance candidate selection. Enumerating the eligible corrections and rotations provides bounded evidence for this capability, not a proof for every arena and input sequence; unrelated parameter products are not the primary progress oracle.

### Other verification scope

Object-interaction properties include release of Bomb capacity while a Blast persists, expiry of one overlapping Blast, Fire-up/Exit reveal, protection from the revealing effect versus destruction by a distinct later effect, collection before placement, placement-time reach snapshots, temporary per-Character Bomb passage (including approach, crossing, and reversal before clearance), off-centre dead-end retreat, blocked re-entry after full clearance, centre paths crossing live hazards, simultaneous player–enemy centre contact, and harm before victory. Check exact detonation/expiry boundaries, atomic deadline-overflow rejection including AI/random state, and frozen outcome state through whole-Game stepping and public observation. Observation means concrete terrain, Character positions/kinds, living enemies, Items, Bomb/Blast properties, abilities, tick, and outcome—not a test-only state mutation interface.

Test generated Scenario invariants through the same validated construction boundary, using reproducible random state. Check safe opening, enemy distances/adjacency, distinct covered Fire-up/Exit Cells, bounds/pillars, and connectivity after Block removal; statistical coverage or seed counts alone do not prove these properties. Check centred NPC travel, half-speed progress beyond the former four-Cell run limit, no reversal on clear routes, 90/10 straight-versus-side junction decisions, 75/25 reverse-versus-side obstruction decisions, forced alternatives, trapped waiting, and complete clone continuation without depending on a particular internal PRNG layout. Through frontend input/time/rendering boundaries, check the separate visual boundary, concealed/revealed objects, locked/ready Exit, and fresh-generation restart with cleared input/timing state. Do not require a randomized restart's terrain to differ by chance; verify that fresh generation is requested rather than restoring the old Game.

[Kani research](../research/kani-representation-constraints.md) supports bounded vectors as well as arrays; it does not establish comparative proof cost. Construct small valid arenas through the production interface, state assumptions/dimensions/action horizons, and retain unwinding checks. Small arenas do not prove larger sizes or unbounded play; arithmetic/limit cases need separately scoped checks. Kani uses its own toolchain, not automatically the repository's Rust pin; compatibility and practical proof cost are not established by this design.

## Open design decisions

- Additional movement rules for later character kinds or dynamic effects; initial movement parameters and input/errors are defined in the movement slice spec.
- Bomb/blast resource policies when capacity greater than one or additional action sources are introduced; single-bomber, capacity-one timing and deadline-overflow rules are settled above.
- Solid Character conflicts, swaps, and contact rules for modes beyond the specified bomber/Buroru interactions.
- AI policies beyond momentum-based Buroru wandering, including pursuit, hazard avoidance, and computer-controlled bombers. Implementations must make the specified policy's processing/random-consumption order explicit.
- Objectives and Items beyond this level's enemy-clear objective and Fire-up rules, including additional kinds, multiple-Item policies, and multiple-collector conflicts.

The representation accommodates these choices without silently defining them. Gameplay dimensions, timing, and body size remain explicit parameters where tuning is needed; terminal display and unchecked integer types are not their source of truth. Delivery scope and verification-tool rollout belong in the relevant [GitHub spec issue or implementation ticket](../agents/issue-tracker.md); the [project plan](../project-plan.md) records broad direction and priorities.
