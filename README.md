# Exploderous

An original Bomberman-inspired game in Rust, with correctness as a first-class goal.

## Development

Install [rustup](https://rustup.rs/). Cargo commands automatically provision the Rust version, `rustfmt`, and `clippy` pinned in `rust-toolchain.toml`, both locally and in CI:

```sh
cargo run -p exploderous-tui --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

The root is a virtual workspace. The Bomb survival playground is `crates/tui` (package `exploderous-tui`) and requires a terminal with Kitty keyboard-protocol support. Arrow keys move, Space places a Bomb, R restarts, and Q/Escape quits; `h` opens paused help. Entering a live Blast Cell with the player's centre ends the run in Game Over; restart or quit to leave the frozen outcome. Each run overwrites `app.log` with terminal events, per-tick movement/placement inputs and positions, and catch-up/error records; pasted contents are redacted. The headless simulation lives in `crates/core` (package `exploderous-core`); root-level check commands cover all workspace members. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests.

## Project status and direction

The headless core and TUI support movement, Bomb placement, lethal Blasts that clear Blocks, and frozen Game Over with restart. There is no Exit objective yet. The next broad milestone is a complete playable Normal level; see the [project plan](docs/project-plan.md). Feature scope and acceptance gates are tracked in GitHub issues, while lasting gameplay rules live in the design.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
- [Terrain and object representation decision](docs/adr/0002-grid-terrain-and-positioned-objects.md)
