# Exploderous

A Bomberman-clone in Rust with a focus on correctness through different testing techniques.

## Development

Install stable Rust with `rustfmt` and `clippy` using [rustup](https://rustup.rs/):

```sh
rustup toolchain install stable --profile minimal --component rustfmt,clippy
rustup default stable
cargo run --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

The executable is currently a placeholder, with one temporary smoke test. CI runs formatting, Clippy (warnings fail), and tests on pushes and pull requests. There is no gameplay yet.

## Current route

Start with a headless game core and basic TUI; expand gameplay and local competitive modes incrementally. Graphics, networking, and browser support will be evaluated as the project develops.

- [Project plan](docs/project-plan.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
