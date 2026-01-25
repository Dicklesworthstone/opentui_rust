#!/bin/bash
# Generate and store performance baselines
# Usage: ./scripts/benchmark_baseline.sh [baseline_name]

set -euo pipefail

BASELINE_NAME="${1:-$(git rev-parse --short HEAD)}"
BASELINE_DIR="target/criterion/baselines"
LOG_FILE="target/benchmark_${BASELINE_NAME}.log"

mkdir -p "$BASELINE_DIR"

echo "=== Generating Performance Baseline: $BASELINE_NAME ===" | tee "$LOG_FILE"
echo "Started at: $(date)" | tee -a "$LOG_FILE"
echo "Git commit: $(git rev-parse HEAD)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Build benchmarks first
echo "Building benchmarks..." | tee -a "$LOG_FILE"
cargo build --benches 2>&1 | tee -a "$LOG_FILE"

# Run full benchmark suite and save as baseline
echo "" | tee -a "$LOG_FILE"
echo "Running benchmarks..." | tee -a "$LOG_FILE"
cargo bench -- --save-baseline "$BASELINE_NAME" 2>&1 | tee -a "$LOG_FILE"

# Save the baseline directory
if [ -d "target/criterion" ]; then
    cp -r target/criterion "$BASELINE_DIR/$BASELINE_NAME"
    echo "" | tee -a "$LOG_FILE"
    echo "=== Baseline saved to: $BASELINE_DIR/$BASELINE_NAME ===" | tee -a "$LOG_FILE"
fi

echo "Completed at: $(date)" | tee -a "$LOG_FILE"
