#!/bin/bash
# Compare current performance against baseline
# Usage: ./scripts/benchmark_regression.sh [baseline_name]

set -euo pipefail

BASELINE="${1:-main}"
LOG_FILE="target/benchmark_regression.log"

echo "=== Performance Regression Check ===" | tee "$LOG_FILE"
echo "Comparing against baseline: $BASELINE" | tee -a "$LOG_FILE"
echo "Started at: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Run benchmarks with comparison
cargo bench -- --baseline "$BASELINE" 2>&1 | tee -a "$LOG_FILE"

# Check for regressions
REGRESSIONS=$(grep -c "regressed" "$LOG_FILE" 2>/dev/null || echo "0")
IMPROVEMENTS=$(grep -c "improved" "$LOG_FILE" 2>/dev/null || echo "0")

echo "" | tee -a "$LOG_FILE"
echo "=== Summary ===" | tee -a "$LOG_FILE"
echo "Regressions: $REGRESSIONS" | tee -a "$LOG_FILE"
echo "Improvements: $IMPROVEMENTS" | tee -a "$LOG_FILE"

if [ "$REGRESSIONS" -gt 0 ]; then
    echo "" | tee -a "$LOG_FILE"
    echo "=== REGRESSIONS DETECTED ===" | tee -a "$LOG_FILE"
    grep "regressed" "$LOG_FILE" | tee -a "$LOG_FILE"
    echo "" | tee -a "$LOG_FILE"
    echo "Review the changes carefully before merging." | tee -a "$LOG_FILE"
    # Note: We don't exit 1 here as some regressions may be acceptable
fi

if [ "$IMPROVEMENTS" -gt 0 ]; then
    echo "" | tee -a "$LOG_FILE"
    echo "=== IMPROVEMENTS ===" | tee -a "$LOG_FILE"
    grep "improved" "$LOG_FILE" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"
echo "Completed at: $(date)" | tee -a "$LOG_FILE"
