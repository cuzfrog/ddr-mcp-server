use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Top-level CLI struct for docent.
#[derive(Parser)]
#[command(
    name = "docent",
    about = "MCP server for Document & Code History indexing and querying."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Index files from a directory.
    IndexFile(IndexFileArgs),
    /// Index git history from a repository.
    IndexGit(IndexGitArgs),
    /// Start the MCP server.
    Serve(ServeArgs),
    /// List all supported embedding models.
    ListModels,
}

/// Arguments for the `index-file` subcommand.
#[derive(clap::Args)]
pub struct IndexFileArgs {
    /// Path to a file or directory to index (required).
    pub file: PathBuf,

    /// Path to config file (default: ./config.toml).
    #[arg(long, default_value = "./config.toml")]
    pub config: PathBuf,

    /// Wipe existing index and re-embed everything from scratch.
    #[arg(long)]
    pub rebuild: bool,

    /// Show individual file paths as they are indexed.
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for the `index-git` subcommand.
#[derive(clap::Args)]
pub struct IndexGitArgs {
    /// Path to a git repository (must contain .git).
    pub file: PathBuf,

    /// Path to config file (default: ./config.toml).
    #[arg(long, default_value = "./config.toml")]
    pub config: PathBuf,

    /// Re-index full git history from scratch.
    #[arg(long)]
    pub rebuild: bool,

    /// Show individual commits as they are indexed.
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for the `serve` subcommand.
#[derive(clap::Args)]
pub struct ServeArgs {
    /// Path to config file (default: ./config.toml).
    #[arg(long, default_value = "./config.toml")]
    pub config: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_index_file_minimal_positional() {
        let cli = Cli::try_parse_from(["docent", "index-file", "./ddrs"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexFile(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("./ddrs"));
                assert_eq!(args.config, std::path::PathBuf::from("./config.toml"));
                assert!(!args.rebuild);
            }
            _ => panic!("expected IndexFile command"),
        }
    }

    #[test]
    fn test_index_file_with_config_flag() {
        let cli =
            Cli::try_parse_from(["docent", "index-file", "./ddrs", "--config", "/etc/docent.toml"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexFile(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("./ddrs"));
                assert_eq!(args.config, std::path::PathBuf::from("/etc/docent.toml"));
            }
            _ => panic!("expected IndexFile command"),
        }
    }

    #[test]
    fn test_index_file_with_rebuild_flag() {
        let cli = Cli::try_parse_from(["docent", "index-file", "./ddrs", "--rebuild"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexFile(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("./ddrs"));
                assert!(args.rebuild);
            }
            _ => panic!("expected IndexFile command"),
        }
    }

    #[test]
    fn test_index_file_all_flags() {
        let cli = Cli::try_parse_from([
            "docent",
            "index-file",
            "./ddrs",
            "--config",
            "custom.toml",
            "--rebuild",
        ]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexFile(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("./ddrs"));
                assert_eq!(args.config, std::path::PathBuf::from("custom.toml"));
                assert!(args.rebuild);
            }
            _ => panic!("expected IndexFile command"),
        }
    }

    #[test]
    fn test_index_file_verbose_flag() {
        let cli = Cli::try_parse_from(["docent", "index-file", "./ddrs", "--verbose"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexFile(args) => {
                assert!(args.verbose);
            }
            _ => panic!("expected IndexFile command"),
        }
    }

    #[test]
    fn test_index_file_missing_file_fails() {
        let cli = Cli::try_parse_from(["docent", "index-file"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_index_git_minimal() {
        let cli = Cli::try_parse_from(["docent", "index-git", "./my-repo"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexGit(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("./my-repo"));
                assert_eq!(args.config, std::path::PathBuf::from("./config.toml"));
                assert!(!args.rebuild);
                assert!(!args.verbose);
            }
            _ => panic!("expected IndexGit command"),
        }
    }

    #[test]
    fn test_index_git_with_rebuild() {
        let cli = Cli::try_parse_from(["docent", "index-git", "./my-repo", "--rebuild"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::IndexGit(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("./my-repo"));
                assert!(args.rebuild);
            }
            _ => panic!("expected IndexGit command"),
        }
    }

    #[test]
    fn test_index_git_requires_path() {
        let cli = Cli::try_parse_from(["docent", "index-git"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_serve_default_config() {
        let cli = Cli::try_parse_from(["docent", "serve"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::Serve(args) => {
                assert_eq!(args.config, std::path::PathBuf::from("./config.toml"));
            }
            _ => panic!("expected Serve command"),
        }
    }

    #[test]
    fn test_serve_custom_config() {
        let cli = Cli::try_parse_from(["docent", "serve", "--config", "prod.toml"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::Serve(args) => {
                assert_eq!(args.config, std::path::PathBuf::from("prod.toml"));
            }
            _ => panic!("expected Serve command"),
        }
    }

    #[test]
    fn test_unknown_subcommand_fails() {
        let cli = Cli::try_parse_from(["docent", "unknown"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_list_models() {
        let cli = Cli::try_parse_from(["docent", "list-models"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::ListModels => {}
            _ => panic!("expected ListModels command"),
        }
    }
}
