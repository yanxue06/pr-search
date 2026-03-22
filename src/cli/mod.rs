use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "pr-search",
    about = "Semantic search for GitHub Pull Requests",
    version,
    after_help = "Examples:\n  pr-search init                          Download the embedding model\n  pr-search index octocat/hello-world     Index PRs from a repository\n  pr-search search \"fix auth bug\"         Search indexed PRs\n  pr-search tui                           Launch interactive TUI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to a git repository (defaults to current directory)
    #[arg(long, global = true)]
    pub path: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Download and initialize the embedding model
    Init {
        /// Force re-download even if model exists
        #[arg(long)]
        force: bool,
    },

    /// Index PRs from a GitHub repository
    Index {
        /// Repository in owner/repo format (e.g., rust-lang/rust)
        repo: String,

        /// Force full re-index, ignoring any existing index
        #[arg(long)]
        force: bool,

        /// Maximum number of PRs to fetch (default: all)
        #[arg(long, short = 'n')]
        limit: Option<usize>,

        /// Include PR diffs in the index (slower but more accurate)
        #[arg(long)]
        with_diffs: bool,
    },

    /// Search indexed PRs using natural language
    Search {
        /// Natural language search query
        query: String,

        /// Maximum number of results to show
        #[arg(long, short = 'n', default_value = "10")]
        num_results: usize,

        /// Filter by PR author
        #[arg(long)]
        author: Option<String>,

        /// Filter by PR label
        #[arg(long)]
        label: Option<String>,

        /// Filter by PR state (open, closed, merged)
        #[arg(long)]
        state: Option<String>,

        /// Filter PRs created after this date (YYYY-MM-DD)
        #[arg(long)]
        after: Option<String>,

        /// Filter PRs created before this date (YYYY-MM-DD)
        #[arg(long)]
        before: Option<String>,
    },

    /// Show index statistics
    Stats,

    /// Launch interactive TUI for searching PRs
    Tui,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parses_init() {
        let cli = Cli::parse_from(["pr-search", "init"]);
        assert!(matches!(cli.command, Commands::Init { force: false }));
    }

    #[test]
    fn test_cli_parses_init_force() {
        let cli = Cli::parse_from(["pr-search", "init", "--force"]);
        assert!(matches!(cli.command, Commands::Init { force: true }));
    }

    #[test]
    fn test_cli_parses_index() {
        let cli = Cli::parse_from(["pr-search", "index", "rust-lang/rust"]);
        match cli.command {
            Commands::Index {
                repo,
                force,
                limit,
                with_diffs,
            } => {
                assert_eq!(repo, "rust-lang/rust");
                assert!(!force);
                assert!(limit.is_none());
                assert!(!with_diffs);
            }
            _ => panic!("Expected Index command"),
        }
    }

    #[test]
    fn test_cli_parses_index_with_options() {
        let cli = Cli::parse_from([
            "pr-search",
            "index",
            "owner/repo",
            "--force",
            "-n",
            "50",
            "--with-diffs",
        ]);
        match cli.command {
            Commands::Index {
                repo,
                force,
                limit,
                with_diffs,
            } => {
                assert_eq!(repo, "owner/repo");
                assert!(force);
                assert_eq!(limit, Some(50));
                assert!(with_diffs);
            }
            _ => panic!("Expected Index command"),
        }
    }

    #[test]
    fn test_cli_parses_search() {
        let cli = Cli::parse_from(["pr-search", "search", "fix auth bug"]);
        match cli.command {
            Commands::Search {
                query, num_results, ..
            } => {
                assert_eq!(query, "fix auth bug");
                assert_eq!(num_results, 10);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parses_search_with_filters() {
        let cli = Cli::parse_from([
            "pr-search",
            "search",
            "memory leak",
            "--author",
            "octocat",
            "--label",
            "bug",
            "--state",
            "merged",
            "--after",
            "2025-01-01",
            "--before",
            "2026-01-01",
            "-n",
            "5",
        ]);
        match cli.command {
            Commands::Search {
                query,
                num_results,
                author,
                label,
                state,
                after,
                before,
            } => {
                assert_eq!(query, "memory leak");
                assert_eq!(num_results, 5);
                assert_eq!(author.as_deref(), Some("octocat"));
                assert_eq!(label.as_deref(), Some("bug"));
                assert_eq!(state.as_deref(), Some("merged"));
                assert_eq!(after.as_deref(), Some("2025-01-01"));
                assert_eq!(before.as_deref(), Some("2026-01-01"));
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parses_global_path() {
        let cli = Cli::parse_from(["pr-search", "--path", "/tmp/repo", "stats"]);
        assert_eq!(cli.path.as_deref(), Some("/tmp/repo"));
        assert!(matches!(cli.command, Commands::Stats));
    }

    #[test]
    fn test_cli_verify_app() {
        // clap's built-in validation
        Cli::command().debug_assert();
    }
}
