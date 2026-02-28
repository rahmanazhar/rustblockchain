mod app;
mod config;

use app::Application;
use clap::{Parser, Subcommand};
use config::NodeConfig;
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser)]
#[command(
    name = "rustchain",
    version,
    about = "RustChain - A production-grade Proof of Stake blockchain"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to configuration file
    #[arg(short, long, default_value = "config/node.toml")]
    config: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Data directory
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the node
    Run {
        /// Enable block production (validator mode)
        #[arg(long)]
        validator: bool,

        /// Path to validator keyfile
        #[arg(long)]
        keyfile: Option<PathBuf>,

        /// Run in devnet mode with auto-generated config
        #[arg(long)]
        devnet: bool,
    },
    /// Initialize a new chain from genesis config
    Init {
        /// Path to genesis configuration file
        #[arg(long)]
        genesis: PathBuf,

        /// Output directory for data
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
    },
    /// Generate a new validator keypair
    Keygen {
        /// Output path for the keyfile
        #[arg(long, default_value = "./validator.key")]
        output: PathBuf,
    },
    /// Show node version and build info
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&cli.log_level));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    match cli.command {
        Commands::Run {
            validator,
            keyfile: _,
            devnet,
        } => {
            let (config, keypair) = if devnet {
                tracing::info!("Starting in devnet mode");
                let (mut config, keypair) = NodeConfig::devnet();
                config.consensus.enable_block_production = true; // devnet always produces blocks
                config.storage.path = cli.data_dir.join("chaindb");
                (config, Some(keypair))
            } else {
                let mut config = NodeConfig::load(&cli.config)?;
                config.consensus.enable_block_production = validator;
                config.storage.path = cli.data_dir.join("chaindb");
                (config, None)
            };

            let app = Application::new(config, keypair);
            app.run().await?;
        }
        Commands::Init { genesis, data_dir } => {
            tracing::info!("Initializing chain from {:?}", genesis);

            let genesis_config = rustchain_core::GenesisConfig::load(&genesis)?;
            genesis_config.validate()?;

            std::fs::create_dir_all(&data_dir)?;

            let storage_config = rustchain_storage::StorageConfig {
                path: data_dir.join("chaindb"),
                ..Default::default()
            };

            let storage =
                std::sync::Arc::new(rustchain_storage::ChainDatabase::open(&storage_config)?);

            let _vm = rustchain_vm::WasmEngine::new(&rustchain_vm::VmConfig::default())?;

            let state = rustchain_consensus::ChainState::from_genesis(
                &genesis_config,
                storage.clone(),
            )?;

            tracing::info!(
                "Chain initialized: {} (chain ID: {})",
                genesis_config.chain_name,
                genesis_config.chain_id
            );
            tracing::info!("Genesis block hash: {}", state.head().hash());
            tracing::info!(
                "Initial validators: {}",
                genesis_config.initial_validators.len()
            );
        }
        Commands::Keygen { output } => {
            let keypair = rustchain_crypto::KeyPair::generate();
            let password = "changeme"; // In production, prompt interactively

            rustchain_crypto::Keystore::save_to_file(&keypair, password, &output)?;

            println!("Validator key generated:");
            println!("  Address: {}", keypair.address());
            println!("  Public key: {}", keypair.public_key());
            println!("  Keyfile: {:?}", output);
            println!("  Password: {} (change in production!)", password);
        }
        Commands::Version => {
            println!("rustchain {}", env!("CARGO_PKG_VERSION"));
            println!("Rust edition: 2021");
        }
    }

    Ok(())
}
