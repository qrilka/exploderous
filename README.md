# Exploderous

An original Bomberman-inspired game in Rust, with correctness as a first-class goal. It's an exercise to build an app using https://github.com/mattpocock/skills

## Development

Install [rustup](https://rustup.rs/). Cargo commands automatically provision the Rust version, `rustfmt`, and `clippy` pinned in `rust-toolchain.toml`, both locally and in CI:

```sh
cargo run -p exploderous-tui --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

The root is a virtual workspace. The playable level for Normal mode is `crates/tui` (package `exploderous-tui`) and requires a terminal with Kitty keyboard-protocol support, at least 19 columns × 17 rows. Arrow keys move, Space places a Bomb, R generates a fresh run, and Q/Escape quits; `h` opens paused help. Defeat the three wandering `e` Buroru with Blasts, reveal the covered Exit with a Bomb, and reach its Cell alive to win. A revealed Exit shows `▧` while enemies survive and `⬚` when ready. Reveal `◈` Fire-up under another Block, wait for the Blast to expire, then collect it to increase subsequent Bombs' reach from one Cell to two; the status row shows current blast reach. Ticking Bombs keep their original reach, and later Blasts can destroy exposed Fire-up. The upgrade is optional for victory. Entering a live Blast Cell or sharing a centre Cell with a Buroru ends the run in Game Over; either outcome freezes play until restart or quit. Each run overwrites `app.log` with terminal events, per-tick movement/placement inputs and positions, and catch-up/error records; pasted contents are redacted. The headless simulation lives in `crates/core` (package `exploderous-core`); root-level check commands cover all workspace members. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests.

## Project status and direction

The headless core and TUI provide a generated level for Normal mode with a 13×13 playable Arena, interior Wall pillars, a protected upper-left opening, roughly 40% Blocks, and a separately drawn exterior boundary. It includes movement, Bomb placement with [temporary per-Character passage until full Footprint clearance](docs/design/game-state.md#bombs-and-escape-permissions), lethal Blasts that clear Blocks and kill enemies, three Buroru with [deterministic, momentum-based wandering](docs/design/game-state.md#buroru-movement-and-character-contact) at half player speed, an enemy-gated covered Exit and Fire-up, a collectible blast-reach upgrade, victory/defeat, and fresh generation on startup and every restart with enemies, covered objectives, abilities, hazards, and inputs reset. Wandering uses project tuning, not a claim of exact historical Buroru fidelity. Further gameplay and verification work is outlined in the [project plan](docs/project-plan.md). Feature scope and acceptance gates are tracked in GitHub issues, while lasting gameplay rules live in the design.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
- [Terrain and object representation decision](docs/adr/0002-grid-terrain-and-positioned-objects.md)
