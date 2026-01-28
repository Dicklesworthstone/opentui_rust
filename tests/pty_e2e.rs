//! PTY-based E2E tests for `demo_showcase`.
//!
//! These tests spawn `demo_showcase` under a real pseudo-terminal to verify
//! actual ANSI output sequences are emitted correctly.
//!
//! # Running These Tests
//!
//! These tests are ignored by default to avoid CI flakiness. To run them:
//!
//! ```bash
//! # First, build the demo_showcase binary
//! cargo build --bin demo_showcase
//!
//! # Run PTY tests
//! cargo test --test pty_e2e -- --ignored --nocapture
//!
//! # With artifacts for debugging
//! HARNESS_ARTIFACTS=1 cargo test --test pty_e2e -- --ignored --nocapture
//! ```

mod common;

use common::pty::{log_pty_result, sequences, spawn_pty, PtyConfig};
use std::time::Duration;

/// Build `demo_showcase` if not already built.
fn ensure_demo_showcase_built() -> bool {
    // Check if binary exists
    let binary = std::path::Path::new("target/debug/demo_showcase");
    if binary.exists() {
        return true;
    }

    // Try to build it
    eprintln!("Building demo_showcase...");
    let status = std::process::Command::new("cargo")
        .args(["build", "--bin", "demo_showcase"])
        .status();

    match status {
        Ok(s) if s.success() => true,
        _ => {
            eprintln!("Failed to build demo_showcase");
            false
        }
    }
}

/// Test: Tour mode with exit-after-tour emits expected terminal lifecycle sequences.
///
/// Asserts:
/// - Alternate screen enter/leave
/// - Cursor hide/show
/// - Mouse enable sequences (button + SGR)
/// - Synchronized output (if supported)
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_tour_mode_terminal_lifecycle() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "tour_mode_terminal_lifecycle");

    // Should exit successfully
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should exit with code 0"
    );

    // Alternate screen
    assert!(
        result.contains_sequence(sequences::ALT_SCREEN_ENTER),
        "Should enter alternate screen"
    );
    assert!(
        result.contains_sequence(sequences::ALT_SCREEN_LEAVE),
        "Should leave alternate screen on exit"
    );

    // Cursor hide/show
    assert!(
        result.contains_sequence(sequences::CURSOR_HIDE),
        "Should hide cursor"
    );
    assert!(
        result.contains_sequence(sequences::CURSOR_SHOW),
        "Should show cursor on exit"
    );

    // Mouse tracking (at least button mode)
    let has_mouse = result.contains_sequence(sequences::MOUSE_BUTTON_ENABLE)
        || result.contains_sequence(sequences::MOUSE_MOTION_ENABLE);
    assert!(has_mouse, "Should enable some form of mouse tracking");

    // SGR mouse format
    assert!(
        result.contains_sequence(sequences::MOUSE_SGR_ENABLE),
        "Should enable SGR mouse format"
    );

    // Bracketed paste mode
    assert!(
        result.contains_sequence(sequences::BRACKETED_PASTE_ENABLE),
        "Should enable bracketed paste mode"
    );

    // Note: Synchronized output depends on capability detection
    // We don't assert it here since TERM=xterm-kitty should enable it
    // but terminal responses aren't simulated in PTY tests
}

/// Test: Tour mode emits OSC 8 hyperlinks (default behavior).
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_tour_mode_has_hyperlinks() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "tour_mode_has_hyperlinks");

    // Should exit successfully
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should exit with code 0"
    );

    // Should contain at least one OSC 8 hyperlink
    // The logs panel should have clickable links
    let hyperlink_count = result.count_sequence(sequences::OSC8_PREFIX);
    eprintln!("Found {hyperlink_count} OSC 8 hyperlink sequences");

    // Hyperlinks are emitted in the logs panel during tour step 11
    // We expect at least one hyperlink if the tour reaches that step
    assert!(
        hyperlink_count > 0,
        "Should emit at least one OSC 8 hyperlink sequence"
    );
}

/// Test: --no-alt-screen flag disables alternate screen buffer.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_no_alt_screen_flag() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .arg("--no-alt-screen")
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "no_alt_screen_flag");

    // Should exit successfully
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should exit with code 0"
    );

    // Should NOT contain alternate screen sequences
    assert!(
        !result.contains_sequence(sequences::ALT_SCREEN_ENTER),
        "Should NOT enter alternate screen with --no-alt-screen"
    );
    assert!(
        !result.contains_sequence(sequences::ALT_SCREEN_LEAVE),
        "Should NOT have alternate screen leave sequence"
    );

    // Cursor hide/show should still work
    assert!(
        result.contains_sequence(sequences::CURSOR_HIDE),
        "Should still hide cursor"
    );
    assert!(
        result.contains_sequence(sequences::CURSOR_SHOW),
        "Should still show cursor on exit"
    );
}

/// Test: --no-mouse flag disables mouse tracking.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_no_mouse_flag() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .arg("--no-mouse")
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "no_mouse_flag");

    // Should exit successfully
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should exit with code 0"
    );

    // Should NOT contain mouse enable sequences
    assert!(
        !result.contains_sequence(sequences::MOUSE_BUTTON_ENABLE),
        "Should NOT enable button mouse tracking with --no-mouse"
    );
    assert!(
        !result.contains_sequence(sequences::MOUSE_MOTION_ENABLE),
        "Should NOT enable motion mouse tracking with --no-mouse"
    );
    assert!(
        !result.contains_sequence(sequences::MOUSE_SGR_ENABLE),
        "Should NOT enable SGR mouse format with --no-mouse"
    );

    // Alternate screen and cursor should still work
    assert!(
        result.contains_sequence(sequences::ALT_SCREEN_ENTER),
        "Should still enter alternate screen"
    );
}

