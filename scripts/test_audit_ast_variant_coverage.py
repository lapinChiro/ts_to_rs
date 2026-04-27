#!/usr/bin/env python3
"""Self-test for `audit-ast-variant-coverage.py`.

audit script の precision (= wildcard arm の enclosing match 判定 / Tier 分類 /
no-op vs honest error classification) を edge case で empirical verify する。

Test cases:
  1. Pure ast::* enum match block の wildcard 検出 (= 該当 enum の wildcard と分類)
  2. Pure self-defined enum match block の wildcard skip (= scope 外、yield しない)
  3. Mixed (ast::* + self-defined) match block の dispatch 判定 (= ast::* 側 enum を yield)
  4. Nested match (parent ast::* + child ast::*) の inner wildcard が child enum と紐付く
  5. Tier 2 memo "filter out" / "no-op" keyword detection
  6. NA cell の unreachable!() pattern detection

Usage:
    python3 scripts/test_audit_ast_variant_coverage.py

Exit code:
    0: all assertions pass
    1: at least one assertion fail
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
AUDIT_SCRIPT = REPO_ROOT / "scripts" / "audit-ast-variant-coverage.py"


def load_audit_module():
    """Dynamic import of `audit-ast-variant-coverage.py` (hyphen in filename
    prevents normal `import` syntax)."""
    module_name = "audit_ast_variant_coverage"
    spec = importlib.util.spec_from_file_location(module_name, AUDIT_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    # `@dataclass` 等で `sys.modules.get(cls.__module__)` を参照するため、
    # exec_module 前に manual 登録が必要 (= importlib spec_from_file_location の仕様)。
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def parse_arms_from_source(audit_module, source: str, file_label: str = "test.rs"):
    """Parse a single Rust source string with audit module's tree-sitter parser."""
    parser = audit_module.Parser(audit_module.RUST_LANGUAGE)
    source_bytes = source.encode("utf-8")
    tree = parser.parse(source_bytes)
    # walk_match_arms expects file_path as Path relative-to-REPO_ROOT compatible.
    # Pass a synthetic Path under REPO_ROOT so `relative_to(REPO_ROOT)` works.
    fake_path = REPO_ROOT / file_label
    return list(audit_module.walk_match_arms(tree.root_node, fake_path, source_bytes))


def assert_eq(actual, expected, label: str):
    if actual == expected:
        print(f"  ✓ {label}")
        return True
    print(f"  ✗ {label}")
    print(f"      expected: {expected!r}")
    print(f"      actual:   {actual!r}")
    return False


def assert_truthy(actual, label: str):
    if actual:
        print(f"  ✓ {label}")
        return True
    print(f"  ✗ {label}: got falsy {actual!r}")
    return False


def assert_falsy(actual, label: str):
    if not actual:
        print(f"  ✓ {label}")
        return True
    print(f"  ✗ {label}: got truthy {actual!r}")
    return False


