#!/usr/bin/env bash
# Compare current performance against baseline and fail on benchmark errors or regressions.
# Usage: ./scripts/benchmark_regression.sh [baseline_name]
#
# Environment:
#   BENCH_LOG_FILE=target/benchmark_regression.log  Override output log path
#   BENCH_CARGO=cargo                               Override cargo executable

set -euo pipefail

BASELINE="${1:-main}"
LOG_FILE="${BENCH_LOG_FILE:-target/benchmark_regression.log}"
CARGO_BIN="${BENCH_CARGO:-cargo}"
BENCH_OUT_FILE="${LOG_FILE}.raw"

mkdir -p "$(dirname "$LOG_FILE")"

echo "=== Performance Regression Check ===" | tee "$LOG_FILE"
echo "Comparing against baseline: $BASELINE" | tee -a "$LOG_FILE"
echo "Started at: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Run benchmarks with comparison, preserving cargo's exact exit code.
set +e
"$CARGO_BIN" bench -- --baseline "$BASELINE" 2>&1 | tee "$BENCH_OUT_FILE" | tee -a "$LOG_FILE"
BENCH_EXIT=${PIPESTATUS[0]}
set -e

# Count Criterion regression/improvement verdicts from benchmark output only.
REGRESSIONS=$(grep -c "regressed" "$BENCH_OUT_FILE" 2>/dev/null || true)
REGRESSIONS="${REGRESSIONS:-0}"
IMPROVEMENTS=$(grep -c "improved" "$BENCH_OUT_FILE" 2>/dev/null || true)
IMPROVEMENTS="${IMPROVEMENTS:-0}"

echo "" | tee -a "$LOG_FILE"
echo "=== Summary ===" | tee -a "$LOG_FILE"
echo "Benchmark exit code: $BENCH_EXIT" | tee -a "$LOG_FILE"
echo "Regressions: $REGRESSIONS" | tee -a "$LOG_FILE"
echo "Improvements: $IMPROVEMENTS" | tee -a "$LOG_FILE"

if [ "$REGRESSIONS" -gt 0 ]; then
    echo "" | tee -a "$LOG_FILE"
    echo "=== REGRESSIONS DETECTED ===" | tee -a "$LOG_FILE"
    grep "regressed" "$BENCH_OUT_FILE" | tee -a "$LOG_FILE"
    echo "" | tee -a "$LOG_FILE"
    echo "Review the changes carefully before merging." | tee -a "$LOG_FILE"
fi

if [ "$IMPROVEMENTS" -gt 0 ]; then
    echo "" | tee -a "$LOG_FILE"
    echo "=== IMPROVEMENTS ===" | tee -a "$LOG_FILE"
    grep "improved" "$BENCH_OUT_FILE" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"
echo "Completed at: $(date)" | tee -a "$LOG_FILE"

if [ "$BENCH_EXIT" -ne 0 ]; then
    exit "$BENCH_EXIT"
fi

if [ "$REGRESSIONS" -gt 0 ]; then
    exit 1
fi

exit 0
