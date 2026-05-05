use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Top-level CLI struct for ddr-mcp.
#[derive(Parser)]
#[command(name = "ddr-mcp", about = "A read-only MCP server for Design Decision Records")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Index a file or directory of Design Decision Records.
    Index(IndexArgs),
    /// Start the MCP server.
    Serve(ServeArgs),
}

/// Arguments for the `index` subcommand.
#[derive(clap::Args)]
pub struct IndexArgs {
    /// Path to a file or directory to index (required).
    pub file: PathBuf,

    /// Path to config file (default: ./config.toml).
    #[arg(long, default_value = "./config.toml")]
    pub config: PathBuf,

    /// Wipe existing index and re-embed everything from scratch.
    #[arg(long)]
    pub rebuild: bool,
}

/// Arguments for the `serve` subcommand.
#[derive(clap::Args)]
pub struct ServeArgs {
    /// Path to config file (default: ./config.toml).
    #[arg(long, default_value = "./config.toml")]
    pub config: PathBuf,
}
