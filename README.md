# Exploderous

A Bomberman-clone in Rust with a focus on correctness through different testing techniques.

## Development

Install [rustup](https://rustup.rs/). Cargo commands automatically provision the Rust version, `rustfmt`, and `clippy` pinned in `rust-toolchain.toml`, both locally and in CI:

```sh
cargo run --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

The root executable is still a placeholder, with one temporary smoke test. The headless movement core lives in `crates/core` (package `exploderous-core`), with its movement checks in the `game.rs` test module. Root-level check commands cover both workspace members by default. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests.

## Current route

Start with a headless game core and basic TUI; expand gameplay and local competitive modes incrementally. Add frontend and networking crates under `crates/` when their delivery slices begin, keeping the core independent of them. Graphics, networking, and browser support will be evaluated as the project develops.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
