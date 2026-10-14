# Exploderous

A Bomberman-clone in Rust with a focus on correctness through different testing techniques.

## Development

Install [rustup](https://rustup.rs/). Cargo commands automatically provision the Rust version, `rustfmt`, and `clippy` pinned in `rust-toolchain.toml`, both locally and in CI:

```sh
cargo run -p exploderous-tui --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

The root is a virtual workspace. The separate movement playground is `crates/tui` (package `exploderous-tui`) and requires a terminal with Kitty keyboard-protocol support. Each run overwrites `app.log` with all terminal events, per-tick movement inputs and positions, and catch-up/error records; pasted contents are redacted. The headless movement core lives in `crates/core` (package `exploderous-core`); root-level check commands cover all workspace members. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests.

## Movement comparison

Turn assistance is enabled by default. Compare it with strict movement in the same playground:

```sh
cargo run -p exploderous-tui --locked
cargo run -p exploderous-tui --locked -- --strict-movement
```

Both modes use the [two-Block playtest arena](docs/project-plan.md#following-slice-movement-tui-playtest) and retain the canonical Footprint and movement speed; `R` resets without changing the selected mode. The session-start entry in `app.log` records `turn_assistance`. See the [movement contract](docs/design/game-state.md#turn-assistance): assistance clears an eligible corner gradually, but does not guarantee a zero centre offset.

Manual acceptance for [issue #5](https://github.com/qrilka/exploderous/issues/5) requires comparing both modes in a compatible terminal: turns along both axes and in both directions, clear travel, blocked/unsafe turns, releases and direction changes. Watch the offset readout during correction and after entering the corridor; headless tests cannot establish whether the feel is understandable.

## Current route

Start with a headless game core and basic TUI; expand gameplay and local competitive modes incrementally. Add frontend and networking crates under `crates/` when their delivery slices begin, keeping the core independent of them. Graphics, networking, and browser support will be evaluated as the project develops.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
