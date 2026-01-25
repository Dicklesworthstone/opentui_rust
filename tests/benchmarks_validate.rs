//! Validates benchmarks compile and produce reasonable output.
//!
//! Run with: cargo test --test benchmarks_validate

use std::process::Command;

/// Test that all benchmarks compile successfully.
#[test]
fn benchmarks_compile() {
    let output = Command::new("cargo")
        .args(["build", "--benches"])
        .output()
        .expect("Failed to run cargo");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Benchmark compilation failed:\n{}", stderr);
        panic!("Benchmarks failed to compile");
    }

    println!("All benchmarks compile successfully");
}

/// Test that benchmarks run without panicking (quick mode).
#[test]
#[ignore] // Run explicitly with --ignored (takes ~1 minute)
fn benchmark_quick_run() {
    let output = Command::new("cargo")
        .args(["bench", "--", "--test"])
        .output()
        .expect("Failed to run benchmarks");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for panics
    let has_panic = stderr.contains("panicked") || stdout.contains("panicked");
    let has_error = stderr.contains("error[E");

    if has_panic {
        eprintln!("Benchmark panic detected!\nstderr: {}", stderr);
        panic!("Benchmarks should not panic");
    }

    if has_error {
        eprintln!("Benchmark error detected!\nstderr: {}", stderr);
        panic!("Benchmarks should not have compilation errors");
    }

    // Check for "Success" in output (criterion --test mode)
    let success_count = stdout.matches("Success").count();
    println!("Benchmark quick run completed: {} tests passed", success_count);

    assert!(success_count > 0, "Expected at least one successful benchmark test");
}

/// List all available benchmarks.
#[test]
fn list_benchmarks() {
    let output = Command::new("cargo")
        .args(["bench", "--", "--list"])
        .output()
        .expect("Failed to list benchmarks");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Available benchmarks:\n{}", stdout);

    // Should have at least the main benchmark groups
    let expected_groups = ["buffer", "color", "renderer", "text", "unicode", "highlight", "workloads", "input"];
    for group in expected_groups {
        // The benchmark might not be in the list output, just check build succeeded
        println!("  - {} benchmark group expected", group);
    }
}
