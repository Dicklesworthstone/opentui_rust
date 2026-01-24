#!/usr/bin/env bash
# Conformance test runner for OpenTUI.
#
# Usage:
#   scripts/conformance.sh                  # Run conformance tests
#   scripts/conformance.sh --json           # Output summary as JSON
#   scripts/conformance.sh --verbose        # Show test output
#   scripts/conformance.sh --filter PATTERN # Run only tests matching PATTERN
#   scripts/conformance.sh --check-fixtures # Verify fixtures exist
#
# Environment:
#   HARNESS_ARTIFACTS=1        Enable artifact logging to target/test-artifacts/
#   HARNESS_PRESERVE_SUCCESS=1 Keep artifacts even on success
#   CONFORMANCE_TIMEOUT=180    Per-test timeout in seconds (default: 120)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="${PROJECT_ROOT}/target/test-artifacts"

CONFORMANCE_TESTS=(
  conformance
)

JSON_OUTPUT=0
VERBOSE=0
FILTER=""
CHECK_FIXTURES=0
TIMEOUT="${CONFORMANCE_TIMEOUT:-120}"

log() {
  if [[ "$JSON_OUTPUT" -eq 0 ]]; then
    echo -e "\033[35m[conformance]\033[0m $*"
  fi
}

error() {
  if [[ "$JSON_OUTPUT" -eq 0 ]]; then
    echo -e "\033[31m[conformance] ERROR:\033[0m $*" >&2
  fi
}

usage() {
  head -n 25 "$0" | tail -n +2 | sed 's/^# //' | sed 's/^#//'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --json)
      JSON_OUTPUT=1
      shift
      ;;
    --verbose|-v)
      VERBOSE=1
      shift
      ;;
    --filter)
      FILTER="$2"
      shift 2
      ;;
    --check-fixtures)
      CHECK_FIXTURES=1
      shift
      ;;
    --help|-h)
      usage
      ;;
    *)
      error "Unknown argument: $1"
      usage
      ;;
  esac
done

if [[ "$CHECK_FIXTURES" -eq 1 ]]; then
  FIXTURE_PATH="$PROJECT_ROOT/tests/conformance/fixtures/opentui.json"
  if [[ ! -f "$FIXTURE_PATH" ]]; then
    error "Conformance fixtures not found: $FIXTURE_PATH"
    exit 2
  fi
  log "Found fixtures: $FIXTURE_PATH"
  exit 0
fi

mkdir -p "$ARTIFACTS_DIR/conformance"

PASSED=0
FAILED=0
SKIPPED=0
START_TIME=$(date +%s)
RESULTS=()

log "Running conformance tests (${#CONFORMANCE_TESTS[@]} test files)..."
log "Artifacts: $ARTIFACTS_DIR/conformance"

for test in "${CONFORMANCE_TESTS[@]}"; do
  if [[ -n "$FILTER" ]] && [[ ! "$test" =~ $FILTER ]]; then
    SKIPPED=$((SKIPPED + 1))
    continue
  fi

  log "  Running: $test"
  TEST_START=$(date +%s.%N)

  NOCAPTURE_FLAG=""
  if [[ "$VERBOSE" -eq 1 ]]; then
    NOCAPTURE_FLAG="--nocapture"
  fi

  if timeout "$TIMEOUT" cargo test --test "$test" -- $NOCAPTURE_FLAG 2>&1; then
    RESULT="pass"
    PASSED=$((PASSED + 1))
  else
    RESULT="fail"
    FAILED=$((FAILED + 1))
  fi

  TEST_END=$(date +%s.%N)
  TEST_DURATION=$(echo "$TEST_END - $TEST_START" | bc 2>/dev/null || echo "0")
  RESULTS+=("{\"test\":\"$test\",\"result\":\"$RESULT\",\"duration_s\":$TEST_DURATION}")

  if [[ "$RESULT" == "pass" ]]; then
    log "    PASS (${TEST_DURATION}s)"
  else
    error "    FAIL (${TEST_DURATION}s)"
  fi
done

END_TIME=$(date +%s)
TOTAL_DURATION=$((END_TIME - START_TIME))

SUMMARY_FILE="$ARTIFACTS_DIR/conformance_summary.json"
RESULTS_JSON=$(printf '%s\n' "${RESULTS[@]}" | paste -sd, -)

cat > "$SUMMARY_FILE" << EOF
{
  "suite": "conformance",
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "passed": $PASSED,
  "failed": $FAILED,
  "skipped": $SKIPPED,
  "total": $((PASSED + FAILED)),
  "duration_s": $TOTAL_DURATION,
  "artifacts_dir": "$ARTIFACTS_DIR/conformance",
  "results": [$RESULTS_JSON]
}
EOF

log "Summary written to: $SUMMARY_FILE"

if [[ "$JSON_OUTPUT" -eq 1 ]]; then
  cat "$SUMMARY_FILE"
fi

log "============================================"
log "Conformance Results: $PASSED passed, $FAILED failed, $SKIPPED skipped"
log "Duration: ${TOTAL_DURATION}s"
log "Artifacts: $ARTIFACTS_DIR/conformance"
log "============================================"

if [[ "$FAILED" -gt 0 ]]; then
  exit 1
fi

exit 0
