//! Nash solver binary — runs CFR+ on a default heads-up NLHE configuration.

use nash::{CFRConfig, CFRSolver, GameConfig};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let game_config = GameConfig::default();

    let cfr_config = CFRConfig {
        num_iterations: 100,
        exploitability_interval: 50,
        save_path: Some("strategy.bin".to_string()),
        ..CFRConfig::default()
    };

    let mut solver = CFRSolver::new(game_config, cfr_config).unwrap_or_else(|e| {
        eprintln!("Failed to create solver: {e}");
        std::process::exit(1);
    });
    solver.solve();
}
