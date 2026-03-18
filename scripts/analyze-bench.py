#!/usr/bin/env python3
"""
Hono ベンチマーク結果の解析・履歴追記スクリプト。

hono-bench.sh から呼び出される。エラー JSON を解析し、
決まったスキーマで bench-history.jsonl に1行追記する。

使い方:
    python3 scripts/analyze-bench.py <errors.json> <total_files> <hono_clean_dir>
"""

import json
import sys
import subprocess
from collections import Counter
from datetime import datetime, timezone


def categorize(kind: str) -> str:
    """エラー kind 文字列をカテゴリに分類する。"""
    # 順序が重要: より具体的なパターンを先にチェック
    if "object literal requires" in kind:
        return "OBJECT_LITERAL_NO_TYPE"
    if "type alias body" in kind:
        return "TYPE_ALIAS_UNSUPPORTED"
    if ("Regex" in kind and "literal" in kind.lower()) or "Regex(Regex" in kind:
        return "REGEX_LITERAL"
    if "arrow" in kind and "default" in kind:
        return "ARROW_DEFAULT_PARAM"
    if "arrow parameter pattern" in kind:
        return "ARROW_PARAM_PATTERN"
    if "member property" in kind:
        return "MEMBER_PROPERTY"
    if "indexed access" in kind:
        return "INDEXED_ACCESS"
    if "intersection" in kind:
        return "INTERSECTION_TYPE"
    if "type in union" in kind:
        return "UNION_TYPE"
    # TsNonNull を Null より先にチェック (I-162: 誤分類防止)
    if "TsNonNull" in kind:
        return "TS_NON_NULL"
    if "Null" in kind:
        return "NULL_LITERAL"
    if "no type annotation" in kind:
        return "NO_TYPE_ANNOTATION"
    if "default parameter value" in kind or "default parameter requires" in kind:
        return "DEFAULT_PARAM_VALUE"
    if "binary operator" in kind:
        return "BINARY_OPERATOR"
    if "ForIn" in kind:
        return "FOR_IN_STMT"
    if "multiple declarators" in kind:
        return "FOR_MULTI_DECL"
    if "type literal member" in kind:
        return "TYPE_LITERAL_MEMBER"
    if "call target" in kind:
        return "CALL_TARGET"
    if "TsBigInt" in kind or "BigInt" in kind:
        return "BIGINT"
    if "TsModuleDecl" in kind:
        return "TS_MODULE_DECL"
    if "ExportAll" in kind:
        return "EXPORT_ALL"
    if "for...of binding" in kind:
        return "FOR_OF_BINDING"
    if "object destructuring" in kind:
        return "OBJ_DESTRUCT_NO_TYPE"
    if "call signature" in kind:
        return "CALL_SIGNATURE_PARAM"
    if "function type parameter" in kind:
        return "FN_TYPE_PARAM"
    if "object literal key" in kind or "object literal property" in kind:
        return "OBJECT_LITERAL_KEY"
    if "interface member" in kind:
        return "INTERFACE_MEMBER"
    if "TaggedTpl" in kind:
        return "TAGGED_TEMPLATE"
    if "compound assignment" in kind:
        return "COMPOUND_ASSIGN"
    if "TsUndefinedKeyword" in kind:
        return "UNDEFINED_KEYWORD"
    if "TsTypePredicate" in kind:
        return "TYPE_PREDICATE"
    if "TsSatisfies" in kind or "satisfies" in kind:
        return "SATISFIES_EXPR"
    if "TsTypeQuery" in kind:
        return "TYPEOF_TYPE"
    if "TsTypeOperator" in kind:
        return "TYPE_OPERATOR"
    if "assignment target" in kind:
        return "ASSIGN_TARGET"
    if "qualified type name" in kind:
        return "QUALIFIED_TYPE"
    if "SeqExpr" in kind:
        return "SEQ_EXPR"
    if "Empty" in kind:
        return "EMPTY_STMT"
    return "OTHER"


def get_git_sha() -> str:
    """現在の git SHA (short) を返す。"""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        return result.stdout.strip() if result.returncode == 0 else "unknown"
    except Exception:
        return "unknown"


