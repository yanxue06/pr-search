use clap::Parser;

use pr_search::cli::{Cli, Commands};
use pr_search::errors::format_error_chain;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let result = run(cli);

    if let Err(err) = result {
        eprint!("{}", format_error_chain(&err));
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Init { force } => {
            tracing::info!(force, "Initializing embedding model");
            println!("pr-search init: model initialization not yet implemented");
            Ok(())
        }
        Commands::Index {
            repo,
            force,
            limit,
            with_diffs,
        } => {
            tracing::info!(%repo, force, ?limit, with_diffs, "Indexing PRs");
            println!("pr-search index: PR indexing not yet implemented for {repo}");
            Ok(())
        }
        Commands::Search {
            query,
            num_results,
            author,
            label,
            state,
            after,
            before,
        } => {
            tracing::info!(%query, num_results, "Searching PRs");
            let _ = (author, label, state, after, before); // will be used by search engine
            println!("pr-search search: search not yet implemented for \"{query}\"");
            Ok(())
        }
        Commands::Stats => {
            println!("pr-search stats: statistics not yet implemented");
            Ok(())
        }
        Commands::Tui => {
            println!("pr-search tui: TUI not yet implemented");
            Ok(())
        }
    }
}
