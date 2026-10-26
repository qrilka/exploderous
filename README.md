# Exploderous

An original Bomberman-inspired game in Rust, with correctness as a first-class goal. It's an exercise to build an app using https://github.com/mattpocock/skills

## Development

### Development setup

Requirements:

- [rustup](https://rustup.rs/) to manage the Rust toolchain.
- Python 3 for repository automation tests.
- [just](https://just.systems/) 1.51.0, [cargo-machete](https://github.com/bnjbvr/cargo-machete) 0.9.2, and [ast-grep](https://ast-grep.github.io/) 0.51.0 for the complete quality check. Install the same versions pinned in CI:

  ```sh
  cargo install --locked just --version 1.51.0
  cargo install --locked cargo-machete --version 0.9.2
  cargo install --locked ast-grep --version 0.51.0
  ```

- Repository-local hooks enabled with `git config --local core.hooksPath .githooks`.

### Run and validate

Cargo commands automatically provision the Rust version, `rustfmt`, and `clippy` pinned in `rust-toolchain.toml`, both locally and in CI:

```sh
cargo run -p exploderous-tui --locked
just check
```

The root is a virtual workspace. The playable level for Normal mode is `crates/tui` (package `exploderous-tui`) and requires a terminal with Kitty keyboard-protocol support, at least 19 columns × 17 rows. Arrow keys move, Space places a Bomb, R generates a fresh run, and Q/Escape quits; `h` opens paused help. Defeat the three wandering `e` Buroru with Blasts, reveal the covered Exit with a Bomb, and reach its Cell alive to win. A revealed Exit shows `▧` while enemies survive and `⬚` when ready. Reveal `◈` Fire-up under another Block, wait for the Blast to expire, then collect it to increase subsequent Bombs' reach from one Cell to two; the status row shows current blast reach. Ticking Bombs keep their original reach, and later Blasts can destroy exposed Fire-up. The upgrade is optional for victory. Entering a live Blast Cell or sharing a centre Cell with a Buroru ends the run in Game Over; either outcome freezes play until restart or quit. Each run overwrites `app.log` with terminal events, per-tick movement/placement inputs and positions, and catch-up/error records; pasted contents are redacted. The headless simulation lives in `crates/core` (package `exploderous-core`); root-level check commands cover all workspace members.

`just check` runs unused-dependency detection, formatting, Clippy (warnings fail), hook regression tests, mutation-runner regression tests (no mutation scan), and Rust tests across the workspace. It stops at the first failure and also works from workspace subdirectories. CI runs the same command on pushes and pull requests. The extra quality-check tools are not needed to play the game.

Following [Dev Quality Gates](https://github.com/mokevnin/dev-quality-gates), this project keeps the official rustfmt/Clippy gates and adds cargo-machete to detect unused dependencies. Machete uses source-text matching, so macro-generated usage can produce false positives; investigate findings before adding a narrowly justified `package.metadata.cargo-machete.ignored` exception. No exceptions are currently needed.

### Mutation testing

Mutation scans are opt-in: run them locally or through the manual CI workflow. cargo-mutants is not required to play the game or run the standard Cargo checks. See [Manual core mutation testing](docs/mutation-testing.md) for prerequisites, commands, and result interpretation.

Ordinary CI runs lightweight checks of the mutation runner itself, but does not perform mutation scans.

### Pre-commit file-size check

CI enables the repository hooks and runs `python3 scripts/test_pre_commit.py` on every push and pull request. Run the same test when changing the hook. It creates a fresh fixture clone, enables the hook, and verifies accepted and rejected commits.

The hook rejects added, modified, renamed/copied, or type-changed hand-maintained files whose **staged content exceeds 5,000 tokens**, estimated as bytes / 4 (20,000 bytes). Unchanged files and unstaged edits are not checked. Oversized files must be split by responsibility; the error provides instructions for Rust modules, tests, and documentation.

`Cargo.lock` is exempt through `.gitattributes`. Mark other genuinely tool-generated output there with `path/to/generated/** linguist-generated=true`; do not exempt hand-maintained code. Exemptions are read from the staged attributes, not unstaged edits.

## Project status and direction

The headless core and TUI provide a generated level for Normal mode with a 13×13 playable Arena, interior Wall pillars, a protected upper-left opening, roughly 40% Blocks, and a separately drawn exterior boundary. It includes movement, Bomb placement with [temporary per-Character passage until full Footprint clearance](docs/design/game-state.md#bombs-and-escape-permissions), lethal Blasts that clear Blocks and kill enemies, three Buroru with [deterministic, momentum-based wandering](docs/design/game-state.md#buroru-movement-and-character-contact) at half player speed, an enemy-gated covered Exit and Fire-up, a collectible blast-reach upgrade, victory/defeat, and fresh generation on startup and every restart with enemies, covered objectives, abilities, hazards, and inputs reset. Wandering uses project tuning, not a claim of exact historical Buroru fidelity. Further gameplay and verification work is outlined in the [project plan](docs/project-plan.md). Feature scope and acceptance gates are tracked in GitHub issues, while lasting gameplay rules live in the design.

- [Project plan](docs/project-plan.md)
- [Game-state and simulation design](docs/design/game-state.md)
- [Gameplay glossary](GLOSSARY.md)
- [Headless core decision](docs/adr/0001-headless-deterministic-core.md)
- [Terrain and object representation decision](docs/adr/0002-grid-terrain-and-positioned-objects.md)
