# Character movement and contact

Part of the [game-state design](game-state.md).

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

The initial bomber's footprint is a square whose sides remain horizontal and vertical. Its side length is smaller than a cell so one-cell-wide corridors allow some positioning error. The allowed deviation from a corridor's centreline is half the difference between the cell and footprint side lengths. For movement collision and Bomb escape permissions, rectangles overlap only when their intersections have positive length on both axes; edge or corner contact alone is safe. Blast damage instead uses the Character's centre-containing Cell; see [Blasts across Cells and ticks](bombs-and-blasts.md#blasts-across-cells-and-ticks).

Movement:

- Receives at most one cardinal direction per character per tick, with displacement determined by the game's rules. A command applies only to its tick; omission means no movement, not continuation of a remembered direction. Frontends represent held input with repeated tick commands. Initial movement parameters are fixed at construction; later dynamic effects require explicit gameplay rules.
- Checks the entire path of the footprint, not merely its centre or destination. Stop at the furthest legal position at the first obstacle; exact edge contact is allowed, penetration is not.
- Keeps the complete footprint inside arena bounds and outside terrain forbidden by that character's rules.
- Uses the explicit turn-assistance rule below when enabled, not unrestricted corner sliding or frontend alignment.

### Turn assistance

`GameRules` holds movement and bomb parameters together, selects assisted or strict movement before Scenario construction, and remains fixed for the Game's lifetime. Assistance is enabled by default; strict mode uses only ordinary cardinal movement. The canonical bomber Footprint remains 192 world units and its movement distance remains 16. At the 256-unit Cell scale, that Footprint allows centre offsets of ±32 in a one-Cell corridor.

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

Kinds are explicit compiled-in variants with kind-specific state, not arbitrary property bags, a plugin system, or a trait hierarchy. Control source is separate from kind: a bomber can be human-controlled or AI-controlled, while Normal mode can also have distinct enemy kinds. The current Scenario interface authors its Normal-mode player with `HumanBomberStart { id, position }` and its Buroru with `BuroruStart { id, cell }`; those slot-specific inputs determine kind and control without accepting invalid combinations. Read-only Character observations retain both concepts. Other modes may introduce their own starting roles. Overlap legality is decided by rules, not a universal one-character-per-cell storage invariant.

### Buroru movement and Character contact

Buroru are NPC enemies, not competing bombers. They do not place Bombs or collect Items. Their initial Footprint uses the canonical 192-unit side; initial movement is 8 world units per tick, half the player's 16. Speed remains a distinct tuning value.

Authored Buroru starts use Cell coordinates rather than world positions. Scenario construction resolves each Cell's centre and rejects out-of-bounds or obstructed placement, using the canonical Buroru Footprint independently of player tuning. Off-centre authored enemy starts are not representable. During play they retain exact between-centre positions without snapping. Direction legality at a centre means a full Footprint-safe route to the adjacent centre, not merely room for the next eight-unit step. If a newly placed non-overlapping Bomb interrupts travel between centres, the enemy stops at legal contact and waits. After removal it continues its heading to the next centre before choosing again; there is no off-centre direction selection or escape planning.

Their policy is persistent cardinal wandering with momentum, without player pursuit, Blast prediction, or deliberate hazard avoidance. There are no randomized run timers or clear-path reversals. At a Cell centre:

- With no heading yet, choose uniformly among legal cardinal directions. Initial choice is a single decision; do not immediately apply another junction choice in the same tick.
- If the remembered forward direction is legal and at least one perpendicular direction is legal, continue straight with 90% probability or turn with 10% probability, divided uniformly among legal perpendicular directions. With no legal side turn, continue straight without a random draw. Reversal is never chosen while forward is legal.
- If forward is blocked and reversal plus at least one side turn are legal, reverse with 75% probability or turn with 25% probability, divided uniformly among legal perpendicular directions. If only reversal is legal, reverse; if reversal is blocked, choose uniformly among legal side turns. If no direction is legal, wait without forgetting an existing heading.

These probabilities apply once per eligible centre decision, not per tick. Walls, Blocks, Bombs, and bounds determine legality through the full-route Footprint check above. A newly placed overlapping Bomb does not obstruct an enemy while escape permission applies; ordinary wandering continues without escape planning. Centre-based direction selection is distinct from player turn assistance: never snap, turn off-centre, or teleport to resolve an obstruction.

The momentum policy was selected after playtesting to balance readable paths with occasional exploration. Its percentages are project tuning, not a claim of authentic Bomberman ’93 behavior. The [historical NPC research](../research/first-level-npcs.md) records earlier evidence and its limits; it does not define this revised policy.

Buroru do not block or harm one another. Player–enemy contact instead causes Defeat when both centres occupy the same Cell, including contact during actual movement. Footprint overlap into a neighboring Cell alone is harmless. Compare centre occupancy at the same point in tick movement; visiting a Cell at different times is not simultaneous contact. The shared half-open Cell boundaries apply. Each Buroru dies from one Blast hit, using the same centre-containing Cell and actual-path hazard rules as the player.

Each live Buroru record owns its Character and optional heading together; removal cannot leave a separate policy entry behind. Read-only enemy observations iterate Characters without exposing policy storage. NPC decisions use explicit policy/random state and deterministic processing order, independent of external command enumeration or incidental storage order. A Scenario supplies the initial AI seed (zero by default); the Game owns the advanced generator state. Process living Buroru by ascending Character identity. Enumerate legal directions and side-turn candidates Up, Right, Down, Left. The random-choice helper requires a `NonZeroUsize` bound; callers establish nonempty candidate categories before selecting an index. An initial or forced-category selection draws a uniform index only when more than one candidate exists. A clear-forward junction draws one uniform value in 0..10 (zero selects a side turn); a blocked-forward choice with both categories available draws one in 0..4 (zero selects a side turn). If the side-turn category wins, draw its uniform index only when multiple side turns exist. Forced movement, non-junction straight travel, trapped waiting, and between-centre travel consume no randomness. Complete cloning captures headings, random state, and living enemies; there are no obsolete run timers. A removed enemy's identity is not reused.
