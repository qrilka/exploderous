# Headless deterministic gameplay core

Exploderous will keep gameplay rules in a headless Rust core advanced by fixed ticks and explicit inputs, independently of terminal output, rendering, wall-clock pacing, and networking. Any future randomness must have explicit reproducible state. This lets rule tests, deterministic simulations, scoped verification harnesses, and later frontends exercise the same behavior instead of parallel implementations.

We chose this over initially embedding gameplay in the TUI or making Bevy the simulation foundation. It requires an explicit input/time boundary from the beginning, but avoids coupling correctness checks and future frontends to an interactive runtime. The first adapter is a basic TUI; Bevy, browser support, and network synchronization remain undecided. Start with the minimum concrete core API, not a generic frontend framework.
