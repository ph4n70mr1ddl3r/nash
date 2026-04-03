use nash::{CFRConfig, CFRSolver, GameConfig};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

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
        exploitability_interval: 50,
        convergence_threshold: 0.0,
    };

    let mut solver = CFRSolver::new(game_config, cfr_config).unwrap_or_else(|e| {
        eprintln!("Failed to create solver: {e}");
        std::process::exit(1);
    });
    solver.solve();
}
