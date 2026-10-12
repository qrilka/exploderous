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

The executable is currently a placeholder, with one temporary smoke test. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests. There is no gameplay yet.

## Current route

Start with a headless game core and basic TUI; expand gameplay and local competitive modes incrementally. Graphics, networking, and browser support will be evaluated as the project develops.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