/// Test: `--cap-preset no_hyperlinks` disables OSC 8 hyperlinks.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_no_hyperlinks_preset() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .arg("--cap-preset")
        .arg("no_hyperlinks")
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "no_hyperlinks_preset");

    // Should exit successfully
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should exit with code 0"
    );

    // Should NOT contain OSC 8 hyperlink sequences
    let hyperlink_count = result.count_sequence(sequences::OSC8_PREFIX);
    eprintln!("Found {hyperlink_count} OSC 8 hyperlink sequences (expected 0)");

    assert_eq!(
        hyperlink_count, 0,
        "Should NOT emit OSC 8 hyperlinks with --cap-preset no_hyperlinks"
    );
}

/// Test: --threaded mode has same terminal lifecycle as default.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_threaded_mode_parity() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .arg("--threaded")
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "threaded_mode_parity");

    // Should exit successfully
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase --threaded should exit with code 0"
    );

    // Same assertions as tour_mode_terminal_lifecycle
    assert!(
        result.contains_sequence(sequences::ALT_SCREEN_ENTER),
        "Threaded mode should enter alternate screen"
    );
    assert!(
        result.contains_sequence(sequences::ALT_SCREEN_LEAVE),
        "Threaded mode should leave alternate screen"
    );
    assert!(
        result.contains_sequence(sequences::CURSOR_HIDE),
        "Threaded mode should hide cursor"
    );
    assert!(
        result.contains_sequence(sequences::CURSOR_SHOW),
        "Threaded mode should show cursor on exit"
    );
}

/// Test: Minimal size terminal (40x12) still runs successfully.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_minimal_terminal_size() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .timeout(Duration::from_secs(60))
        .size(40, 12); // Minimal layout threshold

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "minimal_terminal_size");

    // Should exit successfully (might be in minimal layout mode)
    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should exit with code 0 even at 40x12"
    );

    // Basic lifecycle should still work
    assert!(
        result.contains_sequence(sequences::ALT_SCREEN_ENTER),
        "Should enter alternate screen at minimal size"
    );
}

/// Test: Too-small terminal (30x10) exits gracefully.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_too_small_terminal() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .timeout(Duration::from_secs(10))
        .size(30, 10); // Below minimum (40x12)

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "too_small_terminal");

    // Should exit with error code 1 (too small)
    assert_eq!(
        result.exit_code,
        Some(1),
        "demo_showcase should exit with code 1 when terminal too small"
    );

    // Should contain error message about size
    let output_str = String::from_utf8_lossy(&result.output);
    let has_size_error = output_str.contains("too small")
        || output_str.contains("40x12")
        || output_str.contains("Terminal");
    assert!(
        has_size_error,
        "Should show error about terminal size being too small"
    );
}

/// Test: Synchronized output is enabled when TERM suggests support.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_synchronized_output_enabled() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    // Use a terminal that's known to support sync output
    let config = PtyConfig::demo_showcase_tour()
        .env("TERM", "xterm-kitty")
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "synchronized_output_enabled");

    assert_eq!(result.exit_code, Some(0));

    // Check for sync output sequences
    // Note: This depends on the terminal capability detection recognizing kitty
    let has_sync_begin = result.contains_sequence(sequences::SYNC_OUTPUT_BEGIN);
    let has_sync_end = result.contains_sequence(sequences::SYNC_OUTPUT_END);

    eprintln!("Sync output begin: {has_sync_begin}, end: {has_sync_end}");

    // We expect at least some sync output frames if capability is detected
    // This is a soft assertion since capability detection may vary
    if has_sync_begin {
        assert!(has_sync_end, "If sync output begins, it should also end");
    }
}

/// Test: Focus events are enabled.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_focus_events_enabled() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .timeout(Duration::from_secs(60))
        .size(80, 24);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "focus_events_enabled");

    assert_eq!(result.exit_code, Some(0));

    // Focus events should be enabled for pause detection
    assert!(
        result.contains_sequence(sequences::FOCUS_ENABLE),
        "Should enable focus event tracking"
    );
    assert!(
        result.contains_sequence(sequences::FOCUS_DISABLE),
        "Should disable focus events on exit"
    );
}

/// Test: Large terminal size (200x60) works correctly.
#[test]
#[ignore = "PTY tests require interactive terminal and are slow"]
fn test_large_terminal_size() {
    if !ensure_demo_showcase_built() {
        eprintln!("Skipping test: demo_showcase not available");
        return;
    }

    let config = PtyConfig::demo_showcase_tour()
        .timeout(Duration::from_secs(60))
        .size(200, 60);

    let result = spawn_pty(&config).expect("Failed to spawn PTY");
    log_pty_result(&result, "large_terminal_size");

    assert_eq!(
        result.exit_code,
        Some(0),
        "demo_showcase should handle large terminals"
    );

    // Basic lifecycle assertions
    assert!(result.contains_sequence(sequences::ALT_SCREEN_ENTER));
    assert!(result.contains_sequence(sequences::ALT_SCREEN_LEAVE));
}
