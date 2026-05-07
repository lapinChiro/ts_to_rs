#!/usr/bin/env bash
# audit-no-pub-fn-init.sh — `pub fn init` mechanism 廃止 invariant CI check (I-224 INV-4)
#
# Verifies that the `pub fn init` identifier does NOT appear in production code,
# transformer logic, or test infrastructure (Rust source files only).
# This locks in the `pub fn init mechanism 廃止` structural fix from PRD I-224.
#
# Allowed exceptions:
#   - Backlog / PRD doc files (`backlog/*.md`) — historical references in spec.
#   - Generated e2e snapshot artefacts (`tests/e2e/scripts/i-205/cell-*.rs`) —
#     these are regenerated as `fn main` after I-224 T5; the audit checks the
#     source-of-truth `src/` tree and rejects new generators.
#
# Usage:
#   ./scripts/audit-no-pub-fn-init.sh
#
# Exit codes:
#   0 — OK (0 hits in scoped paths)
#   1 — Violation detected (hit found in `src/`, `tools/`, or non-snapshot test
#       infrastructure); the script prints offending lines.
#
# I-224 Spec stage TS-7 artefact, T6a で CI integrate 予定。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Search paths whose contents must NEVER contain `pub fn init`.
# Production source + transformer logic + non-snapshot test code.
declare -a SEARCH_PATHS=(
    "src/"
    "tools/"
    "tests/e2e/rust-runner/"
)

# Optional broader scan (warning only, does not fail the audit).
# `tests/e2e/scripts/i-205/cell-*.rs` are regenerated snapshots; flag them as
# advisory so re-running the e2e suite refreshes the snapshots and clears them.
declare -a ADVISORY_PATHS=(
    "tests/e2e/scripts/"
)

# `pub fn init` declaration pattern.
#
# **Match**: actual Rust function definitions of the legacy `pub fn init` shape:
#   `pub fn init(...)` (sync params)
#   `pub fn init<T>(...)` (generic params)
# **Don't match**: doc comments, panic / format strings, or other prose mentions
#   of the historic mechanism — these are textual references to the design's
#   evolution and have no runtime impact on the converted binary.
#
# Implementation: anchor the identifier to start-of-line whitespace (rejects
# lines that have any non-whitespace prefix, including `//`, `///`, `//!`, or
# the leading `"`/`'` of a string literal) AND require an opening parenthesis
# or angle bracket after `init` (= the syntactic shape of a function header).
# This is the empirical lock-in for INV-4: only true function definitions are
# rejected; doc commentary that names the mechanism stays free to discuss it.
PATTERN='^\s*pub\s+fn\s+init\s*[\(<]'

violations=0

for path in "${SEARCH_PATHS[@]}"; do
    abs_path="$PROJECT_ROOT/$path"
    if [[ ! -e "$abs_path" ]]; then
        continue
    fi
    # `grep -rEn` with extended regex; suppress error if no matches.
    if matches=$(grep -rEn --include='*.rs' "$PATTERN" "$abs_path" 2>/dev/null); then
        echo "VIOLATION (forbidden path): $path"
        echo "$matches"
        violations=$((violations + 1))
    fi
done

# Advisory scan — only printed, never fails.
advisory_hits=0
for path in "${ADVISORY_PATHS[@]}"; do
    abs_path="$PROJECT_ROOT/$path"
    if [[ ! -e "$abs_path" ]]; then
        continue
    fi
    if matches=$(grep -rEn --include='*.rs' "$PATTERN" "$abs_path" 2>/dev/null); then
        if [[ $advisory_hits -eq 0 ]]; then
            echo "ADVISORY (snapshot artefacts, regenerate to clear):"
        fi
        echo "$matches"
        advisory_hits=$((advisory_hits + 1))
    fi
done

if [[ $violations -gt 0 ]]; then
    echo
    echo "FAIL: \`pub fn init\` found in $violations enforced path(s)."
    echo "I-224 INV-4 requires the \`pub fn init\` mechanism to be fully removed"
    echo "from production code. Replace with the \`fn main\` synthesis introduced"
    echo "by I-224 T2-T4."
    exit 1
fi

echo "OK: 0 hits of \`pub fn init\` in enforced paths (src/, tools/, tests/e2e/rust-runner/)."
if [[ $advisory_hits -gt 0 ]]; then
    echo "Note: $advisory_hits advisory hit(s) found in snapshot artefacts;"
    echo "      run the e2e suite to regenerate them under the new \`fn main\` mechanism."
fi
exit 0
