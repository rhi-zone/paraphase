//! Integration tests for cambium CLI.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cambium_bin() -> PathBuf {
    // Build the binary if needed and return its path
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/cambium");
    path
}

fn test_data_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/data");
    path
}

fn setup() {
    // Build the CLI
    Command::new("cargo")
        .args(["build", "-p", "cambium-cli"])
        .status()
        .expect("Failed to build CLI");

    // Create test data directory
    let data_dir = test_data_dir();
    fs::create_dir_all(&data_dir).ok();
}

#[test]
fn test_help() {
    setup();
    let output = Command::new(cambium_bin())
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Type-driven data transformation"));
}

#[test]
fn test_list() {
    setup();
    let output = Command::new(cambium_bin())
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Available converters"));
}

#[test]
fn test_json_to_yaml_conversion() {
    setup();
    let data_dir = test_data_dir();

    // Create test JSON file
    let input = data_dir.join("test.json");
    let output = data_dir.join("test.yaml");
    fs::write(&input, r#"{"name": "test", "value": 42}"#).expect("Failed to write test file");

    // Convert
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify output exists
    assert!(output.exists(), "Output file not created");

    // Verify content
    let content = fs::read_to_string(&output).expect("Failed to read output");
    assert!(content.contains("name:") || content.contains("name :")); // YAML format
    assert!(content.contains("test"));

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_yaml_to_json_conversion() {
    setup();
    let data_dir = test_data_dir();

    // Create test YAML file
    let input = data_dir.join("test2.yaml");
    let output = data_dir.join("test2.json");
    fs::write(&input, "name: hello\ncount: 123\n").expect("Failed to write test file");

    // Convert
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify output exists
    assert!(output.exists(), "Output file not created");

    // Verify content is JSON
    let content = fs::read_to_string(&output).expect("Failed to read output");
    assert!(content.contains("{"));
    assert!(content.contains("\"name\""));

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_format_detection_from_to_flags() {
    setup();
    let data_dir = test_data_dir();

    // Create a file with no extension
    let input = data_dir.join("noext");
    let output = data_dir.join("noext_out");
    fs::write(&input, r#"{"foo": "bar"}"#).expect("Failed to write test file");

    // Convert with explicit format flags
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--from",
            "json",
            "--to",
            "yaml",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists(), "Output file not created");

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_plan_command() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["plan", "input.json", "output.yaml"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Planning:") || stdout.contains("json") || stdout.contains("yaml"));
}

#[test]
fn test_quiet_mode() {
    setup();
    let data_dir = test_data_dir();

    let input = data_dir.join("quiet_test.json");
    let output = data_dir.join("quiet_test.yaml");
    fs::write(&input, r#"{"test": true}"#).expect("Failed to write test file");

    // Convert with -q flag
    let result = Command::new(cambium_bin())
        .args([
            "-q",
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(result.status.success());
    // Quiet mode should produce no stdout
    assert!(result.stdout.is_empty(), "Expected no output in quiet mode");

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_completions() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["completions", "bash"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("complete") || stdout.contains("cambium"));
}

#[test]
fn test_error_on_missing_input() {
    setup();
    let result = Command::new(cambium_bin())
        .args(["convert", "nonexistent.json", "-o", "out.yaml"])
        .output()
        .expect("Failed to execute command");

    assert!(!result.status.success(), "Expected command to fail");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Failed to read") || stderr.contains("error") || stderr.contains("Error")
    );
}
