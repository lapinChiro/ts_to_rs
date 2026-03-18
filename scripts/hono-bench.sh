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
        -not -path '*/types*' \
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

    # Parse JSON with python3
    python3 -c "
import json, sys, os
from collections import Counter

with open('$json_file') as f:
    content = f.read().strip()
    data = json.loads(content) if content else []

files_with_errors = set()
error_kinds = Counter()

for item in data:
    loc = item['location']
    filename = loc.split(':')[0].replace('$HONO_CLEAN/', '')
    files_with_errors.add(filename)

    kind = item['kind']
    # Categorize
    if 'object literal requires' in kind: cat = 'OBJECT_LITERAL_NO_TYPE'
    elif 'type alias body' in kind: cat = 'TYPE_ALIAS_UNSUPPORTED'
    elif 'Regex' in kind and 'literal' in kind.lower() or 'Regex(Regex' in kind: cat = 'REGEX_LITERAL'
    elif 'arrow' in kind and 'default' in kind: cat = 'ARROW_DEFAULT_PARAM'
    elif 'arrow parameter pattern' in kind: cat = 'ARROW_PARAM_PATTERN'
    elif 'member property' in kind: cat = 'MEMBER_PROPERTY'
    elif 'indexed access' in kind: cat = 'INDEXED_ACCESS'
    elif 'intersection' in kind: cat = 'INTERSECTION_TYPE'
    elif 'type in union' in kind: cat = 'UNION_TYPE'
    elif 'Null' in kind: cat = 'NULL_LITERAL'
    elif 'no type annotation' in kind: cat = 'NO_TYPE_ANNOTATION'
    elif 'default parameter value' in kind or 'default parameter requires' in kind: cat = 'DEFAULT_PARAM_VALUE'
    elif 'binary operator' in kind: cat = 'BINARY_OPERATOR'
    elif 'ForIn' in kind: cat = 'FOR_IN_STMT'
    elif 'multiple declarators' in kind: cat = 'FOR_MULTI_DECL'
    elif 'type literal member' in kind: cat = 'TYPE_LITERAL_MEMBER'
    elif 'call target' in kind: cat = 'CALL_TARGET'
    elif 'TsBigInt' in kind or 'BigInt' in kind: cat = 'BIGINT'
    elif 'TsModuleDecl' in kind: cat = 'TS_MODULE_DECL'
    elif 'ExportAll' in kind: cat = 'EXPORT_ALL'
    elif 'for...of binding' in kind: cat = 'FOR_OF_BINDING'
    elif 'object destructuring' in kind: cat = 'OBJ_DESTRUCT_NO_TYPE'
    elif 'call signature' in kind: cat = 'CALL_SIGNATURE_PARAM'
    elif 'function type parameter' in kind: cat = 'FN_TYPE_PARAM'
    elif 'object literal key' in kind: cat = 'OBJECT_LITERAL_KEY'
    elif 'interface member' in kind: cat = 'INTERFACE_MEMBER'
    elif 'TaggedTpl' in kind: cat = 'TAGGED_TEMPLATE'
    elif 'compound assignment' in kind: cat = 'COMPOUND_ASSIGN'
    elif 'TsUndefinedKeyword' in kind: cat = 'UNDEFINED_KEYWORD'
    elif 'TsTypePredicate' in kind: cat = 'TYPE_PREDICATE'
    elif 'Empty' in kind: cat = 'EMPTY_STMT'
    else: cat = 'OTHER'
    error_kinds[cat] += 1

total = $total
clean = total - len(files_with_errors)

print(f'=== DIRECTORY MODE (--report-unsupported) ===')
print(f'Total files:       {total}')
print(f'Clean (0 errors):  {clean} ({clean*100//total}%)')
print(f'With errors:       {len(files_with_errors)} ({len(files_with_errors)*100//total}%)')
print(f'Error instances:   {len(data)}')
print()
print('Error categories (instance count):')
for cat, count in error_kinds.most_common():
    print(f'  {count:4d}  {cat}')
" 2>&1
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
