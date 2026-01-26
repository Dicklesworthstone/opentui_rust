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
