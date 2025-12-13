use std::fs::File;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
}

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--version"])
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
}

#[test]
fn test_cli_eu4_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args(&["run", "--", "--eu4-path", path])
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
}

#[test]
fn test_cli_pretty_print() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut file = File::create(file_path).unwrap();
    writeln!(file, "test = foo").unwrap();

    let path = dir.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args(&["run", "--", "--eu4-path", path, "--pretty-print"])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8(output.stdout).unwrap();
    println!("{}", stdout);
    assert!(stdout.contains("test = foo"));
}
