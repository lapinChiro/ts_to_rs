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

    # Analyze and append to history
    python3 "$SCRIPT_DIR/analyze-bench.py" "$json_file" "$total" "$HONO_CLEAN"
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
