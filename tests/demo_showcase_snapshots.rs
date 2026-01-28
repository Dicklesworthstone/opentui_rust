//! Snapshot regression tests for `demo_showcase`.
//!
//! These tests run the demo in headless mode with JSON output and snapshot
//! the results using insta for regression testing.

use std::process::Command;

/// Parse JSON output from headless demo.
fn run_headless_json(args: &[&str]) -> serde_json::Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_demo_showcase"));
    cmd.args(["--headless-smoke", "--headless-dump-json"]);
    cmd.args(args);

    let output = cmd.output().expect("Failed to execute demo_showcase");

    assert!(
        output.status.success(),
        "demo_showcase failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("Failed to parse JSON output")
}

/// Extract a compact snapshot structure from the full JSON.
fn extract_snapshot(json: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "config": {
            "fps_cap": json["config"]["fps_cap"],
            "seed": json["config"]["seed"],
            "cap_preset": json["config"]["cap_preset"],
        },
        "headless_size": json["headless_size"],
        "layout_mode": json["layout_mode"],
        "frames_rendered": json["frames_rendered"],
        "sentinels": json["sentinels"],
        // First and last frame stats only (to keep snapshot small)
        "first_frame_dirty": json["frame_stats"][0]["dirty_cells"],
        "last_frame_dirty": json["last_dirty_cells"],
    })
}

/// Extract snapshot with effective capabilities for degradation testing.
fn extract_capability_snapshot(json: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "config": {
            "cap_preset": json["config"]["cap_preset"],
        },
        "headless_size": json["headless_size"],
        "effective_caps": json["effective_caps"],
        "warnings": json["warnings"],
        "layout_mode": json["layout_mode"],
    })
}

#[test]
fn test_headless_default_snapshot() {
    let json = run_headless_json(&[]);
    let snapshot = extract_snapshot(&json);
    insta::assert_json_snapshot!("headless_default", snapshot);
}

#[test]
fn test_headless_custom_size_snapshot() {
    let json = run_headless_json(&["--headless-size", "120x40"]);
    let snapshot = extract_snapshot(&json);
    insta::assert_json_snapshot!("headless_120x40", snapshot);
}

#[test]
fn test_headless_compact_size_snapshot() {
    // Small size triggers compact layout mode
    let json = run_headless_json(&["--headless-size", "60x20"]);
    let snapshot = extract_snapshot(&json);
    insta::assert_json_snapshot!("headless_compact", snapshot);
}

#[test]
fn test_headless_max_frames_snapshot() {
    let json = run_headless_json(&["--max-frames", "5"]);
    let snapshot = extract_snapshot(&json);
    insta::assert_json_snapshot!("headless_5_frames", snapshot);
}

#[test]
fn test_headless_deterministic() {
    // Run twice with same seed and verify identical output
    let json1 = run_headless_json(&["--seed", "42", "--max-frames", "3"]);
    let json2 = run_headless_json(&["--seed", "42", "--max-frames", "3"]);

    assert_eq!(
        json1["frame_stats"], json2["frame_stats"],
        "Frame stats should be deterministic with same seed"
    );
    assert_eq!(
        json1["sentinels"], json2["sentinels"],
        "Sentinels should be deterministic with same seed"
    );
}

// ============================================================================
// Capability + Size Degradation Matrix (bd-2bnv)
// ============================================================================
//
// These tests verify that the demo stays usable across constrained terminals
// by snapshotting output across different capability presets and sizes.

#[test]
fn test_cap_degradation_ideal_120x40() {
    let json = run_headless_json(&["--headless-size", "120x40", "--cap-preset", "ideal"]);
    let snapshot = extract_capability_snapshot(&json);

    // Assert invariants
    assert!(json["warnings"].as_array().unwrap().is_empty(),
        "Ideal preset should have no warnings");
    assert_eq!(json["effective_caps"]["truecolor"], true);
    assert_eq!(json["effective_caps"]["hyperlinks"], true);
    assert_eq!(json["effective_caps"]["mouse"], true);

    insta::assert_json_snapshot!("cap_ideal_120x40", snapshot);
}

