# Nash - Heads-Up NLHE Solver Framework

A heads-up No-Limit Hold'em solver using the CFR+ algorithm with card abstraction, action abstraction, and strategy serialization.

## Features

- **CFR+ Algorithm**: Counterfactual Regret Minimization with linear weighting
- **Card Abstraction**: Hand bucketing for reduced game tree complexity
- **Action Abstraction**: Configurable bet/raise sizes
- **Strategy Serialization**: Save/load strategies using bincode format
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
};
```

## Project Structure

- `main.rs` - Entry point with all game logic, CFR solver, and hand evaluation

## Notes

- CFR iterations are computationally expensive. Use `--release` for production builds.
- Strategies are saved to `strategy.bin` by default.
- Use chance sampling for faster convergence on large game trees.

## License

MIT
