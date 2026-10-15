# Game state and simulation design

Agreed representation and behavioral contract for the headless core. See the [project plan](../project-plan.md), [glossary](../../GLOSSARY.md), and [representation ADR](../adr/0002-grid-terrain-and-positioned-objects.md).

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

Classic arenas use an odd-sized playable interior surrounded by a one-cell ring of walls. Wall pillars occur at every other interior row and column: with zero-based interior coordinates, both coordinates are odd. With the outer ring included, both pillar coordinates are even. The other interior cells form corridors and may contain floor or authored blocks. This is not a checkerboard of isolated floor cells.

For example, `#` denotes a wall and `.` denotes floor:

```text
#######
#.....#
#.#.#.#
#.....#
#.#.#.#
#.....#
#######
```

Odd interior dimensions let the alternating pattern begin and end with corridors; adding two border cells preserves odd total dimensions. This is a classic layout rule, not a universal terrain-storage invariant. General scenario construction also accepts valid even-sized rectangles and layouts without a wall ring. Concrete level layouts choose their playable size.

## Characters and precise positions

```rust
// Centre coordinates; the movement spec defines the scale and limits.
struct WorldX(i32);
struct WorldY(i32);
struct Position {
    x: WorldX,
    y: WorldY,
}

struct Column(u32);
struct Row(u32);
struct CellCoord {
    column: Column,
    row: Row,
}

struct Character {
    id: CharacterId,
    position: Position,
    // Common state plus compiled-in kind-specific state and control source.
}
```

`Position` is the character's centre in integer world units, not a terrain cell coordinate or a screen pixel. Its axis-specific newtypes prevent swapping world X and Y. `CellCoord` likewise keeps Column distinct from Row; it is a grid address, not the physical Cell area. The origin is the arena's outer top-left corner; X increases rightward and Y downward. One fixed scale relates world units to cell side length. `Arena` is the canonical conversion point: it maps a Position to its containing in-bounds `CellCoord`, maps an in-bounds `CellCoord` to its world-space centre, and owns row-major terrain indexing. Cell assignment uses consecutive lower-inclusive, upper-exclusive intervals; negative or out-of-Arena positions have no containing cell. A footprint can span several cells regardless of its centre's cell.

The initial bomber's footprint is a square whose sides remain horizontal and vertical. Its side length is smaller than a cell so one-cell-wide corridors allow some positioning error. The allowed deviation from a corridor's centreline is half the difference between the cell and footprint side lengths. Rectangles overlap only when their intersections have positive length on both axes; edge or corner contact alone is safe.

Movement:

- Receives at most one cardinal direction per character per tick, with displacement determined by the game's rules. A command applies only to its tick; omission means no movement, not continuation of a remembered direction. Frontends represent held input with repeated tick commands. Initial movement parameters are fixed at construction; later dynamic effects require explicit gameplay rules.
- Checks the entire path of the footprint, not merely its centre or destination. Stop at the furthest legal position at the first obstacle; exact edge contact is allowed, penetration is not.
- Keeps the complete footprint inside arena bounds and outside terrain forbidden by that character's rules.
- Uses the explicit turn-assistance rule below when enabled, not unrestricted corner sliding or frontend alignment.

### Turn assistance

`GameRules` holds movement and bomb parameters together, selects assisted or strict movement before Scenario construction, and remains fixed for the Game's lifetime. Assistance is enabled by default; strict mode uses only ordinary cardinal movement. The canonical Footprint remains 192 world units and movement distance remains 16. At the 256-unit Cell scale, that Footprint allows centre offsets of ±32 in a one-Cell corridor.

For each commanded tick:

1. Calculate ordinary cardinal movement with full-path collision checks. If it completes the requested distance, use that result unchanged.
2. If ordinary movement is shortened, consider corrections in both perpendicular directions, up to 128 world units of actual sideways correction, inclusive (half a Cell). This limit applies to all valid Footprint sizes and movement distances; offset from the current Cell centreline does not determine eligibility.
3. Choose the smallest positive integer correction for which the entire perpendicular path is legal and a full requested forward step from the corrected position is legal. A correction may point away from the current Cell centreline or cross a Cell boundary. Both segments respect the complete Footprint, terrain, and arena bounds; a legal destination does not justify crossing an intervening obstruction.
4. If such a correction exists, begin at the original position for the tick. Spend the shared movement-distance budget on correction first, then any remainder on forward movement. Otherwise use the ordinary result, including any partial progress. Do not drift sideways if the proposed forward path remains blocked.

At most one perpendicular direction can have a safe entry within this limit. Ordinary movement shortened by terrain overlaps at least one blocking Cell along its proposed forward path. Clearing opposite sides of that Cell requires entry positions separated by at least one Cell plus the Footprint side, greater than the 256 units spanned by two opposite 128-unit corrections. Thus the current terrain/Footprint model needs no tie-break. A forward arena-boundary violation cannot be repaired by perpendicular correction.

