# Game state and simulation design

Agreed representation and behavioral contract for the headless core. See the [project plan](../project-plan.md), [glossary](../../GLOSSARY.md), and [representation ADR](../adr/0002-grid-terrain-and-positioned-objects.md).

Characters use integer sub-cell positions and movement. Rust sketches below explain data relationships rather than complete declarations. Names such as `Cell`, `Tick`, and identity types denote distinct concepts; numeric choices not yet settled are listed under [Open design decisions](#open-design-decisions).

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

The vector stores rows consecutively. Cell `(column, row)` has index `row * width + column`, using checked dimensions, arithmetic, conversions, and coordinates. Its length must equal the validated cell count.

Characters, bombs, blasts, items, and the exit do not replace terrain values. For example, a block and a concealed exit can share a cell. Destroying the block changes terrain to floor without erasing the exit.

The outside of the arena is solid. A surrounding ring of wall cells is not required by storage; small test arenas may have floor at their edges.

## Characters and precise positions

```rust
// Illustrative representation; world-unit scale is not chosen yet.
struct Position {
    x: i32,
    y: i32,
}

struct Character {
    id: CharacterId,
    position: Position,
    // Common state plus compiled-in kind-specific state and control source.
}
```

Position is the character's centre in integer world units, not a terrain cell coordinate. One fixed scale relates world units to cell size. For illustration only, with 16 units per cell, centres `(24, 24)` and `(29, 24)` both lie in cell `(1, 1)`.

The initial bomber's footprint is a square whose sides remain horizontal and vertical. It is smaller than a cell so one-cell-wide corridors allow some positioning error. For a 16-unit passage spanning coordinates 16 through 32, a 12-unit body fits with its centre anywhere from 22 to 26; a 16-unit body requires centre 24 exactly. These numbers explain clearance, not final gameplay constants.

Movement:

- Receives one cardinal direction per character per tick, with displacement determined by the game's rules.
- Checks the entire path of the footprint, not merely its centre or destination. Stop at the furthest legal position at the first obstacle; exact edge contact is allowed, penetration is not.
- Keeps the complete footprint inside arena bounds and outside terrain forbidden by that character's rules.
- Does not automatically align to corridor centres or move sideways along obstacles. Any turn assistance would be an additional movement rule, not an implicit collision adjustment.

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

Bombs are cell-anchored. Placement selects the cell containing the character's centre and does not move or snap the character. A live bomb occupies its cell for movement collision. Ownership lets capacity be enforced per character; capacity counts undetonated bombs, not lingering blasts.

The placing character receives passage permission for that specific bomb. Permission ends once the whole footprint clears the bomb cell, or the bomb ceases to exist. Other bombs still block movement. Permission is authoritative gameplay state included in cloning/replay, not inferred solely from the character's current centre.

A newly placed bomb receives its full fuse; a live blast can trigger it immediately. Deadlines use simulation ticks, never wall-clock time. Exact fuse boundary conventions remain an open design decision.

Detonation removes the live bomb and produces a blast. Bomb capacity can therefore be released while the resulting blast remains dangerous. Bomb identities used by permissions must not alias a later bomb.

## Blasts across cells and ticks

```rust
struct Blast {
    cells: Vec<Cell>,
    expires_at: Tick,
}
```

One blast record covers a set of cells and lasts for several simulation ticks. Compute its unique, in-bounds coverage at detonation using the selected propagation rules. Coverage includes the origin and cardinal rays, respecting walls and blocks. The covered cells remain fixed for that effect's lifetime; later terrain changes do not extend an existing blast.

Each blast has its own lifetime. If A and B overlap, expiry of A must not remove B's danger. Cell danger is derived from live effect records; an independent mutable `is_on_fire` terrain flag would duplicate truth. An acceleration structure would be a derived index. Source attribution and other effect-specific state are required only by rules that depend on them.

Damage uses footprint overlap, not just the centre's cell. Check the traveled path against blasts already live during movement. A blast created after movement checks the final footprint, consistent with actions preceding explosions. Exact edge contact is safe; overlap is harmful. Lifetime/expiry boundaries must be specified so movement and harm agree about which effects are live.

A blast can trigger another bomb in the same tick. All propagation in that explosion phase uses the terrain present at its start: destroying a block does not open a route for a second same-phase ray. Bomb processing order must not change blast reach.

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

1. Construct a scenario and rules through validation.
2. Advance the entire game by exactly one tick with an input frame.
3. Inspect a read-only observation.
4. Clone complete in-memory simulation state for an independent continuation.

Keep mutable storage private. Scenario data may be authored externally, but only validated construction creates live state. Tests use that same construction/step interface; no privileged setters that bypass invariants. Valid scenarios need not come with a full replay proving reachability.

Validate dimensions, cell-count arithmetic/storage length, coordinate ranges and footprint placement, rule parameters, unique identities and references, and explicit resource ceilings. Kind-specific relationships are part of the validation contract. Invalid input must be reported, not turned into indexing panics or silently wrapped arithmetic. Numeric ceilings remain open; the Kani harness's smaller bounds are not the game's maximum size.

One frame contains at most one movement direction and, when supported, a placement request for each addressed character. Missing human input means no action; an empty valid frame still advances time. Externally submitted commands target human-controlled characters. NPC decisions run within the deterministic simulation using explicit AI/random state.

Reject duplicate commands, unknown identities, and external commands targeting NPCs before any tick, AI, or random-state advancement. A valid blocked action is ordinary gameplay: movement may stop partway, a failed placement has no placement effect, and time still advances. Input enumeration order must not affect results. Processing order and random consumption must be explicit rather than accidental collection order.

Cloning captures all state affecting future behavior, not just what is drawn. Same initial state, rules, and future input frames must yield the same complete state for the same implementation. Saved-file formats, cross-version replay, network authentication/synchronization, and rollback protocols are outside this representation contract.

A TUI may approximate positions or draw multiple terminal columns per cell. It cannot quantize simulation positions or make terminal rendering the clock. The shared observation interface is read-only state, not an event stream.

## Testing implications

All methods exercise production construction and movement routines; there is no reduced production engine solely for verification.

| Method | Intended evidence |
| --- | --- |
| Worked examples | Each direction, exact contact, off-centre corridor entry, partial stopping, crossed obstacles, borders, blocked and free paths |
| Validation/error tests | Bad dimensions/lengths/positions/parameters, limits and arithmetic edges, duplicate identities/commands, and unchanged complete state on rejected frames |
| Independent movement reference | On bounded scenarios, advance the footprint one world unit at a time and compare the furthest legal position to the production sweep |
| Deterministic sequence tests | Check invariants throughout action sequences; capture initial state, parameters, inputs, and generator seed for reproduction |
| Clone/continuation tests | Clone at intermediate ticks, apply identical suffixes and compare complete state; independently branch without sharing mutable state |
| Kani | Scoped bounded proofs of collision/bounds safety, exact clear-path progress, and maximal legal stopping using actual routines |
| Mutation testing | Challenge direction/edge comparisons, stopping/progress checks, and validation sensitivity; review survivors in the stated scope |

The reference must not share production collision helpers whose defect could make both algorithms agree incorrectly. Use hand-worked examples and properties as well: agreement between two implementations is not a complete specification.

Safety alone is insufficient: a character that never moves stays out of walls. Require correct movement on clear paths and stopping without an unnecessary gap on blocked paths. Check that unrelated state is preserved while the tick and intended state advance.

Object-interaction properties include release of bomb capacity while a blast persists, expiry of one overlapping blast, hidden item/exit reveal, escaped-bomb re-entry, paths crossing live hazards, and death before victory.

[Kani research](../research/kani-representation-constraints.md) supports bounded vectors as well as arrays; it does not establish comparative proof cost. Construct small valid arenas through the production interface, state assumptions/dimensions/action horizons, and retain unwinding checks. Small arenas do not prove larger sizes or unbounded play; arithmetic/limit cases need separately scoped checks. Kani uses its own toolchain, not automatically the repository's Rust pin; compatibility and practical proof cost are not established by this design.

## Open design decisions

- Movement parameters: world-unit scale, integer widths/ranges, footprint size, displacement per tick, tick rate, numeric resource ceilings, and exact accepted input/error behavior.
- Temporal rules: fuse and blast lifetime/phase boundaries and resource-exhaustion behavior.
- Multi-character interactions: conflicting moves, swaps, and contact rules.
- AI: decision ordering and random-consumption conventions.
- Objectives and items: additional completion conditions, power-up catalog/effects, item destruction/reveal behavior, multiple-item policies, and pickup conflicts.

The representation accommodates these choices without silently defining them. Gameplay dimensions, timing, and body size remain explicit parameters where tuning is needed; terminal display and unchecked integer types are not their source of truth. Delivery scope and verification-tool rollout belong in the [project plan](../project-plan.md).
