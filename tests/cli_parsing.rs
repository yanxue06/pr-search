use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_binary_version() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("pr-search"));
}

#[test]
fn test_binary_help() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Semantic search for GitHub Pull Requests",
        ))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("index"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("stats"))
        .stdout(predicate::str::contains("tui"));
}

#[test]
fn test_init_help() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .args(["init", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("embedding model"));
}

#[test]
fn test_search_help() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .args(["search", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("query"))
        .stdout(predicate::str::contains("--author"))
        .stdout(predicate::str::contains("--label"));
}

#[test]
fn test_unknown_subcommand() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_search_missing_query() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .arg("search")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_index_missing_repo() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .arg("index")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_examples_in_help() {
    Command::cargo_bin("pr-search")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("pr-search init"));
}
