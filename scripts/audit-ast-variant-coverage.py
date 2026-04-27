#!/usr/bin/env python3
"""PRD 2.7 — AST variant coverage audit (Q4: Rule 11 d-1〜d-4 compliance).

ast-variants.md (single source of truth) と ts_to_rs codebase の AST match 文の
sync を verify する。`spec-stage-adversarial-checklist.md` Rule 11 (AST node
enumerate completeness check) compliance audit。

Approach: tree-sitter-rust ベース AST parse (PRD 2.7 M1 修正 確定 approach、
高 precision)。

Usage:
    python3 scripts/audit-ast-variant-coverage.py [--verbose] [--codebase-wide]

Options:
    --verbose: 各 enum の Tier classification + code arm 一覧を stderr に出力
    --codebase-wide: PRD 2.7 scope (ClassMember / PropOrSpread / Prop) に加え、
                     codebase 全体の `_` arm を I-203 用 detection report として出力

Exit code:
    0: audit pass (doc-code sync OK)
    非 0: audit fail (sync 違反 detected)

Dependencies:
    pip install tree-sitter tree-sitter-rust
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

import tree_sitter_rust
from tree_sitter import Language, Node, Parser

REPO_ROOT = Path(__file__).resolve().parent.parent
AST_VARIANTS_MD = REPO_ROOT / "doc" / "grammar" / "ast-variants.md"
SRC_DIR = REPO_ROOT / "src"

# 本 PRD 2.7 scope の audit 対象 enum (Q4 application)
# 既存 codebase 全体の `_` arm refactor は I-203 で別 PRD
PRD_2_7_SCOPE_ENUMS = ("ClassMember", "PropOrSpread", "Prop")

# 本 PRD 2.7 scope 内で改修される file (T8 + T9 + T10)
# scope 外 file の violation は I-203 用 detection report として出力 (exit code 0)
PRD_2_7_SCOPE_FILES = frozenset(
    {
        "src/pipeline/type_resolver/visitors.rs",
        "src/pipeline/type_resolver/expressions.rs",
        "src/transformer/expressions/data_literals.rs",
    }
)

# Tier 2 memo 内 keyword: 含むと "no-op が ideal" (filter out / no-op variant)、
# 含まないと "honest error が ideal" (UnsupportedSyntaxError 経由)
NO_OP_KEYWORDS = ("filter out", "no-op", "no_op", "filter-out")

RUST_LANGUAGE = Language(tree_sitter_rust.language())


@dataclass
class TierClassification:
    """ast-variants.md の 1 enum section の Tier 分類。

    `tier2_no_op` は Tier 2 のうち memo 列に "filter out" / "no-op" 等 keyword を
    含む variant (= no-op 実装が ideal、honest error 不要)。
    `tier2_honest_error` は memo に該当 keyword なし (= UnsupportedSyntaxError 経由
    honest error report 必須)。
    """

    tier1: set[str] = field(default_factory=set)
    tier2_honest_error: set[str] = field(default_factory=set)
    tier2_no_op: set[str] = field(default_factory=set)
    na: set[str] = field(default_factory=set)

    @property
    def tier2(self) -> set[str]:
        return self.tier2_honest_error | self.tier2_no_op


@dataclass
class MatchArm:
    """1 つの match_arm node の解析結果。"""

    file: str
    enum_name: str  # 例: "ClassMember"、wildcard arm では enclosing match の context
    variant_name: str  # 例: "StaticBlock"、"_" は wildcard
    body_text: str
    line: int
    enclosing_scrutinee: str = ""  # wildcard arm の enclosing match_expression scrutinee 文字列


def parse_ast_variants_md() -> dict[str, TierClassification]:
    """ast-variants.md の全 enum section から Tier 1 / Tier 2 / NA variant を抽出。"""
    content = AST_VARIANTS_MD.read_text(encoding="utf-8")

    # `## NN. EnumName (...)` heading で sections 分割
    section_pattern = re.compile(r"^## \d+\.\s+(\w+)\s*\(", re.MULTILINE)
    sections: dict[str, str] = {}
    matches = list(section_pattern.finditer(content))
    for i, m in enumerate(matches):
        enum_name = m.group(1)
        start = m.start()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(content)
        sections[enum_name] = content[start:end]

    enum_classifications: dict[str, TierClassification] = {}
    for enum_name, section_content in sections.items():
        tiers = TierClassification()
        # `### Tier 1 — ...` / `### Tier 2 — ...` / `### NA — ...` で sub-section 分割
        subsection_pattern = re.compile(
            r"^### (Tier 1|Tier 2|NA)\b[^\n]*", re.MULTILINE
        )
        sub_matches = list(subsection_pattern.finditer(section_content))
        for j, sm in enumerate(sub_matches):
            tier_label = sm.group(1)
            sub_start = sm.end()
            sub_end = (
                sub_matches[j + 1].start()
                if j + 1 < len(sub_matches)
                else len(section_content)
            )
            sub_content = section_content[sub_start:sub_end]
            # markdown table を行単位で parse、variant + memo 全 cell を抽出
            # 形式: `| \`Variant\` | memo |` または `| \`Variant\` | Status | 備考 |`
            row_pattern = re.compile(r"^\|\s*`(\w+)`[^|]*\|(.*)$", re.MULTILINE)
            for vm in row_pattern.finditer(sub_content):
                variant_name = vm.group(1)
                memo = vm.group(2)
                memo_lower = memo.lower()
                if tier_label == "Tier 1":
                    tiers.tier1.add(variant_name)
                elif tier_label == "Tier 2":
                    if any(kw in memo_lower for kw in NO_OP_KEYWORDS):
                        tiers.tier2_no_op.add(variant_name)
                    else:
                        tiers.tier2_honest_error.add(variant_name)
                elif tier_label == "NA":
                    tiers.na.add(variant_name)
        enum_classifications[enum_name] = tiers
    return enum_classifications


def walk_match_arms(
    node: Node, file_path: Path, source_bytes: bytes
) -> Iterable[MatchArm]:
    """tree-sitter Node を walk、`match_expression` node 単位で arm 群を batch process。

    各 match_expression の直下 match_block 内の arm を一括処理:
    - `ast::EnumName::Variant` pattern arm: enum_name + variant_name で yield
    - `_` (wildcard) arm: 同 match block 内の explicit arm から推定された enum 名で yield
      (explicit arm が `ast::*` を含まない場合 = 自前 enum の wildcard、yield しない =
      本 audit scope 外として除外、これにより PropEvent / RustType / TypeDef 等の
      self-defined enum の wildcard 誤検出を構造的に排除)

    Nested match (e.g., `match prop { Foo::A(a) => match a { ... } }`) は再帰で別 batch
    として処理されるため、内側 wildcard は内側 enum の context、外側 wildcard は外側 enum
    の context として correctly 分類される。
    """
    if node.type == "match_expression":
        body = node.child_by_field_name("body")
        if body is not None:
            arms_in_block = [c for c in body.children if c.type == "match_arm"]
            # 各 arm の pattern を解析、explicit variant 抽出 + wildcard 検出
            explicit_yields: list[MatchArm] = []
            wildcard_arm_nodes: list[Node] = []
            enums_in_block: set[str] = set()
            for arm_node in arms_in_block:
                pattern_node = arm_node.child_by_field_name("pattern")
                if pattern_node is None:
                    for child in arm_node.children:
                        if child.type == "match_pattern":
                            pattern_node = child
                            break
                if pattern_node is None:
                    continue
                pat_str = pattern_text(pattern_node, source_bytes)
                if pat_str == "_":
                    wildcard_arm_nodes.append(arm_node)
                    continue
                for enum_variant in extract_enum_variants_from_pattern(pattern_node):
                    enum_name, variant_name = enum_variant
                    enums_in_block.add(enum_name)
                    body_text = source_bytes[
                        arm_node.start_byte : arm_node.end_byte
                    ].decode("utf-8", errors="replace")
                    explicit_yields.append(
                        MatchArm(
                            file=str(file_path.relative_to(REPO_ROOT)),
                            enum_name=enum_name,
                            variant_name=variant_name,
                            body_text=body_text,
                            line=arm_node.start_point[0] + 1,
                        )
                    )
            # explicit arms を yield
            yield from explicit_yields
            # wildcard arms は本 match block 内の enums_in_block のいずれかの context として yield
            # (typically 1 enum、enums_in_block 空 = 自前 enum の wildcard で本 audit scope 外
            # として skip)
            for warm_node in wildcard_arm_nodes:
                body_text = source_bytes[
                    warm_node.start_byte : warm_node.end_byte
                ].decode("utf-8", errors="replace")
                for enum_name in enums_in_block:
                    yield MatchArm(
                        file=str(file_path.relative_to(REPO_ROOT)),
                        enum_name=enum_name,
                        variant_name="_",
                        body_text=body_text,
                        line=warm_node.start_point[0] + 1,
                    )
        # match_expression の children を walk (nested match_expression を find)
        for child in node.children:
            yield from walk_match_arms(child, file_path, source_bytes)
        return

    for child in node.children:
        yield from walk_match_arms(child, file_path, source_bytes)


def pattern_text(node: Node, source_bytes: bytes) -> str:
    return source_bytes[node.start_byte:node.end_byte].decode("utf-8", errors="replace").strip()


def extract_enum_variants_from_pattern(pattern_node: Node) -> list[tuple[str, str]]:
    """match_pattern から `ast::EnumName::Variant` の (EnumName, Variant) を抽出。

    例: `ast::ClassMember::StaticBlock(sb)` → [("ClassMember", "StaticBlock")]
    例: `ast::Prop::Method(m) | ast::Prop::Getter(g)` → [("Prop", "Method"), ("Prop", "Getter")]

    tree-sitter-rust の `scoped_identifier` は再帰的 nested 構造を持つ
    (`ast::Prop::KeyValue` = `scoped_identifier(scoped_identifier(ast, Prop), KeyValue)`)
    のため、outer-most scoped_identifier のみ process し、inner scoped_identifier
    (= path component) を re-process しないよう構造的に skip する
    (= nested processing で false positive (ast, Prop) を generate しない)。
    """
    results: list[tuple[str, str]] = []

    def process_scoped(n: Node) -> None:
        text = n.text.decode("utf-8", errors="replace") if n.text else ""
        parts = text.split("::")
        if len(parts) >= 3 and parts[0] == "ast":
            results.append((parts[-2], parts[-1]))
        elif len(parts) == 2:
            # `Prop::Method` (no ast:: prefix)
            results.append((parts[0], parts[1]))

    def walk(n: Node) -> None:
        if n.type == "scoped_identifier":
            # outer-most scoped_identifier として process、子 scoped_identifier
            # (path component) は再度 walk しない (= 子は本 process が text 全体に含めて
            # 処理済 = `ast::Prop::Variant` form 全体から enum/variant を抽出済)
            process_scoped(n)
            return
        for c in n.children:
            walk(c)

    walk(pattern_node)
    return results




def classify_arm_body(body_text: str) -> str:
    """arm body から handle 種類を分類。

    Returns:
        "unsupported_error": UnsupportedSyntaxError::new(...) を含む
        "unreachable": unreachable!(...) を含む
        "no_op": 空 block ({}) または comment-only block
        "handled": 上記以外 (= 実 code)
    """
    # body_text は "Variant(args) => body," 形式
    # `=>` 後の body 部分を抽出
    arrow_idx = body_text.find("=>")
    if arrow_idx < 0:
        return "unknown"
    body = body_text[arrow_idx + 2:].strip()
    # trailing `,` を除去
    body = body.rstrip(",").strip()

    if "UnsupportedSyntaxError::new" in body:
        return "unsupported_error"
    if "unreachable!" in body:
        return "unreachable"
    # empty block detection
    no_op_pattern = re.compile(r"^\s*{(\s|/\*[^*]*\*/|//[^\n]*\n)*}\s*$", re.DOTALL)
    if no_op_pattern.match(body):
        return "no_op"
    # 単一 `()` (unit) も no_op
    if body == "()" or body == "{ () }":
        return "no_op"
    return "handled"


def parse_rust_file(file_path: Path) -> list[MatchArm]:
    """1 つの .rs file を tree-sitter で parse、全 match_arm を抽出。"""
    parser = Parser(RUST_LANGUAGE)
    source_bytes = file_path.read_bytes()
    tree = parser.parse(source_bytes)
    return list(walk_match_arms(tree.root_node, file_path, source_bytes))


def is_transformer_file(file: str) -> bool:
    return "/transformer/" in file


def is_typeresolver_file(file: str) -> bool:
    return "/type_resolver/" in file or "/pipeline/" in file


def verify_enum(
    enum_name: str,
    doc: TierClassification,
    code_arms: list[MatchArm],
    wildcard_arms: list[MatchArm],
    scope_files: frozenset[str],
) -> tuple[list[str], list[str]]:
    """1 enum の doc-code sync を verify、(scope 内 violation, scope 外 violation) を返す。

    scope 内 violation = exit code 1 で fail
    scope 外 violation = I-203 用 detection report (exit code 0、stderr に warning)
    """
    in_scope: list[str] = []
    out_scope: list[str] = []

    def record(file: str, msg: str) -> None:
        if file in scope_files:
            in_scope.append(msg)
        else:
            out_scope.append(msg)

    # `_` arm 検出 = Rule 11 (d-1) violation
    for warm in wildcard_arms:
        record(
            warm.file,
            f"Rule 11 (d-1) violation: `_` arm in match block dispatching "
            f"`ast::{enum_name}::*` ({warm.file}:{warm.line})",
        )

    # variant 別 grouping
    code_by_variant: dict[str, list[MatchArm]] = {}
    for arm in code_arms:
        code_by_variant.setdefault(arm.variant_name, []).append(arm)

    # Tier 1 (Handled) check
    for variant in doc.tier1:
        if variant not in code_by_variant:
            # 全 src/ で 1 arm もないなら drift report (which file? 一般的)
            in_scope.append(
                f"Tier 1 doc-code drift: `{enum_name}::{variant}` (doc Tier 1) "
                f"but no match arm found in src/"
            )
            continue
        for arm in code_by_variant[variant]:
            kind = classify_arm_body(arm.body_text)
            if kind == "unreachable":
                record(
                    arm.file,
                    f"Tier 1/NA mismatch: `{enum_name}::{variant}` (doc Tier 1) "
                    f"but code arm at {arm.file}:{arm.line} is unreachable!()",
                )

    # Tier 2 honest error check: Transformer = UnsupportedSyntaxError 必須
    for variant in doc.tier2_honest_error:
        if variant not in code_by_variant:
            continue
        for arm in code_by_variant[variant]:
            kind = classify_arm_body(arm.body_text)
            if is_transformer_file(arm.file):
                if kind not in ("unsupported_error", "unreachable", "handled"):
                    record(
                        arm.file,
                        f"Tier 2 (honest error) phase mismatch: `{enum_name}::{variant}` "
                        f"in Transformer file {arm.file}:{arm.line} should call "
                        f"`UnsupportedSyntaxError::new(...)` but is {kind}",
                    )
            elif is_typeresolver_file(arm.file):
                # TypeResolver は no-op (reason comment 付き) or handled
                if kind == "unsupported_error":
                    record(
                        arm.file,
                        f"Tier 2 phase mismatch: `{enum_name}::{variant}` in TypeResolver "
                        f"file {arm.file}:{arm.line} should not call UnsupportedSyntaxError "
                        f"(static analysis phase abort 不可、明示 no-op で記述)",
                    )

    # Tier 2 no-op (filter out) check: 全 phase で no-op が ideal
    for variant in doc.tier2_no_op:
        if variant not in code_by_variant:
            continue
        for arm in code_by_variant[variant]:
            kind = classify_arm_body(arm.body_text)
            if kind not in ("no_op", "handled"):
                # handled は special case (phase split の Tier 1) として許容
                # 残りは違反 (UnsupportedSyntaxError or unreachable)
                record(
                    arm.file,
                    f"Tier 2 (no-op / filter-out) mismatch: `{enum_name}::{variant}` "
                    f"at {arm.file}:{arm.line} should be no-op (filter out / no-op variant) "
                    f"but is {kind}",
                )

    # NA variant check: unreachable!() であるべき
    for variant in doc.na:
        if variant not in code_by_variant:
            continue
        for arm in code_by_variant[variant]:
            kind = classify_arm_body(arm.body_text)
            if kind != "unreachable":
                record(
                    arm.file,
                    f"NA mismatch: `{enum_name}::{variant}` (doc NA) at {arm.file}:{arm.line} "
                    f"should be unreachable!() but is {kind}",
                )

    return in_scope, out_scope


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit AST variant coverage (PRD 2.7 Q4 / Rule 11 compliance)"
    )
    parser.add_argument("--verbose", action="store_true", help="Verbose info to stderr")
    parser.add_argument(
        "--codebase-wide",
        action="store_true",
        help="Codebase 全体の `_` arm を I-203 用 report として出力 (本 PRD scope 外も含む)",
    )
    parser.add_argument(
        "--files",
        nargs="*",
        type=str,
        default=None,
        help=(
            "新 PRD の Impact Area file (relative path from repo root) を listing し、"
            "これらの file 内 `_` arm violations を out-of-scope detection として "
            "report (RC-8 / Rule 11 d-5 適用)。指定 file は PRD 2.7 SCOPE_FILES と "
            "等価扱い (scope-equivalent verification)。"
        ),
    )
    args = parser.parse_args()

    if not AST_VARIANTS_MD.exists():
        print(f"ERROR: {AST_VARIANTS_MD} not found", file=sys.stderr)
        return 2

    doc_classifications = parse_ast_variants_md()
    if args.verbose:
        for enum_name, tc in doc_classifications.items():
            print(
                f"[doc] {enum_name}: Tier1={sorted(tc.tier1)} "
                f"Tier2={sorted(tc.tier2)} NA={sorted(tc.na)}",
                file=sys.stderr,
            )

    # 全 .rs file を parse、全 match_arm を収集
    all_arms: list[MatchArm] = []
    for rust_file in SRC_DIR.rglob("*.rs"):
        all_arms.extend(parse_rust_file(rust_file))

    # RC-8 / Rule 11 (d-5): --files で指定された PRD Impact Area file を scope に追加
    # (PRD 2.7 SCOPE_FILES と等価扱い、新 PRD の pre-draft audit)
    effective_scope_files: frozenset[str] = PRD_2_7_SCOPE_FILES
    if args.files:
        effective_scope_files = frozenset(PRD_2_7_SCOPE_FILES | set(args.files))
        if args.verbose:
            print(
                f"[scope] additional --files: {sorted(args.files)}",
                file=sys.stderr,
            )

    # PRD 2.7 scope enum ごとに sync 検証
    all_in_scope: list[str] = []
    all_out_scope: list[str] = []
    for enum_name in PRD_2_7_SCOPE_ENUMS:
        if enum_name not in doc_classifications:
            all_in_scope.append(
                f"Grammar gap: `{enum_name}` section not found in ast-variants.md "
                f"(PRD 2.7 scope enum)"
            )
            continue

        scope_arms = [
            a for a in all_arms if a.enum_name == enum_name and a.variant_name != "_"
        ]
        scope_wildcards = [
            a for a in all_arms if a.enum_name == enum_name and a.variant_name == "_"
        ]

        if args.verbose:
            print(
                f"[code] {enum_name}: {len(scope_arms)} arm(s) in "
                f"{len(set(a.file for a in scope_arms))} file(s)",
                file=sys.stderr,
            )

        in_scope, out_scope = verify_enum(
            enum_name,
            doc_classifications[enum_name],
            scope_arms,
            scope_wildcards,
            effective_scope_files,
        )
        all_in_scope.extend(in_scope)
        all_out_scope.extend(out_scope)

    # Out-of-scope violations = I-203 用 detection report (exit code 0 で warning)
    if all_out_scope:
        print(
            f"WARNING: {len(all_out_scope)} out-of-scope violation(s) "
            f"(I-203 候補、本 PRD 2.7 scope 外、priority reclassify input):",
            file=sys.stderr,
        )
        if args.codebase_wide or args.verbose:
            for v in all_out_scope:
                print(f"  - {v}", file=sys.stderr)
        else:
            print(
                "  (--codebase-wide または --verbose で詳細出力)", file=sys.stderr
            )

    if all_in_scope:
        print(
            f"FAIL: {len(all_in_scope)} in-scope (PRD 2.7) violation(s) "
            f"in scope files {sorted(PRD_2_7_SCOPE_FILES)}:",
            file=sys.stderr,
        )
        for v in all_in_scope:
            print(f"  - {v}", file=sys.stderr)
        return 1

    print(
        f"PASS: AST variant coverage audit (PRD 2.7 scope enums: "
        f"{', '.join(PRD_2_7_SCOPE_ENUMS)}, scope files: "
        f"{len(PRD_2_7_SCOPE_FILES)})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