A correction larger than the budget takes multiple commanded ticks. Eligibility and the nearest legal correction are recomputed each tick; there is no remembered direction or pending alignment. Empty input stops immediately, and a changed command uses its own geometry. Once ordinary movement becomes legal, assistance stops: it permits entry, not guaranteed centering. For example, at the canonical defaults, a corridor entry requiring 32 units of sideways correction consumes two 16-unit ticks; forward travel resumes on the next tick, potentially still 32 units off-centre. Clear cardinal travel is never recentered.

The core owns both path segments and every position change. Frontends only select rules at startup, send ordinary per-tick commands, and display observations. Any later movement-path consumers (such as blast contact) must account for both actually traveled segments in their order rather than infer a straight line between tick endpoints.

Character records own positions. There is no second authoritative character-occupancy grid. Initially, occupancy queries scan the records. This has an O(n) cost per query; add a derived spatial index only if profiling shows these scans matter.

Character identities remain stable through removal/reordering of other records, and are not reused during the same game. They are distinct from both positions and vector indices. Any identity allocator/exhaustion state is part of the simulation; identities must never silently wrap.

Kinds are explicit compiled-in variants with kind-specific state, not arbitrary property bags, a plugin system, or a trait hierarchy. Control source is separate from kind: a bomber can be human-controlled or AI-controlled, while Normal can also have distinct enemy kinds. Overlap legality is decided by rules, not a universal one-character-per-cell storage invariant.

## Bombs and escape permissions

```rust
struct Bomb {
    id: BombId,
    cell: Cell,
    owner: CharacterId,
    detonate_at: Tick,
}

struct BombEscapePermission {
    character: CharacterId,
    bomb: BombId,
}
```

The records above illustrate the fuller identity-based model. For the current single-character, capacity-one implementation, the Game instead stores one optional Bomb with its escape-permission flag attached. Removing the Bomb removes its permission; no separate Bomb identity allocator or permission collection is needed. Revisit this representation when multiple live Bombs or characters are introduced.

Bombs are cell-anchored. Placement selects the cell containing the character's centre and does not move or snap the character. A live bomb occupies its cell for movement collision. Ownership lets capacity be enforced per character; capacity counts undetonated bombs, not lingering blasts.

The placing character receives passage permission for that specific bomb. Permission ends once the whole footprint clears the bomb cell, or the bomb ceases to exist. Other bombs still block movement. Permission is authoritative gameplay state included in cloning/replay, not inferred solely from the character's current centre.

A newly placed bomb receives its full fuse; a live blast can trigger it immediately. Store absolute simulation-tick deadlines, never wall-clock deadlines or decrementing countdowns. A bomb placed during tick P with positive fuse F detonates during tick P + F unless triggered earlier. Placement selects the centre-containing cell after that tick's movement.

Detonation removes the live bomb and produces a blast. Bomb capacity can therefore be released while the resulting blast remains dangerous. Bomb identities used by permissions must not alias a later bomb.

## Blasts across cells and ticks

```rust
struct Blast {
    cells: Vec<Cell>,
    expires_at: Tick,
}
```

One blast record covers a set of cells and lasts for several simulation ticks. Compute its unique, in-bounds coverage at detonation using the selected propagation rules. Coverage includes the origin and cardinal rays; reach counts cells beyond the origin in each direction. Walls remain unharmed and stop rays. A block is destroyed, included in the blast, and stops its ray. The covered cells remain fixed for that effect's lifetime; later terrain changes do not extend an existing blast.

Each blast has its own lifetime. If A and B overlap, expiry of A must not remove B's danger. Cell danger is derived from live effect records; an independent mutable `is_on_fire` terrain flag would duplicate truth. An acceleration structure would be a derived index. Source attribution and other effect-specific state are required only by rules that depend on them.

Damage uses footprint overlap, not just the centre's cell. Check the traveled path against blasts already live during movement, including both actual turn-assistance segments. A blast created after movement checks the final footprint, consistent with actions preceding explosions. Exact edge contact is safe; overlap is harmful.

A blast can trigger another bomb in the same tick. All propagation in that explosion phase uses the terrain present at its start: destroying a block does not open a route for a second same-phase ray. Bomb processing order must not change blast reach. Even with capacity one, an earlier lingering blast can trigger a newly placed bomb; capacity counts undetonated bombs rather than live effects.

### Tick phases and deadline boundaries

The tick label T denotes the resulting tick of a step advancing from T - 1. During active play, expire blasts whose deadline is T before movement, then resolve movement, placement, explosions, harm, and victory in that order. Preserve path-contact evidence for harm from blasts live during movement; a safe endpoint does not undo crossing a hazard.