#[test]
fn test_cap_degradation_no_truecolor_120x40() {
    let json = run_headless_json(&["--headless-size", "120x40", "--cap-preset", "no_truecolor"]);
    let snapshot = extract_capability_snapshot(&json);

    // Assert invariants
    let warnings = json["warnings"].as_array().unwrap();
    assert!(!warnings.is_empty(), "no_truecolor preset should have warnings");
    assert_eq!(json["effective_caps"]["truecolor"], false);
    assert_eq!(json["effective_caps"]["hyperlinks"], true); // Still available
    assert_eq!(json["effective_caps"]["mouse"], true); // Still available

    insta::assert_json_snapshot!("cap_no_truecolor_120x40", snapshot);
}

#[test]
fn test_cap_degradation_no_hyperlinks_120x40() {
    let json = run_headless_json(&["--headless-size", "120x40", "--cap-preset", "no_hyperlinks"]);
    let snapshot = extract_capability_snapshot(&json);

    // Assert invariants
    let warnings = json["warnings"].as_array().unwrap();
    assert!(!warnings.is_empty(), "no_hyperlinks preset should have warnings");
    assert_eq!(json["effective_caps"]["hyperlinks"], false);
    assert_eq!(json["effective_caps"]["truecolor"], true); // Still available
    assert_eq!(json["effective_caps"]["mouse"], true); // Still available

    insta::assert_json_snapshot!("cap_no_hyperlinks_120x40", snapshot);
}

#[test]
fn test_cap_degradation_no_mouse_120x40() {
    let json = run_headless_json(&["--headless-size", "120x40", "--cap-preset", "no_mouse"]);
    let snapshot = extract_capability_snapshot(&json);

    // Assert invariants
    let warnings = json["warnings"].as_array().unwrap();
    assert!(!warnings.is_empty(), "no_mouse preset should have warnings");
    assert_eq!(json["effective_caps"]["mouse"], false);
    assert_eq!(json["effective_caps"]["truecolor"], true); // Still available
    assert_eq!(json["effective_caps"]["hyperlinks"], true); // Still available

    insta::assert_json_snapshot!("cap_no_mouse_120x40", snapshot);
}

#[test]
fn test_cap_degradation_minimal_80x24() {
    let json = run_headless_json(&["--headless-size", "80x24", "--cap-preset", "minimal"]);
    let snapshot = extract_capability_snapshot(&json);

    // Assert invariants
    let warnings = json["warnings"].as_array().unwrap();
    assert!(warnings.len() >= 3, "minimal preset should degrade multiple capabilities");
    assert_eq!(json["effective_caps"]["truecolor"], false);
    assert_eq!(json["effective_caps"]["hyperlinks"], false);
    assert_eq!(json["effective_caps"]["mouse"], false);

    insta::assert_json_snapshot!("cap_minimal_80x24", snapshot);
}

#[test]
fn test_cap_degradation_tiny_50x15() {
    let json = run_headless_json(&["--headless-size", "50x15", "--cap-preset", "minimal"]);
    let snapshot = extract_capability_snapshot(&json);

    // Assert invariants - 50x15 triggers Minimal layout (40-59 x 12-15)
    assert_eq!(
        json["layout_mode"].as_str().unwrap(),
        "Minimal",
        "50x15 should trigger Minimal layout mode (single panel, no sidebar)"
    );

    let warnings = json["warnings"].as_array().unwrap();
    assert!(warnings.len() >= 3, "minimal preset should degrade multiple capabilities");
    assert_eq!(json["effective_caps"]["truecolor"], false);
    assert_eq!(json["effective_caps"]["hyperlinks"], false);
    assert_eq!(json["effective_caps"]["mouse"], false);

    insta::assert_json_snapshot!("cap_tiny_50x15", snapshot);
}
