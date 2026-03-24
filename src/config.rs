#[derive(Debug, Clone, Copy)]
pub struct GameConfig {
    pub initial_stacks: [u64; 2],
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_bet: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CFRConfig {
    pub num_iterations: usize,
    pub log_interval: usize,
    pub save_interval: usize,
    pub save_path: Option<String>,
    pub use_chance_sampling: bool,
}

impl Default for CFRConfig {
    fn default() -> Self {
        CFRConfig {
            num_iterations: 100,
            log_interval: 10,
            save_interval: 50,
            save_path: None,
            use_chance_sampling: true,
        }
    }
}
