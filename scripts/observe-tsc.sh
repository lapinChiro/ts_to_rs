#!/usr/bin/env bash
# observe-tsc.sh — tsc / tsx で TS fixture の挙動を観測する helper
#
# Usage:
#   ./scripts/observe-tsc.sh <fixture.ts>
#   ./scripts/observe-tsc.sh --runtime-only <fixture.ts>
#   ./scripts/observe-tsc.sh --type-check-only <fixture.ts>
#
# Output: JSON to stdout with the following structure:
#   {
#     "fixture": "<path>",
#     "type_check": { "exit_code": N, "stdout": "...", "stderr": "...", "errors": [...] },
#     "runtime": { "exit_code": N, "stdout": "...", "stderr": "..." }
#   }
#
# Prerequisites:
#   - tsc: available via tools/extract-types/node_modules/.bin/tsc
#   - tsx: available via npx tsx (or globally installed)
#
# SDCDF Phase 1 artifact (I-SDCDF)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TSC_BIN="$PROJECT_ROOT/tools/extract-types/node_modules/.bin/tsc"

# --- argument parsing ---
MODE="both"  # both | runtime-only | type-check-only
FIXTURE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --runtime-only)
            MODE="runtime-only"
            shift
            ;;
        --type-check-only)
            MODE="type-check-only"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--runtime-only|--type-check-only] <fixture.ts>"
            echo ""
            echo "Observes TypeScript fixture behavior via tsc (type check) and tsx (runtime)."
            echo "Output: JSON to stdout."
            exit 0
            ;;
        *)
            FIXTURE="$1"
            shift
            ;;
    esac
done

if [[ -z "$FIXTURE" ]]; then
    echo "Error: fixture path required" >&2
    echo "Usage: $0 [--runtime-only|--type-check-only] <fixture.ts>" >&2
    exit 1
fi

if [[ ! -f "$FIXTURE" ]]; then
    echo "Error: fixture not found: $FIXTURE" >&2
    exit 1
fi

# --- helpers ---
json_escape() {
    # Escape string for JSON embedding
    python3 -c "import json,sys; print(json.dumps(sys.stdin.read()), end='')"
}

parse_tsc_errors() {
    # Parse tsc error output into JSON array of error objects
    python3 -c "
import sys, json, re

errors = []
for line in sys.stdin:
    line = line.rstrip()
    # Match pattern: file(line,col): error TSxxxx: message
    m = re.match(r'^(.+)\((\d+),(\d+)\):\s+error\s+(TS\d+):\s+(.+)$', line)
    if m:
        errors.append({
            'file': m.group(1),
            'line': int(m.group(2)),
            'col': int(m.group(3)),
            'code': m.group(4),
            'message': m.group(5)
        })
print(json.dumps(errors))
"
}

# --- type check ---
run_type_check() {
    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf $tmpdir" RETURN

    # Create minimal tsconfig for strict checking
    cat > "$tmpdir/tsconfig.json" <<'TSCONFIG'
{
  "compilerOptions": {
    "strict": true,
    "strictNullChecks": true,
    "noEmit": true,
    "target": "ES2022",
    "module": "ES2022",
    "moduleResolution": "node",
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["input.ts"]
}
TSCONFIG

    cp "$FIXTURE" "$tmpdir/input.ts"

    # Single invocation: capture stdout, stderr, and exit code in one run
    local tsc_exit=0
    "$TSC_BIN" --project "$tmpdir/tsconfig.json" \
        >"$tmpdir/stdout.txt" 2>"$tmpdir/stderr.txt" || tsc_exit=$?

    local escaped_stdout escaped_stderr errors_json
    escaped_stdout=$(json_escape < "$tmpdir/stdout.txt")
    escaped_stderr=$(json_escape < "$tmpdir/stderr.txt")
    errors_json=$(parse_tsc_errors < "$tmpdir/stdout.txt")

    echo "{\"exit_code\":$tsc_exit,\"stdout\":$escaped_stdout,\"stderr\":$escaped_stderr,\"errors\":$errors_json}"
}

# --- runtime ---
run_runtime() {
    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf $tmpdir" RETURN

    # Copy fixture and add main() invocation if needed
    cp "$FIXTURE" "$tmpdir/input.ts"

    # Check if fixture defines a main() function that isn't already invoked.
    # Match standalone invocation "main();" — not the declaration "function main()".
    # The pattern requires main() at statement level (possibly indented), with semicolon.
    if grep -qP '\bfunction main\b' "$tmpdir/input.ts" && \
       ! grep -qP '^\s*main\(\);' "$tmpdir/input.ts"; then
        echo 'main();' >> "$tmpdir/input.ts"
    fi

    # Single invocation: capture stdout, stderr, and exit code in one run
    local runtime_exit=0
    npx tsx "$tmpdir/input.ts" \
        >"$tmpdir/stdout.txt" 2>"$tmpdir/stderr.txt" || runtime_exit=$?

    local escaped_stdout escaped_stderr
    escaped_stdout=$(json_escape < "$tmpdir/stdout.txt")
    escaped_stderr=$(json_escape < "$tmpdir/stderr.txt")

    echo "{\"exit_code\":$runtime_exit,\"stdout\":$escaped_stdout,\"stderr\":$escaped_stderr}"
}

# --- main ---
FIXTURE_ABS=$(realpath "$FIXTURE")
ESCAPED_FIXTURE=$(echo -n "$FIXTURE_ABS" | json_escape)

echo "{"
echo "  \"fixture\": $ESCAPED_FIXTURE,"

case "$MODE" in
    both)
        TC_JSON=$(run_type_check)
        RT_JSON=$(run_runtime)
        echo "  \"type_check\": $TC_JSON,"
        echo "  \"runtime\": $RT_JSON"
        ;;
    type-check-only)
        TC_JSON=$(run_type_check)
        echo "  \"type_check\": $TC_JSON"
        ;;
    runtime-only)
        RT_JSON=$(run_runtime)
        echo "  \"runtime\": $RT_JSON"
        ;;
esac

echo "}"
