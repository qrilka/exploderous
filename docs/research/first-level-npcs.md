# Research: first Normal level NPCs

## Summary

**Yes: actual Bomberman enemy code evidences familiar approaches—persistent cardinal random movement, local alignment-based pursuit, and small stateful neighbor-selection heuristics.** The strongest inspected evidence is a community disassembly of **US NES Bomberman**, not PC Engine Buroru. It supports these algorithm families, but does not establish that PC Engine Bomberman, Bomberman ’93, NES II, or Saturn reuse them. The initial design interview selected timer-driven wandering without pursuit, informed by this evidence rather than claiming exact PC Engine fidelity. Later playtesting selected the momentum-based policy now owned by the game-state design; the original-code findings below are historical evidence, not that policy’s probability specification.

This note records research evidence and its limits, not normative gameplay requirements. The approved arena, generation, Buroru, Fire-up, bomb-escape, and objective rules belong to the [game-state design](../design/game-state.md). In particular, the selected boundary is separate presentation outside 13×13 stored gameplay cells, not a 15×15 terrain grid.

Read before preparing this note: `CODING_STANDARDS.md` and the game-state design.

## Findings: inspected original-game disassembly

### Source identity and citation method

[emu-russia/bomberman-nes](https://github.com/emu-russia/bomberman-nes) identifies its target as **NES-BM-USA**, PRG CRC32 `A913A222`, CHR CRC32 `1DB14E97`. This is reverse-engineered original-game code, **not Hudson-published source**, and not a fan recreation. I inspected the relevant assembly and constants directly; I did not build it or compare output against a ROM. Confidence below concerns the inspected transcription and its control flow, not independent binary verification.

Citations use exact **file + routine/ROM-address labels**, rather than invented line numbers. Some dispatch targets are raw addresses with unnamed code immediately following nearby labels. [BMAN.NAS](https://github.com/emu-russia/bomberman-nes/blob/master/BMAN.NAS) is the code file; [raw source](https://raw.githubusercontent.com/emu-russia/bomberman-nes/master/BMAN.NAS) was directly readable. [`CONSTS.NAS`](https://raw.githubusercontent.com/emu-russia/bomberman-nes/master/CONSTS.NAS) maps enemy IDs and directions. Branch URLs are mutable; commit API lookup was rate-limited.

1. **Claim: US NES Bomberman uses persistent cardinal movement with a random run-duration field and obstacle reactions.** **Sources:** `BMAN.NAS`, `THINK_PROC`, shared `loc_D33F`–`loc_D365`, `STEP_MONSTER`, `ENEMY_COLLISION`; `CONSTS.NAS`, map/direction constants. **Support:** direct code evidence; “persistent random walk” is researcher terminology. **Confidence:** high.
   - When `byte_49` is nonzero, the shared routine decrements it and calls `STEP_MONSTER` with remembered `M_FACE`. At zero, `loc_D365` calls `RAND`, sets a new cardinal facing, and sets the run count to `0x20 + ((random & 0x18) << 2)`: **32, 64, 96, or 128 eligible AI updates**, not necessarily frames. Enemy-specific frame gates determine update cadence.
   - Collision results below 3 reset the run count; results 3 or above reverse facing through `byte_D412`, set the run count to 96, and reset another behavior field. Constants identify concrete=1, brick=2, bomb=3. **Bomb collision therefore reverses this shared mover**, rather than making it plan an escape.
   - New headings are selected without first filtering all legal neighbors. Blocked attempts can occur. The inspected shared branch is **not** “uniformly choose a legal exit at every junction,” nor strictly “turn only on collision”: the run timer also triggers selection.
   - This is shared movement logic with enemy-specific entry behavior, not proof that every enemy is a pure random walker. In particular, even the first-type entry can call the alignment routines during a counter window; do not summarize this code as “Ballom never reacts to the player.”

2. **Claim: pursuit in the inspected NES routines is a local row/column alignment heuristic, not demonstrated line-of-sight or shortest-path pursuit.** **Sources:** `BMAN.NAS`, `TURN_HORIZONTALLY`, `TURN_VERTICALLY`; entry points `$D2C8`, `$D2E7`, `$D305`, `$D320` and `loc_D337`. **Support:** direct evidence plus classification. **Confidence:** high.
   - `TURN_HORIZONTALLY` requires enemy/player tile Y equality and chooses left/right from their X comparison. `TURN_VERTICALLY` requires tile X equality and chooses up/down from Y. They skip targeting while the player is dying.
   - These two complete routines do **not** trace intervening terrain, compare path lengths, or search a graph. Thus “same-row/column steering” is supported; “unobstructed line of sight” is not. Actual motion is subsequently constrained by collision.
   - Different enemy entries call these helpers under different counter/frame conditions: e.g. type 2 at `$D2E7` calls vertical steering, type 3 at `$D2C8` horizontal steering. Type names come from the constants (Oneal and Dahl); descriptive comments use alternative names.
   - **Scope limit:** this establishes what these routines do, not absence of pathfinding throughout the franchise.

3. **Claim: NES Pass/Pontan use stateful local neighbor checks rather than the same random-run branch.** **Sources:** `BMAN.NAS`, dispatch entries `$D3A0` / `$D38A`, shared `loc_D3AB`–`loc_D406`, `sub_D454`, `byte_D412`; `CONSTS.NAS`, IDs 7/8. **Support:** direct evidence; “small state machine/local turning heuristic” is interpretation. **Confidence:** high for operations, medium for the behavioral shorthand.
   - They share a branch with a behavior countdown, remembered direction, an alternating bit (`byte_4B`), and a run counter. They invoke alignment steering when the countdown is zero.
   - `sub_D454` returns a four-neighbor traversability mask **only when both sub-cell offsets equal the cell centre**. Direction/bit-indexed tables select a candidate turn; a legal candidate changes facing and toggles the bit. The counter can also toggle that bit. Collision either rotates direction or reverses it and starts a countdown.
   - This is concrete evidence for **centre-gated local decisions plus a few state variables**. It is not evidence for choosing uniformly among legal exits, a formal maze-search algorithm, A*, or a globally optimal route. I have not assigned a named wall-following algorithm: the exact table policy warrants deeper tracing before doing so.

4. **Claim: NES movement includes local alignment correction and kind-specific traversal, separate from direction choice.** **Sources:** `BMAN.NAS`, `STEP_ENEMY_RIGHT/LEFT/UP/DOWN`, `ADJUST_ENEMY_HPOS/VPOS`, `ENEMY_COLLISION` / `BRICK_WALL`. **Support:** direct evidence. **Confidence:** high.
   - Steps adjust perpendicular offsets toward the corridor centre when crossing toward another tile. Doria/Ovape/Pontan have brick-pass checks. Consequently, copying only a direction chooser would not reproduce movement.
   - `THINK` iterates enemy slots in descending order; `ENEMY_SAVE/LOAD` retain per-enemy direction/counters. `RAND` mutates shared seed bytes. **Researcher inference:** explicit ordering, counters, and PRNG state are sufficient ingredients for replayable decisions, but modern deterministic replay/restart semantics should follow this repository’s contract, not be claimed as an original-game API.

## Other releases: evidence boundaries

| Release | What is supported | Confidence / limitation |
| --- | --- | --- |
| **Bomberman (PC Engine/TG-16), Buroru/Maron** | Indexed [user wiki](https://bomberman.fandom.com/wiki/Buroru) describes wandering and random turning/reversal at walls/bombs; indexed [PC Engine stage guide](https://randomhoohaas.flyingomelette.com/bomb/pce-1/stage.html) compares Maron with Ballom | **Low for algorithm.** No accessible original routine found. Exact junction, probability, timer and targeting rules unverified. NES code cannot fill this gap. |
| **Bomberman II (NES), Maron** | Indexed [Buroru wiki](https://bomberman.fandom.com/wiki/Buroru) describes travel/pause/line-of-sight charge; indexed [area guide](https://randomhoohaas.flyingomelette.com/bomb/nes-2/area.html) describes periodic straight charging | **Low for algorithm.** Behavioral reports suggest a wander/pause/charge state machine, but no inspected code proves triggers or targeting. |
| **Bomberman ’93 (PC Engine)** | No enemy AI routine established in this pass | **Unresolved.** First-party [manual](https://dds.konami.com/games/manual/pcemini/en_Bomber93.pdf) was an unreadable scan in the prior pass; battle CPU behavior is not Normal enemy AI. |
| **Saturn / other releases** | Prior search suggested nearby-player pursuit for Buroru | **Unverified; not used.** No inspected technical routine. No franchise-wide algorithm conclusion. |

## Considered implementation approaches

These were the interview alternatives, not claims of PC Engine fidelity. Timer-driven wandering was subsequently approved; the owning design records its particular rules:

- **Persistent heading + timer/obstacle reselection:** evidenced as a family in NES code. Requires heading, decision countdown and explicit random state. Decide whether to draw any cardinal direction or only legal directions, whether blocked turns retry immediately, and bomb reversal versus ordinary blockage.
- **Centre/junction legal-neighbor wandering:** small, explainable alternative; keep straight between centres, choose at junctions, reverse at dead ends. **Not proven to be PC Engine Buroru’s policy**, and not the inspected NES random branch. Decide reversal eligibility and straight bias.
- **Local pursuit overlay:** same-row/column steering is evidenced in NES; actual unobstructed sight, distance limits, or a pause/charge state would be additional choices. Full pathfinding is a separate option with no supporting requirement established here.

Bomb-as-obstacle is not fuse-aware danger avoidance. Pocket allowance makes dead-end/trapped behavior important. Regenerate-on-restart needs generation random state/seed progression distinct from cloning an active run. The subsequent interview settled these interactions in the owning design; the alternatives above are retained to explain the evidence boundary, not as open implementation choices.

## Contradictions and corrections

- Corrected the previous note’s **15×15 terrain assumption** and stale fixed-versus-generated/restart options using the user’s settled decisions above.
- Removed fan-clone code as evidence about original Bomberman algorithms. Its Java random picker cannot establish Hudson’s implementation.
- A common indexed description calls original NES Ballom non-pursuing, but the inspected first-type entry reaches alignment helpers during a counter window. Prefer the concrete routine; the observed gameplay significance of that window remains unmeasured.
- NES II indexed descriptions differ in targeting detail (line-of-sight charge versus straight periodic charge). This may be omission rather than contradiction; do not resolve it by importing NES 1 steering.

## Missing evidence and validation

- No PC Engine Buroru, ’93, or NES II disassembly was located in the bounded targeted searches. This is a research gap, **not proof none exists**.
- No game execution, binary equivalence test, original footage observation, or timing measurement was performed. Exact assembly labels/addresses are supplied; line numbers and a pinned commit were not recovered.
- `source_check` for the NES steering claim returned **unclear** (automated semantic assessment unavailable). I manually inspected the original raw assembly and complete relevant steering, shared movement, neighbor-mask and collision routines instead.
- Fandom previously returned 403; the independent guide again failed DNS resolution. Their indexed descriptions remain discovery leads, not verified technical evidence. No global settings changed.

## Sources

- **Kept:** [NES disassembly README](https://github.com/emu-russia/bomberman-nes) — release/checksum provenance, with explicit community-disassembly caveat.
- **Kept:** [`BMAN.NAS`](https://raw.githubusercontent.com/emu-russia/bomberman-nes/master/BMAN.NAS), [`CONSTS.NAS`](https://raw.githubusercontent.com/emu-russia/bomberman-nes/master/CONSTS.NAS) — directly inspected original-game reverse engineering; strongest evidence for algorithm families.
- **Kept, provisional:** linked Buroru wiki and version-specific guides — reference identity and behavior leads only.
- **Deprioritized:** fan clone algorithms, general AI tutorials, reviews of battle-mode CPUs — do not answer how the original Normal enemies work.
- **Deprioritized:** PC Engine disassembler tooling and NES II level-hacking references — useful potential tools, but not enemy algorithm evidence.

## Conclusion

The initial approved policy used **timer-driven persistent wandering**, without the pursuit overlay. It was not established as authentic PC Engine Buroru. Playtesting subsequently replaced it with momentum-based wandering; follow the current game-state design rather than the historical policy choice here. No further fidelity research is required by the selected contract. If exact reproduction becomes a future requirement, the highest-value follow-up is a targeted PC Engine execution trace of one Buroru at an unobstructed junction, a dead end, and a bomb—not broader fan-code reading.
