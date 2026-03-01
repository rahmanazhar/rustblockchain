mod client;
mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rustchain-wallet",
    version,
    about = "RustChain Wallet - Manage accounts and transactions"
)]
struct Cli {
    #[command(subcommand)]
    command: WalletCommand,

    /// Node API endpoint
    #[arg(long, default_value = "http://localhost:8080")]
    node: String,

    /// Keystore directory
    #[arg(long, default_value = "./keystore")]
    keystore: PathBuf,
}

#[derive(Subcommand)]
enum WalletCommand {
    /// Account management
    Account {
        #[command(subcommand)]
        cmd: AccountCommand,
    },
    /// Send tokens
    Transfer {
        /// Sender address (hex)
        #[arg(long)]
        from: String,
        /// Recipient address (hex)
        #[arg(long)]
        to: String,
        /// Amount to send
        #[arg(long)]
        amount: u128,
        /// Keystore password
        #[arg(long, default_value = "")]
        password: String,
    },
    /// Stake tokens to become a validator
    Stake {
        /// Validator address (hex)
        #[arg(long)]
        from: String,
        /// Amount to stake
        #[arg(long)]
        amount: u128,
        /// Keystore password
        #[arg(long, default_value = "")]
        password: String,
    },
    /// Unstake tokens from the validator set
    Unstake {
        /// Validator address (hex)
        #[arg(long)]
        from: String,
        /// Amount to unstake
        #[arg(long)]
        amount: u128,
        /// Keystore password
        #[arg(long, default_value = "")]
        password: String,
    },
    /// Deploy a smart contract
    Deploy {
        /// Deployer address (hex)
        #[arg(long)]
        from: String,
        /// Path to WASM bytecode
        #[arg(long)]
        wasm: PathBuf,
        /// Keystore password
        #[arg(long, default_value = "")]
        password: String,
        /// Gas limit for deployment
        #[arg(long, default_value = "1000000")]
        gas_limit: u64,
    },
    /// Call a smart contract function
    Call {
        /// Caller address (hex)
        #[arg(long)]
        from: String,
        /// Contract address (hex)
        #[arg(long)]
        contract: String,
        /// Function name
        #[arg(long)]
        function: String,
        /// Arguments (hex encoded)
        #[arg(long, default_value = "")]
        args: String,
        /// Value to send with the call
        #[arg(long, default_value = "0")]
        value: u128,
        /// Keystore password
        #[arg(long, default_value = "")]
        password: String,
        /// Gas limit
        #[arg(long, default_value = "500000")]
        gas_limit: u64,
    },
    /// Query blockchain state
    Query {
        #[command(subcommand)]
        cmd: QueryCommand,
    },
}

#[derive(Subcommand)]
enum AccountCommand {
    /// Create a new account
    Create {
        /// Number of mnemonic words (12 or 24)
        #[arg(long, default_value = "24")]
        words: usize,
        /// Password for keystore encryption
        #[arg(long, default_value = "")]
        password: String,
    },
    /// Import from mnemonic phrase
    ImportMnemonic {
        /// Mnemonic phrase
        #[arg(long)]
        phrase: String,
        /// Password
        #[arg(long, default_value = "")]
        password: String,
    },
    /// Import from private key
    ImportKey {
        /// Private key hex
        #[arg(long)]
        key: String,
        /// Password
        #[arg(long, default_value = "")]
        password: String,
    },
    /// List all accounts
    List,
}

#[derive(Subcommand)]
enum QueryCommand {
    /// Query account balance
    Balance {
        /// Account address (hex)
        #[arg(long)]
        address: String,
    },
    /// Query chain info
    ChainInfo,
    /// Query a block
    Block {
        /// Block number or hash
        #[arg(long)]
        id: String,
    },
    /// Query a transaction
    Transaction {
        /// Transaction hash
        #[arg(long)]
        hash: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        WalletCommand::Account { cmd } => match cmd {
            AccountCommand::Create { words, password } => {
                commands::account::create_account(words, &cli.keystore, &password)?;
            }
            AccountCommand::ImportMnemonic { phrase, password } => {
                commands::account::import_from_mnemonic(&phrase, &cli.keystore, &password)?;
            }
            AccountCommand::ImportKey { key, password } => {
                commands::account::import_from_private_key(&key, &cli.keystore, &password)?;
            }
            AccountCommand::List => {
                commands::account::list_accounts(&cli.keystore)?;
            }
        },
        WalletCommand::Transfer {
            from,
            to,
            amount,
            password,
        } => {
            commands::transfer::send_transfer(
                &cli.node,
                &cli.keystore,
                &from,
                &to,
                amount,
                &password,
            )
            .await?;
        }
        WalletCommand::Stake {
            from,
            amount,
            password,
        } => {
            commands::stake::send_stake(&cli.node, &cli.keystore, &from, amount, &password)
                .await?;
        }
        WalletCommand::Unstake {
            from,
            amount,
            password,
        } => {
            commands::stake::send_unstake(&cli.node, &cli.keystore, &from, amount, &password)
                .await?;
        }
        WalletCommand::Deploy {
            from,
            wasm,
            password,
            gas_limit,
        } => {
            commands::contract::deploy_contract(
                &cli.node,
                &cli.keystore,
                &from,
                &wasm,
                &password,
                gas_limit,
            )
            .await?;
        }
        WalletCommand::Call {
            from,
            contract,
            function,
            args,
            value,
            password,
            gas_limit,
        } => {
            commands::contract::call_contract(
                &cli.node,
                &cli.keystore,
                &from,
                &contract,
                &function,
                &args,
                value,
                &password,
                gas_limit,
            )
            .await?;
        }
        WalletCommand::Query { cmd } => match cmd {
            QueryCommand::Balance { address } => {
                commands::query::query_balance(&cli.node, &address).await?;
            }
            QueryCommand::ChainInfo => {
                commands::query::query_chain_info(&cli.node).await?;
            }
            QueryCommand::Block { id } => {
                commands::query::query_block(&cli.node, &id).await?;
            }
            QueryCommand::Transaction { hash } => {
                commands::query::query_transaction(&cli.node, &hash).await?;
            }
        },
    }

    Ok(())
}
