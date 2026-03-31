use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "stark-bot", version, about = "Terminal AI agent client for Starflask")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List available agents
    Agents,
    /// Sync/provision agents from seed packs
    Provision {
        /// Path to seed pack JSON file
        #[arg(short, long)]
        file: Option<String>,
    },
    /// One-shot query to an agent
    Query {
        /// Agent capability name
        agent: String,
        /// Message to send
        message: String,
    },
    /// Guided first-time setup wizard
    Setup,
    /// Show or set config values
    Config {
        /// Config key to get/set
        key: Option<String>,
        /// Value to set (omit to get current value)
        value: Option<String>,
    },
}
