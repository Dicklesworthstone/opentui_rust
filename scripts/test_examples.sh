#!/usr/bin/env bash
set -euo pipefail

EXAMPLES_DIR="examples"
LOG_DIR="target"
LOG_FILE="${LOG_DIR}/examples_test.log"

mkdir -p "${LOG_DIR}"

echo "=== Testing Examples ===" | tee "${LOG_FILE}"
echo "Started at: $(date)" | tee -a "${LOG_FILE}"

failures=()

for example in "${EXAMPLES_DIR}"/*.rs; do
  name="$(basename "${example}" .rs)"
  echo -n "Testing ${name}... " | tee -a "${LOG_FILE}"
  if cargo build --all-features --example "${name}" 2>>"${LOG_FILE}"; then
    echo "COMPILED" | tee -a "${LOG_FILE}"
  else
    echo "COMPILE FAILED" | tee -a "${LOG_FILE}"
    failures+=("${name}")
  fi
done

echo "" | tee -a "${LOG_FILE}"
echo "=== Summary ===" | tee -a "${LOG_FILE}"
echo "Tested: $(ls "${EXAMPLES_DIR}"/*.rs | wc -l) examples" | tee -a "${LOG_FILE}"
echo "Failed: ${#failures[@]}" | tee -a "${LOG_FILE}"

if (( ${#failures[@]} > 0 )); then
  echo "Failures: ${failures[*]}" | tee -a "${LOG_FILE}"
  exit 1
fi

echo "All examples passed!" | tee -a "${LOG_FILE}"
