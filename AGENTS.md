# AI Agent Instructions

## Build Commands

```bash
cargo build --release
```

## Test Commands

```bash
cargo test --release
```

## Lint Commands

```bash
cargo clippy --release
```

## Project Overview

Nash is a heads-up No-Limit Hold'em poker solver using the CFR+ (Counterfactual Regret Minimization with linear weighting) algorithm.

## Architecture

- **card.rs**: Card representation, deck management, and card sets
- **config.rs**: Game and solver configuration structs
- **game.rs**: Game state machine, actions, player positions, streets
- **hand.rs**: Poker hand evaluation (high card through straight flush)
- **strategy.rs**: CFR strategy storage with concurrent DashMap access
- **solver.rs**: CFR+ algorithm implementation
- **main.rs**: Entry point and tests

## Key Patterns

- Use `#[must_use]` for constructors and conversion methods
- Use `#[inline]` for performance-critical functions
- Use `DashMap` for thread-safe strategy storage
- Use `rayon` for parallel iteration in CFR
- Serialize strategies with `postcard`

## Dependencies

- `postcard`: Binary serialization (no_std compatible)
- `dashmap`: Concurrent hash map
- `rand`: Random number generation
- `rayon`: Parallel processing
- `serde`: Serialization framework
- `thiserror`: Error handling
- `tracing`: Logging
