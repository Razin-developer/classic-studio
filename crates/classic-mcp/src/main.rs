use classic_mcp::{ServerConfig, run_stdio};

fn main() {
    let config = match ServerConfig::from_environment() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("classic-mcp configuration error: {error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = run_stdio(config) {
        eprintln!("classic-mcp stopped: {error}");
        std::process::exit(1);
    }
}
