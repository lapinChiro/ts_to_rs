#!/usr/bin/env bash
# INV-8 lint: Production code must not construct Transformer via struct literal.
# Only factory methods in src/transformer/mod.rs are allowed to construct Transformer {}.
# Test code (tests/ directories and #[cfg(test)] modules) is exempt.
#
# Usage: ./scripts/check-transformer-construction.sh

set -euo pipefail

# Search ALL production code (src/) for Transformer { struct construction.
# Exclude:
#   - src/transformer/mod.rs (factory method definitions live here)
#   - files under tests/ directories (test code is exempt)
#   - comment lines

violations=$(
    grep -rn 'Transformer\s*{' src/ \
        --include='*.rs' \
        --exclude-dir='tests' \
    | grep -v '^src/transformer/mod\.rs:' \
    | grep -v '^\s*//' \
    || true
)

if [ -n "$violations" ]; then
    echo "ERROR: Direct Transformer struct construction found in production code."
    echo "Use factory methods (for_module, spawn_nested_scope, spawn_nested_scope_with_local_synthetic) instead."
    echo ""
    echo "Violations:"
    echo "$violations"
    exit 1
fi

echo "OK: No direct Transformer construction in production code."
