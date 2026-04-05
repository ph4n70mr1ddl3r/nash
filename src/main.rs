//! Nash solver binary — runs CFR+ on a short-stack heads-up NLHE configuration.

use nash::{CFRConfig, CFRSolver, GameConfig};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Use short stacks so the binary finishes in a reasonable time.
    // Deep-stack solving requires many more iterations and is better
    // suited for custom configurations via the library API.
    let game_config = GameConfig {
        initial_stacks: [100, 100],
        small_blind: 1,
        big_blind: 2,
        min_bet: 2,
    };

    let cfr_config = CFRConfig {
        num_iterations: 100,
        exploitability_interval: 0,
        save_path: Some("strategy.bin".to_string()),
        ..CFRConfig::default()
    };

    let mut solver = CFRSolver::new(game_config, cfr_config).unwrap_or_else(|e| {
        eprintln!("Failed to create solver: {e}");
        std::process::exit(1);
    });
    solver.solve();
}
