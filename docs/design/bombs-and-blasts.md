# Bombs, Blasts, Items, and Exit

Part of the [game-state design](game-state.md).

## Bombs and escape permissions

```rust
struct Bomb {
    id: BombId,
    cell: Cell,
    owner: CharacterId,
    detonate_at: Tick,
    blast_reach: u32, // Snapshot of the owner's reach at placement.
}

struct BombEscapePermission {
    character: CharacterId,
    bomb: BombId,
}
```

The records above illustrate the fuller identity-based model. The generated level for Normal mode still has only one bomb-placing Character and capacity one: multiple NPCs do not require multiple live Bombs or a generic identity allocator for Bombs. One optional Bomb retains the identities of Characters with escape permission. Removing it removes every associated permission; permissions must not transfer to a later Bomb.

Bombs are cell-anchored. Placement selects the cell containing the character's centre and does not move or snap the character. A live bomb occupies its cell for Footprint-based movement collision. Ownership lets capacity be enforced per character; capacity counts undetonated bombs, not lingering blasts. A Bomb snapshots the placing Character's blast reach; collecting Fire-up afterward cannot alter it.

Every Character whose Footprint overlaps the Bomb Cell at placement receives escape permission for that Bomb. This includes the placing player and any overlapping enemy whose centre may still be in a neighboring Cell. While permission applies, that Bomb is ignored for movement collision: the Character may approach or cross its centre, reverse direction, or travel perpendicular to it. Ordinary Footprint collision with terrain and arena bounds still applies, including both actual player turn-assistance segments. This temporary passage avoids immobilizing an off-centre Character whose only open retreat initially approaches the Bomb centre; it does not guarantee survival of the Blast.

Permission ends once the whole Footprint clears the Bomb Cell, or the Bomb ceases to exist. Clearing it during movement ends permission for the remaining path, not just the next tick. Other Bombs still block movement. Permission is authoritative gameplay state included in cloning/replay, not inferred solely from the Character's current centre. This replaces the former away-only restriction, which could trap an off-centre player in an otherwise open dead end. Full clearance, not centre-Cell departure or elapsed time, is the permission boundary; waiting while overlapping does not revoke it.

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

Damage uses the Cell containing the Character's centre, independently of its Footprint. Body overlap with a Blast is safe while the centre remains outside covered Cells. Cell membership uses the same lower-inclusive, upper-exclusive boundaries as Arena coordinate conversion; a centre exactly on a boundary belongs to the Cell on its right/bottom. Check the entire actually traveled centre path against Blasts already live during movement, including both actual turn-assistance segments and their endpoints; crossing danger cannot be undone by a safe endpoint. A stationary centre in a live Blast Cell is also lethal. A Blast created after movement checks only the final centre-containing Cell, consistent with actions preceding explosions.

A blast can trigger another bomb in the same tick. All propagation in that explosion phase uses the terrain present at its start: destroying a block does not open a route for a second same-phase ray. Bomb processing order must not change blast reach. Even with capacity one, an earlier lingering blast can trigger a newly placed bomb; capacity counts undetonated bombs rather than live effects.

### Tick phases and deadline boundaries

The tick label T denotes the resulting tick of a step advancing from T - 1. During active play, expire blasts whose deadline is T before movement, then resolve movement, exposed Item collection, placement, explosions, harm, and victory in that order. Collection after movement and before placement means entering Fire-up's Cell and placing a Bomb in the same tick uses the upgraded reach. Preserve path-contact evidence for harm from Blasts live during movement and for simultaneous player–enemy centre contact; a safe endpoint does not undo crossing a hazard.

A blast created on tick D with positive lifetime L has an absolute expiry deadline D + L. It is dangerous on creation and through tick D + L - 1, then expires before movement on D + L. For example, a bomb placed on tick 10 with a 120-tick fuse detonates on tick 130; with a 30-tick blast lifetime, its blast is dangerous through tick 159 and gone before movement on tick 160. Initial tuning values are owned by the [first playable level spec](https://github.com/qrilka/exploderous/issues/9).

Absolute labels and deadlines use `Tick`; positive fuse/lifetime durations use `TickSpan`, backed by `NonZeroU64`. Rule tuning parses raw integers into durations; observations retain the distinction instead of returning interchangeable integers. `Tick::checked_after(TickSpan)` is the only exposed time arithmetic and returns no deadline on overflow. Use checked addition for tick advancement and any required bomb/blast deadline. An unrepresentable deadline rejects the entire step with the existing time-exhaustion error and preserves complete state, including terrain, position, permissions, and effects. Do not wrap, saturate, or silently shorten timers. A placement blocked by ordinary capacity/occupancy rules remains a valid gameplay action, not an arithmetic failure.

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

Items are separate cell-anchored records. The specified first Item kind is Fire-up; further kinds, including capacity upgrades, remain outside this level's contract.

An Item under a Block is hidden and cannot be collected. Destroying the Block reveals the Item rather than deleting its record. The bomber collects an exposed Item when its centre enters that Cell; consumption removes the record and applies its specified effect. NPCs cannot collect it. Buroru touching or passing through it do not consume it.

Fire-up increases the bomber's current blast reach by one Cell beyond the origin in each cardinal direction. The generated level starts with reach one and contains one Fire-up, increasing reach to two. Base GameRules remain immutable; current per-Character abilities are mutable gameplay state, reset for a fresh run. Bombs retain their placement-time reach snapshot.

The Blast that reveals Fire-up cannot destroy it, including during that same effect's remaining lifetime. A distinct later Blast can destroy the exposed Item, even while the revealing effect still lingers. This protection must distinguish effects, not simply grant immunity whenever any Blast occupies the Cell. A revealed Item is not a safe Cell while the revealing Blast remains live. A covered Exit survives reveal and later Blasts.

Game state stores a single optional Exit directly as `exit: Option<Cell>`. Scenarios without an Exit have no Exit objective; the generated level for Normal mode requires one.

Exit visibility and eligibility are different. Terrain determines whether it is covered. The objective in Normal mode requires the living player's centre on its uncovered Cell and zero surviving enemies. Fire-up collection is not required. A locked Exit remains traversable Floor after reveal; standing there when the last enemy dies can complete the level without re-entry. Harm precedes victory. Eligibility is derived from enemy state, not a redundant mutable unlock flag; the TUI distinguishes visible locked and ready states.

Example coexistence:

| Object/location | State |
| --- | --- |
| Terrain at `(3, 2)` | Block |
| Exit | `(3, 2)` |
| Item at another covered cell | Its own kind record |
| Character on floor | Its precise footprint may span cells |
| Bomb beneath that character | Separate live bomb and escape permission |
| Two blasts sharing a floor cell | Separate coverage/lifetime records |

Additional power-up kinds, multiple-Item sharing policies, and simultaneous collection by multiple eligible bombers remain open. They are not silently chosen by the container layout; the generated level has one eligible collector and one Fire-up.

