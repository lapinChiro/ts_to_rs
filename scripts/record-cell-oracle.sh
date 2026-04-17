#!/usr/bin/env bash
# record-cell-oracle.sh — cell fixture の tsc runtime stdout を expected output として記録
#
# Usage:
#   ./scripts/record-cell-oracle.sh <cell-fixture.ts>
#   ./scripts/record-cell-oracle.sh tests/e2e/scripts/sdcdf-smoke/let-init-string-lit.ts
#   ./scripts/record-cell-oracle.sh --all <prd-dir>
#   ./scripts/record-cell-oracle.sh --all tests/e2e/scripts/sdcdf-smoke/
#
# For a single fixture:
#   1. Runs observe-tsc.sh --runtime-only to get TS runtime stdout
#   2. Writes stdout to <fixture>.expected alongside the .ts file
#
# For --all:
#   Processes all .ts files in the given directory
#
# .expected files are git-tracked oracle records for PRD spec-stage review.
# They serve as human-readable reference of what TS runtime produces.
# The automated E2E runner (e2e_test.rs) uses live TS execution for comparison,
# not .expected files — this ensures tests reflect current TS behavior rather
# than potentially stale oracle snapshots.
#
# SDCDF Phase 2 artifact (I-SDCDF)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OBSERVE_TSC="$SCRIPT_DIR/observe-tsc.sh"

record_one() {
    local fixture="$1"
    local expected_path="${fixture%.ts}.expected"

    if [[ ! -f "$fixture" ]]; then
        echo "Error: fixture not found: $fixture" >&2
        return 1
    fi

    # Extract runtime stdout from observe-tsc.sh JSON output
    local stdout
    stdout=$("$OBSERVE_TSC" --runtime-only "$fixture" | \
        python3 -c "import sys,json; print(json.load(sys.stdin)['runtime']['stdout'], end='')")

    echo -n "$stdout" > "$expected_path"

    local basename
    basename=$(basename "$fixture")
    local lines
    lines=$(echo "$stdout" | wc -l)
    echo "Recorded: $basename → $(basename "$expected_path") ($lines lines)"
}

# --- argument parsing ---
if [[ $# -eq 0 ]]; then
    echo "Usage: $0 <cell-fixture.ts>"
    echo "       $0 --all <prd-dir>"
    exit 1
fi

if [[ "$1" == "--all" ]]; then
    if [[ $# -lt 2 ]]; then
        echo "Error: --all requires a directory path" >&2
        exit 1
    fi
    prd_dir="$2"
    if [[ ! -d "$prd_dir" ]]; then
        echo "Error: directory not found: $prd_dir" >&2
        exit 1
    fi

    count=0
    for ts_file in "$prd_dir"/*.ts; do
        [[ -f "$ts_file" ]] || continue
        record_one "$ts_file"
        count=$((count + 1))
    done

    echo "---"
    echo "Recorded $count oracle(s) in $prd_dir"
elif [[ "$1" == "-h" ]] || [[ "$1" == "--help" ]]; then
    echo "Usage: $0 <cell-fixture.ts>"
    echo "       $0 --all <prd-dir>"
    echo ""
    echo "Records TS runtime stdout as .expected files for per-cell E2E oracle comparison."
    exit 0
else
    record_one "$1"
fi
