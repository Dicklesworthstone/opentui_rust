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
    assert!(
        json["warnings"].as_array().unwrap().is_empty(),
        "Ideal preset should have no warnings"
    );
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
    assert!(
        !warnings.is_empty(),
        "no_truecolor preset should have warnings"
    );
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
    assert!(
        !warnings.is_empty(),
        "no_hyperlinks preset should have warnings"
    );
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
    assert!(
        warnings.len() >= 3,
        "minimal preset should degrade multiple capabilities"
    );
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
    assert!(
        warnings.len() >= 3,
        "minimal preset should degrade multiple capabilities"
    );
    assert_eq!(json["effective_caps"]["truecolor"], false);
    assert_eq!(json["effective_caps"]["hyperlinks"], false);
    assert_eq!(json["effective_caps"]["mouse"], false);

    insta::assert_json_snapshot!("cap_tiny_50x15", snapshot);
}

// ============================================================================
// Tour Determinism Regression Tests (bd-bqd1)
// ============================================================================
//
// These tests verify that the guided tour produces deterministic output
// across runs, enabling snapshot-based regression testing.

/// Extract tour state from headless JSON output.
fn extract_tour_snapshot(json: &serde_json::Value) -> serde_json::Value {
    // Extract step transition frame numbers only (not full transitions) for stable snapshots
    let step_frames: Vec<u64> = json["tour_state"]["step_transitions"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|t| t["frame"].as_u64()).collect())
        .unwrap_or_default();

    serde_json::json!({
        "config": {
            "seed": json["config"]["seed"],
        },
        "tour_completed": json["tour_state"]["completed"],
        "total_steps": json["tour_state"]["total_steps"],
        "final_step_idx": json["tour_state"]["final_step_idx"],
        "step_transition_frames": step_frames,
    })
}

#[test]
fn test_tour_determinism_snapshot() {
    // Run tour with fixed seed and enough frames to complete
    // Tour takes ~3090 frames (51.5s at 60fps), so use 3200 for buffer
    let json = run_headless_json(&["--seed", "42", "--tour", "--max-frames", "3200"]);
    let snapshot = extract_tour_snapshot(&json);

    // Assert tour completed successfully
    assert!(
        json["tour_state"]["completed"].as_bool().unwrap_or(false),
        "Tour should complete in headless mode with 3200 frames"
    );

    // Assert we have step transitions
    let transitions = json["tour_state"]["step_transitions"]
        .as_array()
        .map_or(0, Vec::len);
    assert!(
        transitions > 0,
        "Tour should have step transitions recorded"
    );

    insta::assert_json_snapshot!("tour_determinism", snapshot);
}

#[test]
fn test_tour_step_titles_present() {
    // Run tour with enough frames to see some transitions
    let json = run_headless_json(&["--seed", "42", "--tour", "--max-frames", "3000"]);

    // Verify tour_state contains expected fields
    let tour_state = &json["tour_state"];
    assert!(
        !tour_state.is_null(),
        "tour_state should be present in JSON output"
    );
    assert!(
        tour_state["total_steps"].as_u64().unwrap_or(0) > 0,
        "Tour should have steps defined"
    );
    assert!(
        tour_state["step_transitions"].is_array(),
        "step_transitions should be present in tour_state"
    );

    let step_transitions = tour_state["step_transitions"].as_array().unwrap();
    assert!(
        !step_transitions.is_empty(),
        "Tour should have step transitions recorded"
    );

    // Verify each transition has a title
    for transition in step_transitions {
        assert!(
            transition["title"].as_str().is_some_and(|s| !s.is_empty()),
            "Each step transition should have a non-empty title"
        );
    }
}

#[test]
fn test_tour_deterministic_across_runs() {
    // Run tour twice with identical seeds and enough frames to complete
    // Tour takes ~3090 frames (51.5s at 60fps), so use 3200 for buffer
    let json1 = run_headless_json(&["--seed", "123", "--tour", "--max-frames", "3200"]);
    let json2 = run_headless_json(&["--seed", "123", "--tour", "--max-frames", "3200"]);

    // Step transitions should be identical
    assert_eq!(
        json1["tour_state"]["step_transitions"], json2["tour_state"]["step_transitions"],
        "Tour step transitions should be deterministic with same seed"
    );
    assert_eq!(
        json1["tour_state"]["final_step_idx"], json2["tour_state"]["final_step_idx"],
        "Tour final step should be deterministic with same seed"
    );
    assert_eq!(
        json1["frames_rendered"], json2["frames_rendered"],
        "Frame count should be deterministic with same seed"
    );
}

#[test]
fn test_tour_exits_cleanly() {
    // This test verifies clean exit by checking the process succeeds
    // (run_headless_json already asserts success, but we verify tour-specific output)
    // Tour takes ~3090 frames (51.5s at 60fps), so use 3200 for buffer
    let json = run_headless_json(&["--seed", "42", "--tour", "--max-frames", "3200"]);

    // Verify tour completed (not just started)
    assert!(
        json["tour_state"]["completed"].as_bool().unwrap_or(false),
        "Tour should complete and exit cleanly"
    );

    // Verify we reached the expected final step
    let final_step = json["tour_state"]["final_step_idx"].as_u64().unwrap_or(0);
    let total_steps = json["tour_state"]["total_steps"].as_u64().unwrap_or(0);
    assert!(
        final_step >= total_steps - 1,
        "Tour should reach the final step before exiting"
    );
}
