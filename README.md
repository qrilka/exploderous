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

The root is a virtual workspace. The separate movement playground is `crates/tui` (package `exploderous-tui`) and requires a terminal with Kitty keyboard-protocol support. The headless movement core lives in `crates/core` (package `exploderous-core`); root-level check commands cover all workspace members. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests.

## Current route

Start with a headless game core and basic TUI; expand gameplay and local competitive modes incrementally. Add frontend and networking crates under `crates/` when their delivery slices begin, keeping the core independent of them. Graphics, networking, and browser support will be evaluated as the project develops.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
