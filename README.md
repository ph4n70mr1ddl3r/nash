# Nash - Heads-Up NLHE Solver Framework

A heads-up No-Limit Hold'em solver using the CFR+ algorithm with action abstraction and strategy serialization.

## Features

- **CFR+ Algorithm**: Counterfactual Regret Minimization with linear weighting
- **Action Abstraction**: Configurable bet/raise sizes
- **Strategy Serialization**: Save/load strategies using postcard binary format
- **Parallel Processing**: Multi-threaded CFR iterations via rayon
- **Progress Tracking**: Iteration counts, memory usage, and exploitability estimates

## Usage

```bash
# Run the solver
cargo run --release

# Run tests
cargo test --release
```

## Configuration

```rust
use nash::{GameConfig, CFRConfig, CFRSolver};

let game_config = GameConfig {
    initial_stacks: [1000, 1000],
    small_blind: 1,
    big_blind: 2,
    min_bet: 2,
};

let cfr_config = CFRConfig {
    num_iterations: 100,
    log_interval: 10,
    save_interval: 50,
    save_path: Some("strategy.bin".to_string()),
    use_chance_sampling: true,
    samples_per_iteration: 0,
    exploitability_interval: 0,
    convergence_threshold: 0.0,
};

let mut solver = CFRSolver::new(game_config, cfr_config)?;
solver.solve();
```

## Project Structure

- `src/lib.rs` - Library exports
- `src/card.rs` - Card, Deck, CardSet types
- `src/config.rs` - GameConfig, CFRConfig
- `src/game.rs` - GameState, Action, Player, Street, InfoSet
- `src/hand.rs` - Hand evaluation
- `src/strategy.rs` - Strategy storage and serialization
- `src/solver.rs` - CFRSolver implementation
- `src/main.rs` - Entry point

## Notes

- CFR iterations are computationally expensive. Use `--release` for production builds.
- Strategies are saved to `strategy.bin` by default.
- Use chance sampling for faster convergence on large game trees.

## License

MIT