def analyze(json_path: str, total_files: int, hono_clean_dir: str) -> dict:
    """エラー JSON を解析し、結果レコードを返す。"""
    with open(json_path) as f:
        content = f.read().strip()
        data = json.loads(content) if content else []

    files_with_errors: set[str] = set()
    categories: Counter[str] = Counter()

    for item in data:
        loc = item["location"]
        filename = loc.split(":")[0].replace(hono_clean_dir + "/", "")
        files_with_errors.add(filename)
        categories[categorize(item["kind"])] += 1

    clean_files = total_files - len(files_with_errors)

    return {
        "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "git_sha": get_git_sha(),
        "total_files": total_files,
        "clean_files": clean_files,
        "clean_pct": round(clean_files * 100 / total_files, 1) if total_files > 0 else 0,
        "error_instances": len(data),
        "categories": dict(categories.most_common()),
    }


def print_summary(record: dict) -> None:
    """解析結果を標準出力に表示する。"""
    print(f"=== DIRECTORY MODE (--report-unsupported) ===")
    print(f"Total files:       {record['total_files']}")
    print(
        f"Clean (0 errors):  {record['clean_files']} ({record['clean_pct']}%)"
    )
    print(
        f"With errors:       {record['total_files'] - record['clean_files']}"
        f" ({100 - record['clean_pct']}%)"
    )
    print(f"Error instances:   {record['error_instances']}")
    print()
    print("Error categories (instance count):")
    for cat, count in sorted(
        record["categories"].items(), key=lambda x: -x[1]
    ):
        print(f"  {count:4d}  {cat}")


def load_history(history_path: str) -> list[dict]:
    """bench-history.jsonl を読み込み、timestamp 順にソートして返す。"""
    entries: list[dict] = []
    try:
        with open(history_path) as f:
            for line in f:
                line = line.strip()
                if line:
                    entries.append(json.loads(line))
    except FileNotFoundError:
        pass
    entries.sort(key=lambda e: e.get("timestamp", ""))
    return entries


def print_diff(prev: dict, curr: dict) -> None:
    """前回エントリとの差分を表示する。"""
    print(f"--- Diff vs previous ({prev['git_sha']}, {prev['timestamp']}) ---")
    dc = curr["clean_files"] - prev["clean_files"]
    de = curr["error_instances"] - prev["error_instances"]
    dp = round(curr["clean_pct"] - prev["clean_pct"], 1)
    sign = lambda v: f"+{v}" if v > 0 else str(v)
    print(f"  Clean files:     {prev['clean_files']} → {curr['clean_files']} ({sign(dc)})")
    print(f"  Clean %:         {prev['clean_pct']}% → {curr['clean_pct']}% ({sign(dp)}pp)")
    print(f"  Error instances: {prev['error_instances']} → {curr['error_instances']} ({sign(de)})")

    # カテゴリ別の変動
    prev_cats = prev.get("categories", {})
    curr_cats = curr.get("categories", {})
    all_cats = sorted(set(prev_cats) | set(curr_cats))
    changes = []
    for cat in all_cats:
        p = prev_cats.get(cat, 0)
        c = curr_cats.get(cat, 0)
        if p != c:
            changes.append((c - p, cat, p, c))
    if changes:
        changes.sort(key=lambda x: x[0])
        print("  Category changes:")
        for delta, cat, p, c in changes:
            print(f"    {sign(delta):>4s}  {cat} ({p} → {c})")


def main() -> None:
    if len(sys.argv) != 4:
        print(
            f"Usage: {sys.argv[0]} <errors.json> <total_files> <hono_clean_dir>",
            file=sys.stderr,
        )
        sys.exit(1)

    json_path = sys.argv[1]
    total_files = int(sys.argv[2])
    hono_clean_dir = sys.argv[3]

    record = analyze(json_path, total_files, hono_clean_dir)

    # bench-history.jsonl のパスを特定
    repo_root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
    ).stdout.strip()
    history_path = f"{repo_root}/bench-history.jsonl"

    # 前回エントリを取得（timestamp 順で最新）
    history = load_history(history_path)

    # 標準出力に表示
    print_summary(record)

    # 前回との差分を表示
    if history:
        print()
        print_diff(history[-1], record)

    # bench-history.jsonl に追記
    with open(history_path, "a") as f:
        f.write(json.dumps(record, ensure_ascii=False) + "\n")

    print()
    print(f"Result appended to {history_path}")


if __name__ == "__main__":
    main()
