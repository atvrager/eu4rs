use assert_cmd::Command;
use std::fs;

#[test]
fn test_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Development automation scripts"));
}

#[test]
fn test_quota_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("quota").arg("--help").assert().success();
}

#[test]
fn test_coverage_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("coverage").arg("--help").assert().success();
}

#[test]
fn test_snapshot_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("snapshot").arg("--help").assert().success();
}

#[test]
fn test_coverage_mock() {
    let dir = tempfile::tempdir().unwrap();
    let eu4_path = dir.path();

    // Create mock directory structure
    let common = eu4_path.join("common/countries");
    let history = eu4_path.join("history/provinces");
    fs::create_dir_all(&common).unwrap();
    fs::create_dir_all(&history).unwrap();

    // Create dummy files
    fs::write(common.join("Sweden.txt"), "color = { 1 1 1 }").unwrap();
    fs::write(history.join("1 - Stockholm.txt"), "owner = SWE").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("coverage")
        .arg("--eu4-path")
        .arg(eu4_path.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains("EU4 Data Coverage Report"))
        .stdout(predicates::str::contains("Countries"))
        .stdout(predicates::str::contains("Provinces History"));
}
