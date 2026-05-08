#!/usr/bin/env bash
# audit-no-init-call-site.sh — `init()` call site invariant CI check (I-224 INV-7)
#
# Verifies that no production code calls a free function `init()` after T4-1
# removed the `pub fn init` declaration mechanism. INV-4 already locks in 0
# declarations of `pub fn init`; INV-7 (= the **external API audit**) locks in
# the symmetric **call-site reachability** = 0 in production paths so the
# `pub fn init` 廃止 doesn't break any downstream caller.
#
# **Pre-Implementation Audit Findings (TS-7、PRD doc embed)**: codebase + Hono
# grep confirmed 0 hits at Spec stage. Post-T4 state is locked in by this
# audit + the INV-7 invariants test that subprocess-invokes it.
#
# Allowed exceptions:
#   - Doc comments / regular comments mentioning the historic mechanism (=
#     these are textual references, not call sites).
#   - Inline TypeScript-source string fixtures inside Rust string literals
#     (= test fixtures contain TS code, not Rust call sites).
#   - Method calls (`obj.init(...)`) and associated function calls
#     (`Type::init(...)`) — these target different scopes than the free
#     function `init()` that INV-7 enforces.
#
# Usage:
#   ./scripts/audit-no-init-call-site.sh
#
# Exit codes:
#   0 — OK (0 free-function `init()` call site hits in scoped paths)
#   1 — Violation detected; the script prints offending lines.
#
# I-224 Spec stage T5-2 artefact (= post-T4 state lock-in pair to INV-4).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Search paths that must NEVER contain a free-function `init()` call site
# (= the same enforcement scope as INV-4 / `audit-no-pub-fn-init.sh`).
declare -a SEARCH_PATHS=(
    "src/"
    "tools/"
    "tests/e2e/rust-runner/"
)

# Free-function `init()` call site pattern.
#
# **Match (= violation)**: `init(` not preceded by `.` (method call), `:`
# (associated call `Type::init`), or word character (`xinit(` is part of
# another identifier). The structural anchor pattern catches realistic call
# vectors at line scope:
#   `init();`            (statement form, line begins with `init(`)
#   `let x = init(...);` (assignment)
#   `= init(...)`        (RHS of assignment / let-binding)
#
# **Don't match (= legitimate / non-violation)**:
#   - Doc comment lines (`/// init()` 等): line starts with `///` so the
#     structural anchors `^\s*init\s*\(` etc. don't match.
#   - Single-line `// init()`: same as above (line starts with `//`).
#   - Method calls (`x.init(` / `Self::init(`): match position has `.` or `:`
#     immediately preceding, excluded by the negative anchor.
#   - Inline TS-source strings inside multi-line Rust string literals: the
#     match line doesn't end with a Rust call shape (= the line is part of a
#     string body), so the `init(` typically appears mid-line after non-Rust
#     text. The structural anchors `^\s*init\s*\(` etc. miss multi-line string
#     bodies that happen to start with `init(` after whitespace; INV-7's audit
#     accepts this small false-positive risk in exchange for filter
#     simplicity. The known instance (`this_dispatch.rs:349`) is documented
#     in the script's `KNOWN_FALSE_POSITIVES` array.
PATTERN='(^|[^a-zA-Z0-9_:.])init\s*\('

# Known false positives — TS-7 audit empirical inventory.
# Each entry = `<relative-file-path>:<line-fragment>`. The script accepts
# matches whose `<file>:<lineno>:<content>` line ends in any of these
# fragments (allowing line numbers to drift across edits).
declare -a KNOWN_FALSE_POSITIVES=(
    # Inline TS-source string inside a Rust multi-line string literal — TS
    # method definition `init(): void { ... }` appears inside the test fixture
    # for I-205 internal `this.x` dispatch. Not a Rust call site.
    # Match the unique content suffix (avoids hard-coding line numbers and
    # absolute paths so the filter is robust across edits).
    'init(): void { this.value ??= 42; } }";'
)

violations=0

for path in "${SEARCH_PATHS[@]}"; do
    abs_path="$PROJECT_ROOT/$path"
    if [[ ! -e "$abs_path" ]]; then
        continue
    fi
    # Use grep -P (Perl regex) for the alternation pattern. -h suppresses
    # filename prefix to allow per-line filtering, but we need filenames for
    # debugging; use -H + manual filter.
    if matches=$(grep -rPn --include='*.rs' "$PATTERN" "$abs_path" 2>/dev/null); then
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            # Extract content (after `<file>:<lineno>:`) and check for comment.
            # `grep -n` output format = `<file>:<lineno>:<content>`. We strip
            # the file:lineno prefix and inspect the raw content.
            content="${line#*:*:}"
            # Trim leading whitespace.
            content_trimmed="${content#"${content%%[![:space:]]*}"}"
            # Skip pure-comment lines (= `///` doc comment or `//` regular
            # comment as the entire content). The match is purely textual
            # documentation referring to the historic `pub fn init()`
            # mechanism — not a Rust call site.
            if [[ "$content_trimmed" == //* ]]; then
                continue
            fi
            # Skip known false positives (= multi-line string literals
            # containing TS-source fixtures).
            is_known=0
            for fp in "${KNOWN_FALSE_POSITIVES[@]}"; do
                if [[ "$line" == *"$fp"* ]]; then
                    is_known=1
                    break
                fi
            done
            if [[ $is_known -eq 1 ]]; then
                continue
            fi
            if [[ $violations -eq 0 ]]; then
                echo "VIOLATION (free-function \`init()\` call site detected):"
            fi
            echo "$line"
            violations=$((violations + 1))
        done <<< "$matches"
    fi
done

if [[ $violations -gt 0 ]]; then
    echo
    echo "FAIL: $violations free-function \`init()\` call site(s) found in enforced paths."
    echo "I-224 INV-7 requires 0 \`init()\` call sites after T4-1 removed the"
    echo "\`pub fn init\` mechanism (= external API audit reachability). Either:"
    echo "  (a) Replace the \`init()\` call with the new \`fn main\` synthesis"
    echo "      mechanism (= I-224 T2-T4 path), OR"
    echo "  (b) If this is a legitimate non-call-site reference (comment / TS-source"
    echo "      string fixture), add it to \`KNOWN_FALSE_POSITIVES\` in this script"
    echo "      and document the rationale."
    exit 1
fi

echo "OK: 0 free-function \`init()\` call sites in enforced paths (src/, tools/, tests/e2e/rust-runner/)."
exit 0
