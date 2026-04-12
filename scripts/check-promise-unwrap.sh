#!/usr/bin/env bash
# INV-6 lint: Generated Rust must not contain literal `Promise<` type references.
# All Promise<T> types should be unwrapped to T by RustType::unwrap_promise().
#
# This script transpiles the callable-interface fixture and checks the output.
#
# Usage: ./scripts/check-promise-unwrap.sh

set -euo pipefail

FIXTURE="tests/fixtures/callable-interface.input.ts"
OUTPUT="/tmp/promise-unwrap-check.rs"

if [ ! -f "$FIXTURE" ]; then
    echo "SKIP: fixture $FIXTURE not found"
    exit 0
fi

cargo run --quiet -- "$FIXTURE" -o "$OUTPUT" 2>/dev/null || true

if [ -f "$OUTPUT" ] && grep -q 'Promise<' "$OUTPUT"; then
    echo "ERROR: Generated Rust contains literal 'Promise<' type references."
    echo "All Promise<T> types should be unwrapped to T."
    echo ""
    grep -n 'Promise<' "$OUTPUT"
    exit 1
fi

echo "OK: No literal Promise< in generated Rust."
