#!/usr/bin/env bash
# src/ 配下の .rs ファイルが指定行数以下であることを検証する。
# 閾値を超えるファイルがあれば一覧を出力し、終了コード 1 で終了する。
#
# Usage: ./scripts/check-file-lines.sh [threshold]
#   threshold: 最大行数（デフォルト: 1000）

set -euo pipefail

THRESHOLD="${1:-1000}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

violations=()

while IFS= read -r line; do
    count="${line%% *}"
    file="${line#* }"
    if [ "$count" -gt "$THRESHOLD" ]; then
        violations+=("$line")
    fi
done < <(find "$PROJECT_ROOT/src" -name '*.rs' -exec wc -l {} + | grep -v ' total$' | sed 's/^ *//' | sort -rn)

if [ "${#violations[@]}" -eq 0 ]; then
    echo "OK: All .rs files are within ${THRESHOLD} lines."
    exit 0
else
    echo "FAIL: ${#violations[@]} file(s) exceed ${THRESHOLD} lines:"
    for v in "${violations[@]}"; do
        echo "  $v"
    done
    exit 1
fi
