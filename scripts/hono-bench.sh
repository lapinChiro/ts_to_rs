#!/usr/bin/env bash
#
# Hono 変換率ベンチマーク
#
# 使い方:
#   ./scripts/hono-bench.sh          # ディレクトリモード（主指標）
#   ./scripts/hono-bench.sh --single # 単一ファイルモード（参考値）
#   ./scripts/hono-bench.sh --both   # 両モード比較
#
# 前提:
#   - cargo build --release 済み
#   - /tmp/hono-src/ に Hono リポジトリがクローン済み
#     なければ自動で git clone する

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$PROJECT_DIR/target/release/ts_to_rs"
HONO_SRC="/tmp/hono-src"
HONO_CLEAN="/tmp/hono-clean"

# --- Setup ---

ensure_hono_src() {
    if [ ! -d "$HONO_SRC/src" ]; then
        echo "Cloning Hono repository..."
        git clone --depth 1 https://github.com/honojs/hono.git "$HONO_SRC" 2>&1 | tail -1
    fi
}

ensure_binary() {
    if [ ! -f "$BINARY" ]; then
        echo "Building release binary..."
        (cd "$PROJECT_DIR" && cargo build --release 2>&1 | tail -1)
    fi
}

prepare_clean_copy() {
    rm -rf "$HONO_CLEAN"
    mkdir -p "$HONO_CLEAN"
    cd "$HONO_SRC/src"
    find . -name '*.ts' \
        -not -name '*.d.ts' \
        -not -name '*.test.ts' \
        -not -name '*.spec.ts' \
        -not -path '*/jsx/*' \
        | while IFS= read -r f; do
            mkdir -p "$HONO_CLEAN/$(dirname "$f")"
            cp "$f" "$HONO_CLEAN/$f"
        done
    cd "$PROJECT_DIR"
}

# --- Measurement ---

run_directory_mode() {
    local output_dir="/tmp/hono-bench-output"
    rm -rf "$output_dir"

    local total
    total=$(find "$HONO_CLEAN" -name '*.ts' | wc -l)

    # Run with --report-unsupported to collect all errors
    local json_file="/tmp/hono-bench-errors.json"
    "$BINARY" --report-unsupported "$HONO_CLEAN/" -o "$output_dir/" > "$json_file" 2>/dev/null

    # Compile check on clean files
    local compile_clean
    compile_clean=$(run_compile_check "$json_file" "$output_dir")

    # Analyze and append to history
    python3 "$SCRIPT_DIR/analyze-bench.py" "$json_file" "$total" "$HONO_CLEAN" "$compile_clean"
}

run_compile_check() {
    local error_json="$1"
    local output_dir="$2"
    local check_dir="/tmp/hono-compile-check"

    # Create Cargo project from template
    rm -rf "$check_dir"
    mkdir -p "$check_dir/src"
    cp "$PROJECT_DIR/tests/compile-check/Cargo.toml" "$check_dir/Cargo.toml"

    # Identify clean .rs files and copy with flattened names
    local total_clean
    total_clean=$(python3 -c "
import json, os, sys, re

error_json = '$error_json'
hono_clean = '$HONO_CLEAN'
output_dir = '$output_dir'
check_dir = '$check_dir'

with open(error_json) as f:
    data = json.loads(f.read().strip() or '[]')

files_with_errors = set()
for item in data:
    loc = item['location']
    filename = loc.split(':')[0].replace(hono_clean + '/', '')
    files_with_errors.add(filename)

mod_decls = []
count = 0
for root, dirs, files in os.walk(hono_clean):
    dirs.sort()
    for fname in sorted(files):
        if not fname.endswith('.ts'):
            continue
        rel = os.path.relpath(os.path.join(root, fname), hono_clean)
        if rel in files_with_errors:
            continue
        rs_rel = rel.replace('.ts', '.rs').replace('-', '_')
        rs_path = os.path.join(output_dir, rs_rel)
        if not os.path.exists(rs_path):
            continue
        # Flatten path to module name
        mod_name = rs_rel.replace('.rs', '').replace('/', '_').replace('-', '_')
        # Strip internal use statements
        with open(rs_path) as rf:
            content = rf.read()
        lines = [l for l in content.splitlines()
                 if not (l.strip().startswith('use crate::') or l.strip().startswith('use super::')
                         or l.strip().startswith('pub use crate::') or l.strip().startswith('pub use super::'))]
        with open(os.path.join(check_dir, 'src', mod_name + '.rs'), 'w') as wf:
            wf.write('\n'.join(lines))
        mod_decls.append(f'mod {mod_name};')
        count += 1

# Write lib.rs
with open(os.path.join(check_dir, 'src', 'lib.rs'), 'w') as f:
    f.write('#![allow(unused, dead_code, unreachable_code, non_snake_case, non_camel_case_types)]\\n')
    f.write('use serde::{Serialize, Deserialize};\\n')
    f.write('\\n'.join(mod_decls) + '\\n')

print(count)
" 2>/dev/null)

    if [ "$total_clean" = "0" ] || [ -z "$total_clean" ]; then
        echo "0"
        return
    fi

    # Run cargo check, parse which modules had errors
    # Use subshell to avoid pipefail propagating cargo check's non-zero exit
    local cargo_json="/tmp/hono-compile-check-output.json"
    (cd "$check_dir" && cargo check --message-format=json 2>/dev/null || true) > "$cargo_json"

    local compile_clean
    compile_clean=$(python3 -c "
import json, sys

total_clean = $total_clean
error_mods = set()
with open('$cargo_json') as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
            if msg.get('reason') == 'compiler-message':
                level = msg.get('message', {}).get('level', '')
                if level != 'error':
                    continue
                for span in msg.get('message', {}).get('spans', []):
                    fname = span.get('file_name', '')
                    if fname.startswith('src/') and fname != 'src/lib.rs':
                        mod_name = fname.replace('src/', '').replace('.rs', '')
                        error_mods.add(mod_name)
        except json.JSONDecodeError:
            pass

print(total_clean - len(error_mods))
" 2>/dev/null)

    echo "${compile_clean:-0}"
}

run_single_file_mode() {
    local total=0
    local success=0
    local fail=0

    while IFS= read -r f; do
        total=$((total + 1))
        err=$("$BINARY" "$f" 2>&1 | grep "Caused by:" -A1 | tail -1 | sed 's/^[[:space:]]*//')
        if [ -z "$err" ]; then
            success=$((success + 1))
        else
            fail=$((fail + 1))
        fi
    done < <(find "$HONO_CLEAN" -name '*.ts' | sort)

    echo "=== SINGLE FILE MODE ==="
    echo "Total files:  $total"
    echo "Success:      $success ($((success * 100 / total))%)"
    echo "Fail:         $fail ($((fail * 100 / total))%)"
}

# --- Main ---

MODE="${1:-}"

ensure_hono_src
ensure_binary
prepare_clean_copy

case "$MODE" in
    --single)
        run_single_file_mode
        ;;
    --both)
        run_directory_mode
        echo ""
        run_single_file_mode
        ;;
    *)
        run_directory_mode
        ;;
esac
