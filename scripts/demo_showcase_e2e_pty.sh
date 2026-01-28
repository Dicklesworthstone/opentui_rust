#!/usr/bin/env bash
# demo_showcase_e2e_pty.sh - Run PTY E2E tests with artifact collection
#
# Usage:
#   ./scripts/demo_showcase_e2e_pty.sh [TEST_FILTER]
#
# Arguments:
#   TEST_FILTER    Optional filter for specific tests (e.g., "test_tour_mode")
#
# Environment:
#   HARNESS_ARTIFACTS_DIR  Override artifact output directory
#                          (default: target/test-artifacts)
#
# This script runs the ignored PTY E2E tests which spawn demo_showcase under
# a real pseudo-terminal to verify actual ANSI output sequences.
#
# Note: These tests are marked #[ignore] because they:
# - Require a TTY-like environment
# - Are slower than unit tests
# - May be flaky in CI environments
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output (disabled if not a TTY)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    NC='\033[0m' # No Color
else
    RED='' GREEN='' YELLOW='' BLUE='' CYAN='' NC=''
fi

# Parse arguments
TEST_FILTER="${1:-}"

cd "$ROOT_DIR"

# Configure artifacts
export HARNESS_ARTIFACTS=1
export HARNESS_ARTIFACTS_DIR="${HARNESS_ARTIFACTS_DIR:-target/test-artifacts/pty}"

echo -e "${BLUE}====================================${NC}"
echo -e "${BLUE}demo_showcase PTY E2E Test Runner${NC}"
echo -e "${BLUE}====================================${NC}"
echo ""
echo "Artifact directory: $HARNESS_ARTIFACTS_DIR"
echo ""

# Create artifact directory
mkdir -p "$HARNESS_ARTIFACTS_DIR"

# === Step 1: Build demo_showcase ===
echo -e "${CYAN}==>${NC} Building demo_showcase..."
if ! cargo build --bin demo_showcase --all-features 2>&1; then
    echo -e "${RED}FAIL${NC}: Failed to build demo_showcase"
    exit 1
fi
echo -e "${GREEN}OK${NC}: demo_showcase built"
echo ""

# === Step 2: Build PTY tests ===
echo -e "${CYAN}==>${NC} Building PTY tests..."
if ! cargo build --test pty_e2e --all-features 2>&1; then
    echo -e "${RED}FAIL${NC}: Failed to build PTY tests"
    exit 1
fi
echo -e "${GREEN}OK${NC}: PTY tests built"
echo ""

# === Step 3: Run PTY tests ===
echo -e "${CYAN}==>${NC} Running PTY E2E tests (ignored by default)..."
echo ""

# Build test command
TEST_CMD=(cargo test --test pty_e2e --all-features -- --ignored --nocapture)
if [[ -n "$TEST_FILTER" ]]; then
    TEST_CMD+=("$TEST_FILTER")
    echo "Filter: $TEST_FILTER"
fi

echo "Command: ${TEST_CMD[*]}"
echo ""
echo -e "${YELLOW}--- Test Output ---${NC}"

START_TIME=$(date +%s)

# Run tests and capture exit code
set +e
"${TEST_CMD[@]}"
TEST_EXIT_CODE=$?
set -e

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo -e "${YELLOW}--- End Test Output ---${NC}"
echo ""

# === Summary ===
echo -e "${BLUE}====================================${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}====================================${NC}"
echo "Duration: ${DURATION}s"
echo "Artifacts: $HARNESS_ARTIFACTS_DIR"
echo ""

# List artifacts if any were created
if [[ -d "$HARNESS_ARTIFACTS_DIR" ]]; then
    ARTIFACT_COUNT=$(find "$HARNESS_ARTIFACTS_DIR" -type f 2>/dev/null | wc -l)
    if [[ "$ARTIFACT_COUNT" -gt 0 ]]; then
        echo "Collected artifacts:"
        find "$HARNESS_ARTIFACTS_DIR" -type f -name "*.txt" -o -name "*.bin" -o -name "*.hex" 2>/dev/null | head -20 | while read -r f; do
            SIZE=$(du -h "$f" | cut -f1)
            echo "  - ${f#$ROOT_DIR/} ($SIZE)"
        done
        if [[ "$ARTIFACT_COUNT" -gt 20 ]]; then
            echo "  ... and $((ARTIFACT_COUNT - 20)) more"
        fi
        echo ""
    fi
fi

if [[ $TEST_EXIT_CODE -eq 0 ]]; then
    echo -e "${GREEN}All PTY E2E tests passed!${NC}"
else
    echo -e "${RED}Some PTY E2E tests failed.${NC}"
    echo ""
    echo "Debug tips:"
    echo "  - Check artifacts in: $HARNESS_ARTIFACTS_DIR"
    echo "  - Look at output.txt for readable ANSI sequences"
    echo "  - Look at output.hex for raw byte inspection"
    echo "  - Run specific test: $0 test_name"
fi

exit $TEST_EXIT_CODE
