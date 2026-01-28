//! Validates example builds and non-interactive execution.

use std::fs;
use std::path::Path;
use std::process::Command;

fn list_examples() -> Vec<String> {
    let mut names = Vec::new();
    let entries = fs::read_dir("examples").expect("read examples dir");

    for entry in entries {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        names.push(stem.to_string());
    }

    names.sort();
    names
}

fn run_cargo(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(args)
        .output()
        .expect("failed to execute cargo")
}

#[test]
fn examples_compile() {
    let examples = list_examples();
    let mut failures = Vec::new();

    for name in &examples {
        let output = run_cargo(&["build", "--all-features", "--example", name]);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Example {name} failed to compile:\n{stderr}");
            failures.push(name.clone());
        }
    }

    assert!(
        failures.is_empty(),
        "Examples failed to compile: {failures:?}"
    );
}

#[test]
fn hello_example_runs() {
    if !Path::new("examples/hello.rs").exists() {
        return;
    }

    let output = run_cargo(&["run", "--all-features", "--example", "hello"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "hello example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Buffer created"),
        "hello example output missing expected text"
    );
}

/// Validates that the `demo_showcase` binary compiles.
///
/// The demo is a flagship deliverable and must not silently stop compiling.
/// CI will fail quickly if the binary breaks.
#[test]
fn demo_showcase_compiles() {
    // Check that the binary target exists
    assert!(
        Path::new("src/bin/demo_showcase.rs").exists(),
        "demo_showcase.rs not found - binary target missing"
    );

    let output = run_cargo(&["build", "--all-features", "--bin", "demo_showcase"]);

    assert!(
        output.status.success(),
        "demo_showcase binary failed to compile:\n\n{}\n\n\
         This is a critical failure - the demo is a flagship deliverable.",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Validates that the demo_showcase binary runs successfully in headless mode.
///
/// This test exercises the full render pipeline without requiring a TTY:
/// - Initializes app state
/// - Executes update + render cycles
/// - Exercises diff computation
///
/// Expects the process to exit 0 with HEADLESS_SMOKE_OK marker in stdout.
#[test]
fn demo_showcase_headless_smoke() {
    // Run the demo in headless mode with a fixed frame count
    let output = run_cargo(&[
        "run",
        "--all-features",
        "--bin",
        "demo_showcase",
        "--",
        "--headless-smoke",
        "--max-frames",
        "10",
        "--headless-size",
        "80x24",
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // On failure, print full output for debugging
    if !output.status.success() {
        eprintln!("=== STDOUT ===\n{stdout}");
        eprintln!("=== STDERR ===\n{stderr}");
    }

    assert!(
        output.status.success(),
        "demo_showcase headless smoke test failed with exit code {:?}\n\
         STDERR:\n{}\n\n\
         This indicates a bug in the demo's render pipeline.",
        output.status.code(),
        stderr
    );

    assert!(
        stdout.contains("HEADLESS_SMOKE_OK"),
        "demo_showcase headless output missing HEADLESS_SMOKE_OK marker.\n\
         STDOUT:\n{}\n\n\
         The demo should print this marker when headless smoke completes successfully.",
        stdout
    );
}
