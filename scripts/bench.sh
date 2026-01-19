#!/usr/bin/env bash
# Benchmark runner for OpenTUI.
#
# Usage:
#   scripts/bench.sh                    # Run all benchmarks
#   scripts/bench.sh --quick            # Run quick comparison only (no criterion)
#   scripts/bench.sh --criterion        # Run criterion benchmarks only
#   scripts/bench.sh --compare          # Alias for quick comparison
#   scripts/bench.sh --json             # Output summary as JSON
#   scripts/bench.sh --save NAME        # Save baseline as NAME
#   scripts/bench.sh --baseline NAME    # Compare against baseline NAME
#
# Environment:
#   BENCH_TIMEOUT=600         Per-benchmark timeout in seconds (default: 300)
#   BENCH_CONFIRM=1           Skip confirmation for full benchmarks

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="${PROJECT_ROOT}/target/test-artifacts"
CRITERION_DIR="${PROJECT_ROOT}/target/criterion"

JSON_OUTPUT=0
QUICK_ONLY=0
CRITERION_ONLY=0
SAVE_BASELINE=""
USE_BASELINE=""
TIMEOUT="${BENCH_TIMEOUT:-300}"

log() {
  if [[ "$JSON_OUTPUT" -eq 0 ]]; then
    echo -e "\033[36m[bench]\033[0m $*"
  fi
}

error() {
  if [[ "$JSON_OUTPUT" -eq 0 ]]; then
    echo -e "\033[31m[bench] ERROR:\033[0m $*" >&2
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
    --quick)
      QUICK_ONLY=1
      shift
      ;;
    --criterion)
      CRITERION_ONLY=1
      shift
      ;;
    --compare)
      QUICK_ONLY=1
      shift
      ;;
    --save)
      SAVE_BASELINE="$2"
      shift 2
      ;;
    --baseline)
      USE_BASELINE="$2"
      shift 2
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

# Guard for long-running benchmarks unless quick-only.
if [[ -z "${BENCH_CONFIRM:-}" ]] && [[ "$JSON_OUTPUT" -eq 0 ]] && [[ "$QUICK_ONLY" -eq 0 ]]; then
  echo -e "\033[33m[bench] WARN:\033[0m Full benchmarks may take several minutes."
  echo -e "\033[33m[bench] WARN:\033[0m Set BENCH_CONFIRM=1 or use --quick for fast feedback."
  read -p "Continue? [y/N] " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log "Aborted."
    exit 0
  fi
fi

mkdir -p "$ARTIFACTS_DIR/benchmark"

START_TIME=$(date +%s)
RESULTS=()

run_quick_benchmark() {
  log "Running quick comparison benchmark..."
  if timeout "$TIMEOUT" cargo test --test benchmark_comparison -- --nocapture 2>&1 | tee "$ARTIFACTS_DIR/benchmark/quick_comparison.log"; then
    RESULTS+=("{\"benchmark\":\"quick_comparison\",\"result\":\"pass\"}")
    log "  Quick comparison: PASS"
  else
    RESULTS+=("{\"benchmark\":\"quick_comparison\",\"result\":\"fail\"}")
    error "  Quick comparison: FAIL"
  fi
}

run_criterion_benchmarks() {
  log "Running criterion benchmarks..."
  local baseline_args=""
  if [[ -n "$SAVE_BASELINE" ]]; then
    baseline_args="--save-baseline $SAVE_BASELINE"
    log "  Saving baseline as: $SAVE_BASELINE"
  elif [[ -n "$USE_BASELINE" ]]; then
    baseline_args="--baseline $USE_BASELINE"
    log "  Comparing against baseline: $USE_BASELINE"
  fi

  if timeout "$TIMEOUT" cargo bench $baseline_args 2>&1 | tee "$ARTIFACTS_DIR/benchmark/criterion.log"; then
    RESULTS+=("{\"benchmark\":\"criterion\",\"result\":\"pass\"}")
    log "  Criterion benchmarks: PASS"
  else
    RESULTS+=("{\"benchmark\":\"criterion\",\"result\":\"fail\"}")
    error "  Criterion benchmarks: FAIL"
  fi
}

if [[ "$QUICK_ONLY" -eq 1 ]]; then
  run_quick_benchmark
elif [[ "$CRITERION_ONLY" -eq 1 ]]; then
  run_criterion_benchmarks
else
  run_quick_benchmark
  run_criterion_benchmarks
fi

END_TIME=$(date +%s)
TOTAL_DURATION=$((END_TIME - START_TIME))

SUMMARY_FILE="$ARTIFACTS_DIR/benchmark_summary.json"
RESULTS_JSON=$(printf '%s\n' "${RESULTS[@]}" | paste -sd, -)

cat > "$SUMMARY_FILE" << EOF
{
  "suite": "benchmark",
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "duration_s": $TOTAL_DURATION,
  "baseline_saved": ${SAVE_BASELINE:+"\"$SAVE_BASELINE\""}${SAVE_BASELINE:-null},
  "baseline_used": ${USE_BASELINE:+"\"$USE_BASELINE\""}${USE_BASELINE:-null},
  "criterion_dir": "$CRITERION_DIR",
  "artifacts_dir": "$ARTIFACTS_DIR/benchmark",
  "results": [$RESULTS_JSON]
}
EOF

log "Summary written to: $SUMMARY_FILE"

if [[ "$JSON_OUTPUT" -eq 1 ]]; then
  cat "$SUMMARY_FILE"
fi

log "============================================"
log "Benchmark Results"
log "Duration: ${TOTAL_DURATION}s"
log "Criterion: $CRITERION_DIR"
log "Artifacts: $ARTIFACTS_DIR/benchmark"
log "============================================"

exit 0
