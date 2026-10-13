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
| Movement TUI playtest | Keyboard input, a fixed-cell movement map, status/help screens, and real-time pacing around the movement core | In a compatible terminal, a human can navigate, stop, turn, reset, inspect help, and quit; normal exit and handled failures restore the terminal |
| One-bomb mechanics | Placement, fuse, blasts, block destruction; extend the existing TUI to exercise them | Rule tests cover placement restrictions, timing, propagation, and lingering hazards |
| Complete first level | Exit objective, death, victory, explicit restart, integrated into the TUI | Scripted and terminal play can complete and restart the arena; tests cover death-before-victory and restoration on restart |

Each slice gets specific behavior and verification acceptance criteria before implementation. It is complete only when those criteria pass. Do not build empty advanced-testing harnesses into the skeleton.

Keep dependencies and project structure minimal. Introduce abstractions and optional build features only for concrete needs, not a speculative frontend or capability matrix.

### Next slice: movement and collision

Implement terrain, validated rules/scenario construction, one human-controlled character, whole-game tick movement, read-only observations, and complete cloning. Use character records/identities that support adding characters without a representation redesign.

Do not scaffold dormant enemy variants, bombs, blast/item collections, an exit field, AI/RNG state, networking, or generic adapters in this slice. Those parts of the full design are implemented with their corresponding gameplay features.

The [movement slice spec](https://github.com/qrilka/exploderous/issues/1) resolves the initial movement parameters and defines concrete validation/error acceptance criteria. [Turn assistance](https://github.com/qrilka/exploderous/issues/5) revises the initial no-alignment restriction; the lasting rule is owned by the [movement design](design/game-state.md#turn-assistance). Implement its worked movement cases, rejected-input state preservation, replay/cloning checks, and bounded comparison against the independent movement reference through the shared whole-game interface. Advanced-tool setup remains separate explicit verification work.

### Following slice: movement TUI playtest

After the movement slice, build a minimal TUI around the same validated construction, tick-input, and read-only observation interface. This is a movement playground, not yet a complete Normal level:

- The root is a virtual Cargo workspace; `crates/tui` owns the runnable app, depends on `exploderous-core`, and uses Ratatui with Crossterm.
- Use one fixed 9×9 classic arena with a Wall border and pillars at interior Cells whose zero-based row and column are both even. The player starts at the centre of zero-based Cell `(1,1)` (world position `(384,384)`). Blocks occupy `(3,1)`, `(5,1)`, `(5,3)`, `(1,5)`, `(3,5)`, `(7,5)`, `(3,7)`, and `(5,7)` (coordinates are column, row).
- Render one contiguous glyph per Cell, with no inter-cell padding. Draw the Character in the Cell containing its centre; sub-cell movement is intentionally not visible in the map. Do not show the former position/speed/footprint telemetry.
- Use the fixed glyphs below. In the live movement view, only Floor, Wall, Block, and the player Character are present; help also documents the reserved future glyphs.

  | Type | Glyph |
  | --- | --- |
  | Floor | blank space (U+0020) |
  | Wall | `█` |
  | Block | `▒` |
  | Player Character | `@` |
  | AI-controlled non-Enemy Character | `a` |
  | Enemy Character | `e` |
  | Bomb | `●` |
  | Blast centre | `┼` |
  | Blast arms, north/south | `│` |
  | Blast arms, east/west | `─` |
  | Exit | `⬚` |
  | Bomb-capacity Item | `▣` |
  | Item that increases Blast reach | `◈` |

  `e` identifies an Enemy regardless of controller; `a` identifies an AI-controlled Character that is not an Enemy. Require the selected glyphs to occupy one terminal column in the manual playtest; do not silently substitute glyphs. When Blasts are added, draw an arm in the destroyed Block's Cell and stop there; a Wall remains visible and stops the ray.
- Keep a one-line status area below the map: `Lives: 0 | Score: 0`. These are display-only placeholders for layout, not fields or behavior in the core Game; omit the former position/speed/footprint telemetry. The status line makes the gameplay minimum 19 columns × 10 rows (the 9×9 map plus one row).
- In gameplay, use arrow keys only for movement, `R` to reset by constructing a fresh Game, and `Q` or Escape to quit. Escape closes help instead when help is open. When multiple directions are held, the most recently pressed wins; releasing it falls back to the most recently pressed remaining direction. Clear held input and elapsed-time backlog on reset. Treat repeat events as neither new presses nor simulation ticks; a direction stays held until release.
- `?`, `h`, or `H` toggles a paused help screen; Escape closes it. Clear held directions when entering or leaving help. The help screen lists controls, the live glyphs, and the reserved future glyphs; Up/Down scrolls the list when it exceeds the available space, without increasing the gameplay minimum. While help is open, only scrolling and help-close/toggle keys work; `R` and `Q` are ignored. Process help/reset/quit commands only on key presses, not repeats.
- Advance the core at the movement spec's nominal 60 Hz, independent of rendering. Use a monotonic clock; process at most five overdue ticks in an update, then discard excess elapsed-time backlog. Drain available key events before stepping; every tick in a catch-up batch uses the resulting held-direction state. Paused time (help, focus loss, or undersized terminal) does not accumulate; reset the clock baseline when resuming idle.
- Center the unchanged arena when it fits. On focus loss, pause tick advancement and clear held input. If the terminal is smaller than 19×10, also show a resize message. Resume idle only when focus is present, the terminal fits, and help is closed. The help screen wraps and scrolls within that same minimum.
- Require [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) support, including event-type reporting and reporting all keys as escape codes. Require the capabilities, not a particular terminal application; any intermediary must preserve them. Negotiate and verify the Kitty keyboard enhancements at startup, and enable Crossterm focus-change reporting for the pause-on-focus-loss behavior. Unsupported terminal capabilities get an actionable error and terminal cleanup, not an OS-key-repeat fallback. Restore the previous keyboard mode and other terminal state on normal exit and handled failures.
- Keep bombs, objectives, arena editing, random generation, and gameplay menus out of this slice. Add later mechanics to the same TUI as their core slices arrive. Test adapter behavior headlessly and require a manual terminal playtest; manually verify keyboard negotiation, all four movement directions, held-key precedence/release, reset, status placeholders, help pause/scroll/resume and ignored `R`/`Q` while help is open, focus and resize recovery without catch-up movement on resume, actual glyph widths, and terminal restoration on normal exit, unsupported-terminal startup, and handled failure.

Before implementation, capture this scope in the TUI slice spec and turn the listed manual checks into an explicit pass checklist. Scoped Kani and mutation-testing work remains interleaved as described below.

## First level

One fixed Normal arena with a safe starting position and an exit hidden under a designated block. Destroy that block and reach its cell with the character's centre alive to win.

Initially omit enemies, power-ups, random arena generation, score, a lives counter, a level deadline, and pre-placed bombs. The player has bomb capacity one. Chain-reaction implementation and tests belong to a later multi-bomb slice.

Death or victory stops gameplay until explicit restart restores the initial arena and gameplay state.

### Simulation and action rules

- Real-time play uses fixed simulation ticks and integer sub-cell movement. The adapter controls pacing; rendering frequency and TUI display precision do not define game time or collision geometry.
- The initial character has a square footprint smaller than a cell. Cardinal movement checks the whole traveled path and stops at the first obstacle; exact edge contact is allowed. The [movement design](design/game-state.md#turn-assistance) defines bounded turn assistance and the strict comparison mode; unrestricted corner sliding remains excluded.
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