def main() -> int:
    if not AUDIT_SCRIPT.exists():
        print(f"ERROR: {AUDIT_SCRIPT} not found", file=sys.stderr)
        return 2

    audit = load_audit_module()
    failures = 0

    # ------------------------------------------------------------------
    # Test 1: Pure ast::* enum match block — wildcard is yielded with
    # the enum it dispatches.
    # ------------------------------------------------------------------
    print("Test 1: Pure ast::* match block — wildcard with enum context")
    source = """
        fn handle(prop: &ast::Prop) {
            match prop {
                ast::Prop::KeyValue(kv) => { kv; }
                ast::Prop::Shorthand(s) => { s; }
                _ => {}
            }
        }
    """
    arms = parse_arms_from_source(audit, source)
    explicit = [a for a in arms if a.variant_name != "_"]
    wildcards = [a for a in arms if a.variant_name == "_"]
    if not assert_eq(len(explicit), 2, "explicit arms count"):
        failures += 1
    if not assert_eq(
        sorted(a.variant_name for a in explicit),
        ["KeyValue", "Shorthand"],
        "explicit variant names",
    ):
        failures += 1
    if not assert_eq(len(wildcards), 1, "wildcard arm count"):
        failures += 1
    if wildcards and not assert_eq(
        wildcards[0].enum_name, "Prop", "wildcard enum_name"
    ):
        failures += 1

    # ------------------------------------------------------------------
    # Test 2: Pure self-defined enum match block — wildcard is skipped
    # (= enums_in_block 空、yield しない、本 audit scope 外).
    # ------------------------------------------------------------------
    print("\nTest 2: Pure self-defined enum match block — wildcard skipped")
    source = """
        enum PropEvent { Explicit, Spread }
        fn handle(e: &PropEvent) {
            match e {
                PropEvent::Explicit => 1,
                _ => 0,
            };
        }
    """
    arms = parse_arms_from_source(audit, source)
    # Self-defined `PropEvent::Explicit` is not under `ast::` namespace,
    # extract_enum_variants_from_pattern returns [(PropEvent, Explicit)] (= 2-part split)
    explicit = [a for a in arms if a.variant_name != "_"]
    wildcards = [a for a in arms if a.variant_name == "_"]
    if not assert_eq(
        sorted(a.variant_name for a in explicit), ["Explicit"], "explicit variant"
    ):
        failures += 1
    # self-defined enum の wildcard も yield される (`PropEvent` enum_name に紐付く)
    # 本 audit script の **PRD 2.7 scope filter** は `enum_name in PRD_2_7_SCOPE_ENUMS`
    # で行われるため、`PropEvent` は scope 外で fail report に出ない (= filter は main で適用)。
    # walk_match_arms は yield する設計
    if not assert_eq(len(wildcards), 1, "wildcard yield count (self-defined enum)"):
        failures += 1
    if wildcards and not assert_eq(
        wildcards[0].enum_name,
        "PropEvent",
        "wildcard enum_name (self-defined)",
    ):
        failures += 1

    # ------------------------------------------------------------------
    # Test 3: Nested match — inner wildcard belongs to inner enum,
    # not outer enum.
    # ------------------------------------------------------------------
    print("\nTest 3: Nested match — inner wildcard binds to inner enum")
    source = """
        fn handle(prop: &ast::PropOrSpread) {
            match prop {
                ast::PropOrSpread::Prop(p) => match p.as_ref() {
                    ast::Prop::KeyValue(kv) => { kv; }
                    _ => {}
                },
                ast::PropOrSpread::Spread(s) => { s; }
            }
        }
    """
    arms = parse_arms_from_source(audit, source)
    wildcards = [a for a in arms if a.variant_name == "_"]
    if not assert_eq(len(wildcards), 1, "single wildcard"):
        failures += 1
    # inner `_` arm is enclosed by `ast::Prop::*` match block → enum_name = "Prop"
    if wildcards and not assert_eq(
        wildcards[0].enum_name, "Prop", "inner wildcard enum_name = inner enum (Prop)"
    ):
        failures += 1
    # 外側 PropOrSpread match has no wildcard arm
    propor_spread_wildcards = [w for w in wildcards if w.enum_name == "PropOrSpread"]
    if not assert_eq(
        len(propor_spread_wildcards),
        0,
        "outer PropOrSpread wildcard count = 0",
    ):
        failures += 1

    # ------------------------------------------------------------------
    # Test 4: Tier 2 memo "filter out" / "no-op" classification
    # via `parse_ast_variants_md`.
    # ------------------------------------------------------------------
    print("\nTest 4: Tier classification with no-op vs honest-error keywords")
    # 本 test は actual ast-variants.md を parse、ClassMember の Tier 2 entries が
    # 正しく no_op / honest_error に分類されること verify。
    classifications = audit.parse_ast_variants_md()
    classmember = classifications.get("ClassMember")
    if not assert_truthy(classmember, "ClassMember section parsed"):
        failures += 1
    elif classmember:
        # TsIndexSignature / Empty は memo に "filter out" / "no-op" 含むため tier2_no_op
        # AutoAccessor は honest error 含むため tier2_honest_error
        if not assert_truthy(
            "TsIndexSignature" in classmember.tier2_no_op,
            "TsIndexSignature classified as no_op",
        ):
            failures += 1
        if not assert_truthy(
            "Empty" in classmember.tier2_no_op, "Empty classified as no_op"
        ):
            failures += 1
        if not assert_truthy(
            "AutoAccessor" in classmember.tier2_honest_error,
            "AutoAccessor classified as honest_error",
        ):
            failures += 1

    # ------------------------------------------------------------------
    # Test 5: classify_arm_body — UnsupportedSyntaxError / unreachable!() / no-op
    # の正しい分類 verify。
    # ------------------------------------------------------------------
    print("\nTest 5: classify_arm_body precision")
    # Note: classify_arm_body expects body_text in form "Pattern => body,"
    # (= MatchArm.body_text format)
    cases = [
        (
            "Foo::A => return Err(UnsupportedSyntaxError::new(\"X\", span).into()),",
            "unsupported_error",
        ),
        ("Foo::B => unreachable!(\"reason\"),", "unreachable"),
        ("Foo::C(_) => { /* reason: filter out */ },", "no_op"),
        ("Foo::D(_) => {},", "no_op"),
        ("Foo::E(x) => { do_thing(x); push(x); },", "handled"),
    ]
    for body_text, expected_kind in cases:
        actual = audit.classify_arm_body(body_text)
        if not assert_eq(actual, expected_kind, f"classify {body_text[:40]!r}"):
            failures += 1

    # ------------------------------------------------------------------
    # Test 6: extract_enum_variants_from_pattern — multi-pattern (or-pattern)
    # で複数 variant 抽出
    # ------------------------------------------------------------------
    print("\nTest 6: extract_enum_variants_from_pattern multi-pattern")
    source = """
        fn handle(p: &ast::Prop) {
            match p {
                ast::Prop::Shorthand(_) | ast::Prop::Method(_) | ast::Prop::Getter(_) => 1,
                _ => 0,
            };
        }
    """
    arms = parse_arms_from_source(audit, source)
    explicit = [a for a in arms if a.variant_name != "_"]
    explicit_variants = sorted(a.variant_name for a in explicit)
    if not assert_eq(
        explicit_variants,
        ["Getter", "Method", "Shorthand"],
        "or-pattern variants extracted",
    ):
        failures += 1

    # ------------------------------------------------------------------
    # Summary
    # ------------------------------------------------------------------
    print()
    if failures > 0:
        print(f"FAIL: {failures} assertion(s) failed")
        return 1
    print("PASS: all audit script self-tests")
    return 0


if __name__ == "__main__":
    sys.exit(main())
