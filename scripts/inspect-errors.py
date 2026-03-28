#!/usr/bin/env python3
"""
Hono ベンチマークエラーの対話的検査スクリプト。

/tmp/hono-bench-errors.json を解析し、カテゴリ別集計・フィルタ・
ソース行表示などを行う。analyze-bench.py がベンチマーク実行時の
自動集計を担うのに対し、本スクリプトは開発者が手動で詳細分析する
ためのツール。

使い方:
    python3 scripts/inspect-errors.py                    # カテゴリ別集計
    python3 scripts/inspect-errors.py --kind TYPEOF      # kind でフィルタ (部分一致)
    python3 scripts/inspect-errors.py --category TYPEOF_TYPE  # カテゴリでフィルタ
    python3 scripts/inspect-errors.py --file client      # ファイル名でフィルタ (部分一致)
    python3 scripts/inspect-errors.py --source           # エラー箇所の TS ソースを表示
    python3 scripts/inspect-errors.py --discriminant     # Discriminant エラーの AST ノード種を推定
    python3 scripts/inspect-errors.py --raw              # フィルタ後のエラーを JSON で出力
"""

import argparse
import json
import os
import re
import sys
from collections import Counter
from pathlib import Path

from bench_categories import categorize

DEFAULT_ERRORS_PATH = "/tmp/hono-bench-errors.json"
HONO_SRC_DIR = "/tmp/hono-src/src"
HONO_CLEAN_DIR = "/tmp/hono-clean"

# SWC TsType enum variant order (swc_ecma_ast).
# std::mem::discriminant returns sequential indices matching definition order.
# Based on swc_ecma_ast 5.x; verify if SWC version changes.
TS_TYPE_VARIANTS = {
    0: "TsKeywordType",
    1: "TsThisType",
    2: "TsFnOrConstructorType",
    3: "TsTypeRef",
    4: "TsTypeQuery",
    5: "TsTypeLit",
    6: "TsArrayType",
    7: "TsTupleType",
    8: "TsOptionalType",
    9: "TsRestType",
    10: "TsUnionOrIntersectionType",
    11: "TsConditionalType",
    12: "TsInferType",
    13: "TsParenthesizedType",
    14: "TsTypeOperator",
    15: "TsIndexedAccessType",
    16: "TsMappedType",
    17: "TsLitType",
    18: "TsTypePredicate",
    19: "TsImportType",
}


def parse_location(loc: str) -> tuple[str, int, int]:
    """location 文字列を (file, line, col) に分割する。"""
    # "/tmp/hono-clean/client/types.ts:28:1"
    parts = loc.rsplit(":", 2)
    if len(parts) == 3:
        return parts[0], int(parts[1]), int(parts[2])
    return loc, 0, 0


def relative_path(filepath: str) -> str:
    """hono-clean ディレクトリからの相対パスを返す。"""
    for prefix in [HONO_CLEAN_DIR + "/", HONO_SRC_DIR + "/"]:
        if filepath.startswith(prefix):
            return filepath[len(prefix):]
    return filepath


def resolve_discriminant(kind: str) -> str | None:
    """'Discriminant(N)' から AST ノード種を推定す��。"""
    m = re.search(r"Discriminant\((\d+)\)", kind)
    if m:
        idx = int(m.group(1))
        return TS_TYPE_VARIANTS.get(idx)
    return None


def read_source_lines(filepath: str, line: int, context: int = 2) -> list[str]:
    """TS ソースファイルの指定行周辺を読む。"""
    # hono-clean パスから hono-src パスに変換
    rel = relative_path(filepath)
    src_path = os.path.join(HONO_SRC_DIR, rel)
    if not os.path.exists(src_path):
        return [f"  (source not found: {src_path})"]
    lines = []
    with open(src_path) as f:
        all_lines = f.readlines()
    start = max(0, line - 1 - context)
    end = min(len(all_lines), line + context)
    for i in range(start, end):
        marker = ">>>" if i == line - 1 else "   "
        lines.append(f"  {marker} {i+1:4d} | {all_lines[i].rstrip()}")
    return lines


def load_errors(path: str) -> list[dict]:
    """エラー JSON を読み込む。"""
    with open(path) as f:
        content = f.read().strip()
        return json.loads(content) if content else []


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Hono ベンチマークエラーの対話的検査"
    )
    parser.add_argument(
        "--errors", default=DEFAULT_ERRORS_PATH, help="エラー JSON パス"
    )
    parser.add_argument("--kind", help="kind での部分一致フィルタ")
    parser.add_argument("--category", help="カテゴリ名での完全一致フィルタ")
    parser.add_argument("--file", help="ファイル名での部分一致フィルタ")
    parser.add_argument(
        "--source", action="store_true", help="エラー箇所の TS ソースを表示"
    )
    parser.add_argument(
        "--discriminant",
        action="store_true",
        help="Discriminant エラーの AST ノード種を推定表示",
    )
    parser.add_argument(
        "--raw", action="store_true", help="フィルタ後のエラーを JSON で出力"
    )
    args = parser.parse_args()

    errors = load_errors(args.errors)
    if not errors:
        print("No errors found.", file=sys.stderr)
        sys.exit(0)

    # フィルタ適用
    filtered = errors
    if args.kind:
        filtered = [e for e in filtered if args.kind.lower() in e["kind"].lower()]
    if args.category:
        filtered = [e for e in filtered if categorize(e["kind"]) == args.category]
    if args.file:
        filtered = [
            e for e in filtered if args.file.lower() in e["location"].lower()
        ]

    if args.raw:
        print(json.dumps(filtered, indent=2, ensure_ascii=False))
        return

    if not filtered:
        print("No matching errors.", file=sys.stderr)
        sys.exit(0)

    # Discriminant 解析モード
    if args.discriminant:
        disc_errors = [e for e in filtered if "Discriminant" in e["kind"]]
        if not disc_errors:
            print("No Discriminant errors in filtered results.")
            return
        by_disc: dict[str, list[dict]] = {}
        for e in disc_errors:
            ast_node = resolve_discriminant(e["kind"]) or "Unknown"
            key = f"{e['kind']} → {ast_node}"
            by_disc.setdefault(key, []).append(e)
        for key, items in sorted(by_disc.items(), key=lambda x: -len(x[1])):
            print(f"\n{len(items):3d}  {key}")
            for e in items:
                filepath, line, col = parse_location(e["location"])
                print(f"       {relative_path(filepath)}:{line}:{col}")
                if args.source:
                    for sl in read_source_lines(filepath, line):
                        print(sl)
        return

    # カテゴリ別集計（デフォルト）
    if not args.kind and not args.category and not args.file:
        cats = Counter(categorize(e["kind"]) for e in filtered)
        print(f"Total: {len(filtered)} errors\n")
        print("Category breakdown:")
        for cat, count in cats.most_common():
            print(f"  {count:4d}  {cat}")
        print()

    # 詳細表示
    for e in filtered:
        filepath, line, col = parse_location(e["location"])
        rel = relative_path(filepath)
        cat = categorize(e["kind"])
        print(f"[{cat}] {rel}:{line}:{col}")
        # kind が長い場合は省略表示
        kind_display = e["kind"]
        if len(kind_display) > 120:
            kind_display = kind_display[:117] + "..."
        print(f"  kind: {kind_display}")
        if args.source:
            for sl in read_source_lines(filepath, line):
                print(sl)
        print()


if __name__ == "__main__":
    main()
