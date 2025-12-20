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

#[test]
fn test_coverage_missing_path() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("coverage")
        .arg("--eu4-path")
        .arg("non_existent_path_xyz_123")
        .assert()
        .success() // It returns Ok(()) even if path is missing, just prints warning
        .stdout(predicates::str::contains("EU4 path not found"));
}

#[test]
fn test_coverage_doc_gen() {
    let dir = tempfile::tempdir().unwrap();
    // Run in temp dir to verify file creation without dirtying repo
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.current_dir(&dir)
        .arg("coverage")
        .arg("--doc-gen")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Generated docs/reference/supported-fields.md",
        ));

    assert!(dir
        .path()
        .join("docs/reference/supported-fields.md")
        .exists());
}

#[test]
fn test_coverage_discover_updates() {
    let dir = tempfile::tempdir().unwrap();
    let eu4_path = dir.path().join("game");
    fs::create_dir_all(&eu4_path).unwrap();

    // Create minimal structure for discovery
    let common = eu4_path.join("common");
    fs::create_dir_all(&common).unwrap();

    // Mock project structure in temp dir for generated files
    let project_root = dir.path();
    let src_generated = project_root.join("eu4data/src/generated");
    fs::create_dir_all(&src_generated).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.current_dir(project_root)
        .arg("coverage")
        .arg("--eu4-path")
        .arg(eu4_path.to_str().unwrap())
        .arg("--discover")
        .arg("--update")
        .assert()
        .success()
        .stdout(predicates::str::contains("Discovery mode enabled"))
        .stdout(predicates::str::contains(
            "Updated eu4data/src/generated/categories.rs",
        ))
        .stdout(predicates::str::contains(
            "Updated eu4data/src/generated/schema.rs",
        ));

    assert!(src_generated.join("categories.rs").exists());
    assert!(src_generated.join("schema.rs").exists());
}

#[test]
fn test_quota_execution() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    // Just ensure it runs without crashing
    cmd.arg("quota").assert().success();
}

#[test]
fn test_ci_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_xtask"));
    cmd.arg("ci").arg("--help").assert().success();
}
