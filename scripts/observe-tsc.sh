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
ESM_MODE="auto"  # auto | force-esm
AUTO_MAIN="auto"  # auto | disabled
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
        --esm)
            # Force ESM runtime so top-level await works.
            # Achieved by writing package.json {"type":"module"} into the temp dir
            # before tsx execution.
            ESM_MODE="force-esm"
            shift
            ;;
        --no-auto-main)
            # Disable the auto-append of `main();` when fixture defines `function main`
            # but doesn't appear to invoke it. Spec stage oracle observation uses this
            # to preserve fidelity (declarations-only fixtures must observe as no-output).
            AUTO_MAIN="disabled"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--runtime-only|--type-check-only] [--esm] [--no-auto-main] <fixture.ts>"
            echo ""
            echo "Observes TypeScript fixture behavior via tsc (type check) and tsx (runtime)."
            echo "  --esm           Force ESM runtime (package.json type=module) so top-level await is allowed."
            echo "  --no-auto-main  Skip the legacy auto-append of main() when function main is defined."
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
    # Recognized invocation forms (any one suppresses auto-append):
    #   "main();"        — standalone sync call
    #   "await main();"  — awaited async call (top-level await context)
    # Auto-append is a legacy convenience for fixtures whose runtime entry point
    # is the user-defined `function main`. Spec-stage oracle observation passes
    # --no-auto-main to disable this entirely (preserving fidelity to source).
    if [[ "$AUTO_MAIN" != "disabled" ]] && \
       grep -qP '\bfunction main\b' "$tmpdir/input.ts" && \
       ! grep -qP '^\s*(await\s+)?main\(\)\s*;' "$tmpdir/input.ts"; then
        echo 'main();' >> "$tmpdir/input.ts"
    fi

    # ESM mode: place package.json {"type":"module"} so tsx accepts top-level await.
    # tsx default is cjs format which rejects top-level await with:
    #   "Top-level await is currently not supported with the 'cjs' output format"
    if [[ "$ESM_MODE" == "force-esm" ]]; then
        echo '{"type":"module"}' > "$tmpdir/package.json"
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
