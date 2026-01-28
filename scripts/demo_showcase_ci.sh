#!/usr/bin/env bash
# demo_showcase_ci.sh - CI-style headless demo smoke tests
#
# This script runs the same headless demo tests that run in CI, useful for:
# - Pre-commit validation
# - Debugging CI failures locally
# - Verifying demo_showcase works across different terminal sizes
#
# Usage:
#   ./scripts/demo_showcase_ci.sh [OPTIONS]
#
# Options:
#   --quick       Only run the 80x24 (standard) size
#   --size SIZE   Run only the specified size (e.g., 80x24, 132x43)
#   --verbose     Show full JSON output
#   --help        Show this help message
#
# Exit codes:
#   0 - All smoke tests passed
#   1 - One or more tests failed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    NC='\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' CYAN='' NC=''
fi

# Default sizes (matches CI matrix)
SIZES=("80x24" "132x43" "40x12" "200x60")
SIZE_DESCRIPTIONS=(
    "Standard terminal"
    "Wide terminal"
    "Minimal terminal"
    "Large terminal"
)

QUICK_MODE=false
VERBOSE=false
SINGLE_SIZE=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --size)
            SINGLE_SIZE="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            head -25 "$0" | tail -n +2 | sed 's/^# //' | sed 's/^#//'
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}" >&2
            exit 1
            ;;
    esac
done

cd "$ROOT_DIR"

# Create artifacts directory
ARTIFACTS_DIR="target/demo-smoke-local"
mkdir -p "$ARTIFACTS_DIR"

# Build demo_showcase
echo -e "${BLUE}==>${NC} Building demo_showcase..."
if ! cargo build --release --bin demo_showcase 2>&1; then
    echo -e "${RED}Failed to build demo_showcase${NC}"
    exit 1
fi
echo -e "${GREEN}  Build complete${NC}"

# Filter sizes based on options
if [[ -n "$SINGLE_SIZE" ]]; then
    SIZES=("$SINGLE_SIZE")
    SIZE_DESCRIPTIONS=("Custom size")
elif [[ "$QUICK_MODE" == "true" ]]; then
    SIZES=("80x24")
    SIZE_DESCRIPTIONS=("Standard terminal")
fi

# Run smoke tests
FAILURES=()
PASSED=0

echo ""
echo -e "${BLUE}==>${NC} Running headless smoke tests..."
echo ""

for i in "${!SIZES[@]}"; do
    SIZE="${SIZES[$i]}"
    DESC="${SIZE_DESCRIPTIONS[$i]:-$SIZE}"
    OUTPUT_FILE="$ARTIFACTS_DIR/${SIZE}.json"

    echo -e "${CYAN}  Testing ${SIZE}${NC} ($DESC)..."

    STDERR_FILE="$ARTIFACTS_DIR/${SIZE}.stderr.log"

    # Run the binary directly (already built above)
    # Redirect stdout to JSON file, stderr to separate log to avoid corrupting JSON
    if ./target/release/demo_showcase \
        --headless-smoke \
        --tour \
        --exit-after-tour \
        --headless-size "$SIZE" \
        --seed 12345 \
        --max-frames 1000 \
        --headless-dump-json > "$OUTPUT_FILE" 2>"$STDERR_FILE"; then

        # Validate JSON
        if jq empty "$OUTPUT_FILE" 2>/dev/null; then
            FRAMES=$(jq -r '.frames_rendered // 0' "$OUTPUT_FILE")
            LAYOUT=$(jq -r '.layout_mode // "unknown"' "$OUTPUT_FILE")
            TOUR_COMPLETED=$(jq -r '.tour_state.completed // false' "$OUTPUT_FILE")

            echo -e "${GREEN}    ✓ PASS${NC} - ${FRAMES} frames, layout: ${LAYOUT}, tour: ${TOUR_COMPLETED}"

            if [[ "$VERBOSE" == "true" ]]; then
                echo -e "${YELLOW}    JSON output:${NC}"
                jq '{ frames_rendered, layout_mode, tour_state: { completed, total_steps } }' "$OUTPUT_FILE" | sed 's/^/      /'
            fi

            PASSED=$((PASSED + 1))
        else
            echo -e "${RED}    ✗ FAIL${NC} - Invalid JSON output"
            FAILURES+=("$SIZE: Invalid JSON")
        fi
    else
        echo -e "${RED}    ✗ FAIL${NC} - Process failed"
        FAILURES+=("$SIZE: Process failed")

        if [[ "$VERBOSE" == "true" ]]; then
            if [[ -f "$STDERR_FILE" ]] && [[ -s "$STDERR_FILE" ]]; then
                echo -e "${YELLOW}    Stderr:${NC}"
                head -20 "$STDERR_FILE" | sed 's/^/      /'
            fi
            if [[ -f "$OUTPUT_FILE" ]] && [[ -s "$OUTPUT_FILE" ]]; then
                echo -e "${YELLOW}    Stdout:${NC}"
                head -20 "$OUTPUT_FILE" | sed 's/^/      /'
            fi
        fi
    fi
done

# Summary
echo ""
echo "========================================"

if [[ ${#FAILURES[@]} -eq 0 ]]; then
    echo -e "${GREEN}All ${PASSED} smoke test(s) passed!${NC}"
    echo ""
    echo "Artifacts saved to: $ARTIFACTS_DIR/"
    exit 0
else
    echo -e "${RED}${#FAILURES[@]} smoke test(s) failed:${NC}"
    for f in "${FAILURES[@]}"; do
        echo "  - $f"
    done
    echo ""
    echo -e "${GREEN}${PASSED} passed${NC}, ${RED}${#FAILURES[@]} failed${NC}"
    echo ""
    echo "Artifacts saved to: $ARTIFACTS_DIR/"
    exit 1
fi
