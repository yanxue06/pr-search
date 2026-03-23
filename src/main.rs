use std::path::PathBuf;

use clap::Parser;

use pr_search::cli::{Cli, Commands};
use pr_search::embedding::ModelManager;
use pr_search::errors::format_error_chain;
use pr_search::github::GitHubFetcher;
use pr_search::index::{IndexBuilder, IndexStorage};
use pr_search::search::{SearchEngine, SearchFilter};
use pr_search::tui::App;

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
    let repo_path = cli
        .path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    match cli.command {
        Commands::Init { force } => {
            let model = ModelManager::new(ModelManager::default_model_dir());
            println!("Downloading embedding model (BGE-small-en-v1.5)...");
            model.download(force)?;
            println!("Model initialized successfully at {:?}", model.model_path());
            Ok(())
        }
        Commands::Index {
            repo,
            force,
            limit,
            with_diffs,
        } => {
            // Fetch PRs from GitHub
            let fetcher = GitHubFetcher::new(&repo)?;
            println!("Fetching PRs from {}...", fetcher.repo_spec());
            let prs = fetcher.fetch_prs(limit, with_diffs)?;
            println!("Fetched {} PRs", prs.len());

            // Load embedding model
            let mut model = ModelManager::new(ModelManager::default_model_dir());
            model.load()?;

            // Build or update index
            let storage = IndexStorage::for_repo(&repo_path)?;
            let mut builder = IndexBuilder::new(&mut model);

            if !force && storage.exists() {
                let mut existing = storage.load()?;
                let added = builder.update(&mut existing, &prs)?;
                storage.save(&existing)?;
                println!(
                    "Updated index: {added} new PRs added ({} total)",
                    existing.len()
                );
            } else {
                let index = builder.build(&repo, &prs, with_diffs)?;
                storage.save(&index)?;
                println!("Built index with {} PRs", index.len());
            }

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
            let filter = SearchFilter {
                author,
                label,
                state,
                after,
                before,
            };
            filter.validate()?;

            // Load index
            let storage = IndexStorage::for_repo(&repo_path)?;
            let index = storage.load()?;

            // Load model and embed query
            let mut model = ModelManager::new(ModelManager::default_model_dir());
            model.load()?;
            let query_embedding = model.embed(&query)?;

            // Search
            let results = SearchEngine::search(&index, &query_embedding, &filter, num_results)?;

            // Display results
            for (i, result) in results.iter().enumerate() {
                let score_pct = (result.score * 100.0) as u32;
                println!(
                    "{:>3}. [{:>3}%] #{:<5} [{}] {}",
                    i + 1,
                    score_pct,
                    result.number,
                    result.state,
                    result.title
                );
                println!(
                    "     @{} | {} | {}",
                    result.author,
                    result.created_at.format("%Y-%m-%d"),
                    result.html_url
                );
                if !result.labels.is_empty() {
                    println!("     labels: {}", result.labels.join(", "));
                }
                println!();
            }

            Ok(())
        }
        Commands::Stats => {
            let storage = IndexStorage::for_repo(&repo_path)?;
            let index = storage.load()?;

            println!("Index Statistics");
            println!("================");
            println!("Repository:    {}", index.repo);
            println!("PRs indexed:   {}", index.len());
            println!(
                "With diffs:    {}",
                if index.with_diffs { "yes" } else { "no" }
            );
            println!("Last PR:       #{}", index.last_pr_number);
            println!(
                "Created:       {}",
                index.created_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            println!(
                "Updated:       {}",
                index.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            println!("Index version: {}", index.version);

            Ok(())
        }
        Commands::Tui => {
            let storage = IndexStorage::for_repo(&repo_path)?;
            let model = ModelManager::new(ModelManager::default_model_dir());
            let mut app = App::new(&storage, model)?;
            app.run()?;
            Ok(())
        }
    }
}