A blast created on tick D with positive lifetime L has an absolute expiry deadline D + L. It is dangerous on creation and through tick D + L - 1, then expires before movement on D + L. For example, a bomb placed on tick 10 with a 120-tick fuse detonates on tick 130; with a 30-tick blast lifetime, its blast is dangerous through tick 159 and gone before movement on tick 160. Initial tuning values are owned by the [first playable level spec](https://github.com/qrilka/exploderous/issues/9).

Use checked addition for tick advancement and any required bomb/blast deadline. An unrepresentable deadline rejects the entire step with the existing time-exhaustion error and preserves complete state, including terrain, position, permissions, and effects. Do not wrap, saturate, or silently shorten timers. A placement blocked by ordinary capacity/occupancy rules remains a valid gameplay action, not an arithmetic failure.

With one placement request per tick and capacity one, at most one bomb can detonate per tick. A finite blast lifetime therefore bounds the number of live effects without an additional smaller effect cap. Do not silently discard live hazards to enforce a storage shortcut. Multi-bomb resource policies remain separate work.

## Items and exit coexist with terrain

```rust
struct Item {
    cell: Cell,
    kind: ItemKind,
}

// Direct field on game state:
// exit: Option<Cell>
```

Items are separate cell-anchored records. Their kinds distinguish specified power-up effects, such as increased bomb capacity or blast reach. The exact catalog remains open.

An item under a block is hidden and cannot be collected. Destroying the block reveals the item rather than deleting its record. Collection uses the character's centre entering its cell. Consumption removes the item record and applies its specified effect to the character.

Game state stores a single optional exit directly as `exit: Option<Cell>`. Scenarios without an exit have no exit objective; the initial Normal level requires one.

Exit visibility and eligibility are different. Terrain determines whether it is covered. Objective rules determine whether it can complete the level. Initially, uncover it and reach its cell with the character's centre alive. Later enemy-clear requirements belong to objective rules, not to terrain or a redundant visibility flag. Harm precedes victory.

Example coexistence:

| Object/location | State |
| --- | --- |
| Terrain at `(3, 2)` | Block |
| Exit | `(3, 2)` |
| Item at another covered cell | Its own kind record |
| Character on floor | Its precise footprint may span cells |
| Bomb beneath that character | Separate live bomb and escape permission |
| Two blasts sharing a floor cell | Separate coverage/lifetime records |

Power-up effects, item destruction by blasts (especially newly revealed items), multiple-item policies, and simultaneous pickup conflicts remain open design decisions. They are not silently chosen by the container layout.

## Level outcomes and restart

The initial Normal level has one life per run. Lethal blast contact ends the run in defeat; alive centre entry into the uncovered exit ends it in victory. Harm precedes victory even if both conditions arise during the same tick.

Either outcome freezes the complete simulation, including tick progression, movement, placement, and bomb/blast deadlines. A frontend stops any catch-up batch immediately when the outcome occurs. It displays a distinct outcome and accepts only restart or quit, not movement, placement, or active-play help.

Restart creates a fresh run from the original scenario and selected rules: initial terrain, covered exit, character position, initial tick, active lifecycle, and available bomb capacity. It removes bombs, blasts, and escape permissions. Frontends also clear held directions, pending placement, and elapsed-time backlog; no old action or paused time leaks into the new run.

## Complete authoritative state

The complete simulation state owns:

- Rules/parameters and resource limits used by that game.
- Current tick and lifecycle/outcome state.
- Arena dimensions and current terrain.
- Character identities, positions, kind-specific state, and control source.
- Bombs, live blast effects, items, and the optional exit.
- Bomb-escape permissions.
- AI policy state, complete random-generator state, and identity allocation state where applicable.
- The initial definition/state needed for restart.

An immutable restart definition and mutable current terrain have different roles; they are not competing copies of live state. Likewise, an initial random seed alone cannot replace the current generator state after random choices have occurred.

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

Object-interaction properties include release of bomb capacity while a blast persists, expiry of one overlapping blast, hidden item/exit reveal, escaped-bomb re-entry, paths crossing live hazards, and death before victory. Check exact detonation/expiry boundaries, atomic deadline-overflow rejection, frozen outcome state, and complete restart restoration through whole-game stepping and read-only observations.

[Kani research](../research/kani-representation-constraints.md) supports bounded vectors as well as arrays; it does not establish comparative proof cost. Construct small valid arenas through the production interface, state assumptions/dimensions/action horizons, and retain unwinding checks. Small arenas do not prove larger sizes or unbounded play; arithmetic/limit cases need separately scoped checks. Kani uses its own toolchain, not automatically the repository's Rust pin; compatibility and practical proof cost are not established by this design.

## Open design decisions

- Additional movement rules for later character kinds or dynamic effects; initial movement parameters and input/errors are defined in the movement slice spec.
- Bomb/blast resource policies when capacity greater than one or additional action sources are introduced; single-character, capacity-one timing and deadline-overflow rules are settled above.
- Multi-character interactions: conflicting moves, swaps, and contact rules.
- AI: decision ordering and random-consumption conventions.
- Objectives and items: additional completion conditions, power-up catalog/effects, item destruction/reveal behavior, multiple-item policies, and pickup conflicts.

The representation accommodates these choices without silently defining them. Gameplay dimensions, timing, and body size remain explicit parameters where tuning is needed; terminal display and unchecked integer types are not their source of truth. Delivery scope and verification-tool rollout belong in the relevant [GitHub spec issue or implementation ticket](../agents/issue-tracker.md); the [project plan](../project-plan.md) records broad direction and priorities.
