#!/usr/bin/env bash
# demo_showcase_verify.sh - Fast CI-like validation for demo_showcase
#
# Usage:
#   ./scripts/demo_showcase_verify.sh [--quick]
#
# Options:
#   --quick    Skip headless smoke test (faster for quick checks)
#
# Exit codes:
#   0 - All checks passed
#   1 - One or more checks failed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output (disabled if not a TTY)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED='' GREEN='' YELLOW='' BLUE='' NC=''
fi

QUICK_MODE=false
for arg in "$@"; do
    case "$arg" in
        --quick) QUICK_MODE=true ;;
        *) echo "Unknown option: $arg" >&2; exit 1 ;;
    esac
done

cd "$ROOT_DIR"

# Track failures
FAILURES=()

step() {
    echo -e "${BLUE}==>${NC} $1"
}

pass() {
    echo -e "${GREEN}  PASS${NC}: $1"
}

fail() {
    echo -e "${RED}  FAIL${NC}: $1"
    FAILURES+=("$1")
}

# === Step 1: Format Check ===
step "Checking code formatting..."
if cargo fmt --check >/dev/null 2>&1; then
    pass "cargo fmt"
else
    fail "cargo fmt (run 'cargo fmt' to fix)"
fi

# === Step 2: Clippy ===
step "Running clippy..."
if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | grep -q "error"; then
    fail "clippy (see above for errors)"
else
    pass "clippy"
fi

# === Step 3: Build ===
step "Building demo_showcase..."
if cargo build --bin demo_showcase --all-features 2>&1; then
    pass "build"
else
    fail "build"
fi

# === Step 4: Unit Tests ===
step "Running tests..."
if cargo test --all-features 2>&1; then
    pass "tests"
else
    fail "tests (see above for failures)"
fi

# === Step 5: demo_showcase Compile Test ===
step "Verifying demo_showcase compiles..."
if cargo test --test examples_compile demo_showcase_compiles --all-features 2>&1; then
    pass "demo_showcase compiles"
else
    fail "demo_showcase_compiles test"
fi

# === Step 6: Headless Smoke Test ===
if [[ "$QUICK_MODE" == "false" ]]; then
    step "Running headless smoke test..."
    if cargo test --test examples_compile demo_showcase_headless_smoke --all-features 2>&1; then
        pass "headless smoke test"
    else
        fail "headless smoke test"
    fi

    # === Step 7: Quick headless JSON dump ===
    step "Running headless JSON dump..."
    ARTIFACTS_DIR="target/test-artifacts/verify"
    mkdir -p "$ARTIFACTS_DIR"

    if cargo run --all-features --bin demo_showcase -- \
        --headless-smoke \
        --headless-dump-json \
        --max-frames 10 \
        > "$ARTIFACTS_DIR/headless_dump.json" 2>&1; then
        pass "headless JSON dump -> $ARTIFACTS_DIR/headless_dump.json"
    else
        fail "headless JSON dump"
    fi
else
    echo -e "${YELLOW}  SKIP${NC}: headless tests (--quick mode)"
fi

# === Summary ===
echo ""
echo "========================================"
if [[ ${#FAILURES[@]} -eq 0 ]]; then
    echo -e "${GREEN}All checks passed!${NC}"
    exit 0
else
    echo -e "${RED}${#FAILURES[@]} check(s) failed:${NC}"
    for f in "${FAILURES[@]}"; do
        echo "  - $f"
    done
    echo ""
    echo "Reproduction commands:"
    echo "  cargo fmt"
    echo "  cargo clippy --all-targets --all-features -- -D warnings"
    echo "  cargo test --all-features"
    exit 1
fi
