# pr-search

A command-line tool for semantic search over GitHub Pull Requests. Instead of relying on keyword matching, pr-search uses a local embedding model (BGE-small-en-v1.5) to understand the meaning of your queries and find the most relevant PRs by cosine similarity. It also includes an interactive terminal UI for browsing results and opening PRs directly in your browser.

## Features

- **Semantic search** -- find PRs by meaning, not just keywords, using cosine similarity over 384-dimensional embeddings
- **Local inference** -- all embedding happens on your machine via ONNX Runtime; no API keys or cloud services needed
- **Incremental indexing** -- only new PRs are embedded when you re-index, so updates are fast
- **Rich filtering** -- narrow results by author, label, state (open/closed/merged), and date range
- **Interactive TUI** -- a terminal interface built with ratatui for real-time search, keyboard navigation, and one-key PR opening in your browser
- **Diff-aware indexing** -- optionally include PR diffs in the index for more accurate search results
- **Structured error handling** -- every error includes a stable code, a human-readable message, and an actionable hint

## Prerequisites

- **Rust** (1.70+) -- for building from source
- **GitHub CLI (`gh`)** -- used to fetch PR data from GitHub. Install it from <https://cli.github.com> and authenticate with `gh auth login`
- A **git repository** -- pr-search stores its index inside the `.git` directory of the repo you run it from

## Installation

```sh
git clone https://github.com/yanxue06/pr-search.git
cd pr-search
cargo build --release
```

The binary will be at `target/release/pr-search`. You can copy it to a directory on your PATH or run it directly.

## Usage

### 1. Download the embedding model

Run this once to download the BGE-small-en-v1.5 ONNX model and tokenizer from HuggingFace:

```sh
pr-search init
```

The model is stored in a platform-appropriate data directory (e.g. `~/Library/Application Support/com.pr-search.pr-search` on macOS). Use `--force` to re-download.

### 2. Index a repository's PRs

Navigate to a local git repository, then index PRs from its GitHub remote:

```sh
pr-search index rust-lang/rust
```

Options:

| Flag | Description |
|------|-------------|
| `-n <N>` / `--limit <N>` | Fetch at most N PRs (default: 1000) |
| `--force` | Rebuild the index from scratch |
| `--with-diffs` | Include PR diffs in the index for better search accuracy (slower) |

Re-running `pr-search index` without `--force` performs an incremental update, embedding only PRs that are not already in the index.

### 3. Search

```sh
pr-search search "fix authentication race condition"
```

Options:

| Flag | Description |
|------|-------------|
| `-n <N>` / `--num-results <N>` | Number of results to return (default: 10) |
| `--author <name>` | Filter by author (case-insensitive partial match) |
| `--label <label>` | Filter by label (case-insensitive partial match) |
| `--state <state>` | Filter by state: `open`, `closed`, or `merged` |
| `--after <YYYY-MM-DD>` | Only PRs created after this date |
| `--before <YYYY-MM-DD>` | Only PRs created before this date |

Example with filters:

```sh
pr-search search "memory leak" --author alice --state merged --after 2025-01-01 -n 5
```

### 4. Interactive TUI

```sh
pr-search tui
```

The TUI provides a search input and a scrollable results list. Key bindings:

| Key | Action |
|-----|--------|
| Type text + Enter | Submit a search query |
| Tab | Switch focus between search input and results list |
| Up/Down or k/j | Navigate results |
| Enter or o | Open the selected PR in your browser |
| / | Jump back to the search input |
| Esc or Ctrl-C | Quit |

### 5. View index statistics

```sh
pr-search stats
```

Displays the repository name, number of indexed PRs, whether diffs are included, and timestamps.

### Global options

| Flag | Description |
|------|-------------|
| `--path <dir>` | Use a different git repository directory instead of the current one |

## How it works

```
GitHub (via gh CLI)
       |
       v
  +-----------+       +------------------+       +--------------+
  |  Fetcher   | ----> |  Index Builder   | ----> |   Storage    |
  | (gh pr     |       | (ONNX embed +   |       | (bincode     |
  |  list/api) |       |  tokenizer)      |       |  serialize)  |
  +-----------+       +------------------+       +--------------+
                                                        |
                                                        v
                                                  .git/semantic-pr-index
                                                        |
                       +------------------+             |
  User query --------> |  Search Engine   | <-----------+
                       | (cosine sim +    |
                       |  filters)        |
                       +------------------+
                              |
                       +------+------+
                       |             |
                    CLI output    TUI (ratatui)
```

1. **Fetching** -- The `GitHubFetcher` shells out to `gh pr list` and `gh api` to retrieve PR metadata, review comments, and optionally diffs.
2. **Embedding** -- The `ModelManager` downloads the BGE-small-en-v1.5 ONNX model from HuggingFace, tokenizes PR text (title + body + comments + optional diff), and runs inference locally via `ort` (ONNX Runtime). Each PR becomes a 384-dimensional L2-normalized vector.
3. **Indexing** -- The `IndexBuilder` embeds all PRs and stores them in a `SemanticIndex`. The `IndexStorage` layer serializes the index with bincode and writes it atomically to `.git/semantic-pr-index`. Incremental updates skip PRs that are already indexed.
4. **Searching** -- The `SearchEngine` embeds the user's query with the same model, computes cosine similarity against every indexed PR, applies any active filters (author, label, state, date range), and returns results ranked by score.
5. **Display** -- Results are either printed to stdout or rendered in an interactive TUI built with ratatui and crossterm, where users can browse results and open PRs in their browser.

## Project structure

```
src/
  main.rs            -- Entry point and command dispatch
  lib.rs             -- Module exports
  cli/mod.rs         -- CLI argument parsing (clap)
  github/
    mod.rs           -- Module exports
    model.rs         -- PR data model (PrData, PrState, ReviewComment)
    fetcher.rs       -- GitHub data fetching via gh CLI
  embedding/
    mod.rs           -- Constants and L2 normalization
    manager.rs       -- ONNX model download, loading, and inference
  index/
    mod.rs           -- SemanticIndex and IndexEntry types
    builder.rs       -- Index construction and incremental updates
    storage.rs       -- Bincode serialization and atomic file I/O
  search/
    mod.rs           -- Module exports
    engine.rs        -- Cosine similarity search
    filter.rs        -- Multi-field filtering (author, label, state, date)
  tui/
    mod.rs           -- Module exports
    app.rs           -- TUI application state and event loop
    ui.rs            -- Terminal rendering with ratatui
  errors/
    mod.rs           -- DomainError trait and error formatting
    embedding.rs     -- Embedding-related errors (E1xxx)
    github.rs        -- GitHub-related errors (E2xxx)
    index.rs         -- Index-related errors (E3xxx)
    search.rs        -- Search-related errors (E4xxx)
tests/
  cli_parsing.rs     -- Integration tests for the CLI binary
```

## License

MIT
