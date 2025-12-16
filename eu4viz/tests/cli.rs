//! CLI integration tests using pre-built binaries
//!
//! Uses `assert_cmd` with `CARGO_BIN_EXE_eu4viz` to run the pre-built binary,
//! avoiding the `cargo run` approach which caused test hangs from parallel
//! compile lock contention.

use assert_cmd::Command;
use predicates::str::contains;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_eu4viz"));
    cmd.arg("--help").assert().success();
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_eu4viz"));
    cmd.arg("--version").assert().success();
}

#[test]
fn test_cli_pretty_print() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut file = File::create(file_path).unwrap();
    writeln!(file, "test = foo").unwrap();

    let path = dir.path().to_str().unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_eu4viz"));
    cmd.args(["--eu4-path", path, "--pretty-print"])
        .assert()
        .success()
        .stdout(contains("test = foo"));
}
