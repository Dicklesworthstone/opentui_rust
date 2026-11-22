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
#   BENCH_CARGO=cargo         Override cargo executable (useful for testing/wrappers)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="${BENCH_ARTIFACTS_DIR:-${PROJECT_ROOT}/target/test-artifacts}"
CRITERION_DIR="${PROJECT_ROOT}/target/criterion"
CARGO_BIN="${BENCH_CARGO:-cargo}"

JSON_OUTPUT=0
QUICK_ONLY=0
CRITERION_ONLY=0
SAVE_BASELINE=""
USE_BASELINE=""
TIMEOUT="${BENCH_TIMEOUT:-300}"

if command -v timeout >/dev/null 2>&1; then
  TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
  TIMEOUT_BIN="gtimeout"
else
  echo "[bench] ERROR: Neither 'timeout' nor 'gtimeout' found in PATH." >&2
  exit 127
fi

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
  exit "${1:-0}"
}

json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  printf '%s' "$s"
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
      if [[ $# -lt 2 ]] || [[ -z "$2" ]]; then
        error "--save requires a baseline name"
        usage 2
      fi
      SAVE_BASELINE="$2"
      shift 2
      ;;
    --baseline)
      if [[ $# -lt 2 ]] || [[ -z "$2" ]]; then
        error "--baseline requires a baseline name"
        usage 2
      fi
      USE_BASELINE="$2"
      shift 2
      ;;
    --help|-h)
      usage 0
      ;;
    *)
      error "Unknown argument: $1"
      usage 2
      ;;
  esac
done

if [[ -n "$SAVE_BASELINE" ]] && [[ -n "$USE_BASELINE" ]]; then
  error "Cannot specify both --save and --baseline"
  exit 2
fi

if [[ "$QUICK_ONLY" -eq 1 ]] && [[ "$CRITERION_ONLY" -eq 1 ]]; then
  error "Cannot specify both --quick/--compare and --criterion"
  exit 2
fi

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
FAILED_COUNT=0
FINAL_EXIT_CODE=0

record_failure() {
  local rc="$1"
  FAILED_COUNT=$((FAILED_COUNT + 1))
  if [[ "$FINAL_EXIT_CODE" -eq 0 ]]; then
    FINAL_EXIT_CODE="$rc"
  fi
}

run_quick_benchmark() {
  log "Running quick comparison benchmark..."
  local log_file="$ARTIFACTS_DIR/benchmark/quick_comparison.log"
  local rc=0

  set +e
  if [[ "$JSON_OUTPUT" -eq 1 ]]; then
    "$TIMEOUT_BIN" "$TIMEOUT" "$CARGO_BIN" test --test benchmark_comparison -- --nocapture >"$log_file" 2>&1
    rc=$?
  else
    "$TIMEOUT_BIN" "$TIMEOUT" "$CARGO_BIN" test --test benchmark_comparison -- --nocapture 2>&1 | tee "$log_file"
    rc=${PIPESTATUS[0]}
  fi
  set -e

  if [[ "$rc" -eq 0 ]]; then
    RESULTS+=("{\"benchmark\":\"quick_comparison\",\"result\":\"pass\",\"exit_code\":0}")
    log "  Quick comparison: PASS"
  elif [[ "$rc" -eq 124 ]]; then
    RESULTS+=("{\"benchmark\":\"quick_comparison\",\"result\":\"timeout\",\"exit_code\":124}")
    error "  Quick comparison: TIMEOUT (exit 124 after ${TIMEOUT}s)"
    record_failure 124
  else
    RESULTS+=("{\"benchmark\":\"quick_comparison\",\"result\":\"fail\",\"exit_code\":${rc}}")
    error "  Quick comparison: FAIL (exit ${rc})"
    record_failure "$rc"
  fi
}

run_criterion_benchmarks() {
  log "Running criterion benchmarks..."
  local log_file="$ARTIFACTS_DIR/benchmark/criterion.log"
  local rc=0
  local -a cmd=("$CARGO_BIN" bench)

  if [[ -n "$SAVE_BASELINE" ]]; then
    cmd+=(-- --save-baseline "$SAVE_BASELINE")
    log "  Saving baseline as: $SAVE_BASELINE"
  elif [[ -n "$USE_BASELINE" ]]; then
    cmd+=(-- --baseline "$USE_BASELINE")
    log "  Comparing against baseline: $USE_BASELINE"
  fi

  set +e
  if [[ "$JSON_OUTPUT" -eq 1 ]]; then
    "$TIMEOUT_BIN" "$TIMEOUT" "${cmd[@]}" >"$log_file" 2>&1
    rc=$?
  else
    "$TIMEOUT_BIN" "$TIMEOUT" "${cmd[@]}" 2>&1 | tee "$log_file"
    rc=${PIPESTATUS[0]}
  fi
  set -e

  if [[ "$rc" -eq 124 ]]; then
    RESULTS+=("{\"benchmark\":\"criterion\",\"result\":\"timeout\",\"exit_code\":124}")
    error "  Criterion benchmarks: TIMEOUT (exit 124 after ${TIMEOUT}s)"
    record_failure 124
  elif [[ "$rc" -ne 0 ]]; then
    RESULTS+=("{\"benchmark\":\"criterion\",\"result\":\"fail\",\"exit_code\":${rc}}")
    error "  Criterion benchmarks: FAIL (exit ${rc})"
    record_failure "$rc"
  else
    local regressions=0
    if [[ -n "$USE_BASELINE" ]]; then
      regressions=$(grep -c "Performance has regressed" "$log_file" 2>/dev/null || true)
      regressions="${regressions:-0}"
    fi

    if [[ "$regressions" -gt 0 ]]; then
      RESULTS+=("{\"benchmark\":\"criterion\",\"result\":\"regressed\",\"exit_code\":1,\"regressions\":${regressions}}")
      error "  Criterion benchmarks: REGRESSED (${regressions} benchmark(s) regressed vs baseline '${USE_BASELINE}')"
      record_failure 1
    else
      RESULTS+=("{\"benchmark\":\"criterion\",\"result\":\"pass\",\"exit_code\":0}")
      log "  Criterion benchmarks: PASS"
    fi
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

OVERALL_STATUS="pass"
if [[ "$FAILED_COUNT" -gt 0 ]]; then
  OVERALL_STATUS="fail"
fi

SAVE_JSON="null"
if [[ -n "$SAVE_BASELINE" ]]; then
  SAVE_JSON="\"$(json_escape "$SAVE_BASELINE")\""
fi

USE_JSON="null"
if [[ -n "$USE_BASELINE" ]]; then
  USE_JSON="\"$(json_escape "$USE_BASELINE")\""
fi

SUMMARY_FILE="$ARTIFACTS_DIR/benchmark_summary.json"
RESULTS_JSON=$(printf '%s\n' "${RESULTS[@]}" | paste -sd, -)

cat > "$SUMMARY_FILE" << EOF
{
  "suite": "benchmark",
  "status": "$OVERALL_STATUS",
  "exit_code": $FINAL_EXIT_CODE,
  "failed_count": $FAILED_COUNT,
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "duration_s": $TOTAL_DURATION,
  "baseline_saved": $SAVE_JSON,
  "baseline_used": $USE_JSON,
  "criterion_dir": "$(json_escape "$CRITERION_DIR")",
  "artifacts_dir": "$(json_escape "$ARTIFACTS_DIR/benchmark")",
  "results": [$RESULTS_JSON]
}
EOF

log "Summary written to: $SUMMARY_FILE"

if [[ "$JSON_OUTPUT" -eq 1 ]]; then
  cat "$SUMMARY_FILE"
fi

log "============================================"
log "Benchmark Results"
log "Status: ${OVERALL_STATUS} (exit_code=${FINAL_EXIT_CODE}, failures=${FAILED_COUNT})"
log "Duration: ${TOTAL_DURATION}s"
log "Criterion: $CRITERION_DIR"
log "Artifacts: $ARTIFACTS_DIR/benchmark"
log "============================================"

exit "$FINAL_EXIT_CODE"
