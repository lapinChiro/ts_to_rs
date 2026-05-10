#!/usr/bin/env python3
"""verify_line_refs.py — Method A formal lock-in utility (PRD I-D-pre Cell 4 v11-7)

================================================================================
Formal lock-in metadata (PRD I-D-pre Cell 4 / v11-7、Path B split adoption 2026-05-11)
================================================================================

**Status**: Formal regression-tested utility lock-in via PRD I-D-pre Implementation
Phase 2 T1-pre-5 (= Path B split 2026-05-11、Iteration v12 bootstrap origin promoted to
formal utility status)。

**Purpose**: Detect line-ref drift in PRD doc by extracting "line N" / "lines N-M" claims
and verifying that referenced lines actually contain content matching the textual context.
Heading-based detection (= "## Foo" / "### Foo") is the most reliable check class.

**Coverage scope (regression-tested)**:
- Heading-based line-ref drift detection (= primary coverage class)
- Historical preservation policy (= "v8 当時 line X" 等の historical claims excluded
  via `is_historical_claim` predicate、本 PRD I-D parent Iteration v12 で formal 確定 policy)
- Out of scope: code-level line refs (= `<file>:<line>` references in handoff docs are
  covered by sibling utility `scripts/audit-handoff-doc-line-refs.py` = PRD I-D-pre Cell 3)

**PRD I-D-pre binding**:
- Cell 4 (v11-7): Layer 1 factual accuracy semantic check rule wording 追加
  (`.claude/rules/check-job-review-layers.md` Layer 1 sub-step) + Method A audit utility
  formal lock-in
- Test contract: `tests/i_d_pre_method_a_test.rs::test_method_a_line_ref_drift_detection`
  + `test_method_a_utility_metadata_header_embed`

**Origin (= bootstrap chain history)**:
- Iteration v12 (2026-05-10): Bootstrap implementation as Method A utility for PRD I-D
  Spec stage convergence (= Cell 19 v11-7 audit auto-verify mechanism early implementation)
- Iteration v17 (2026-05-10): Empirical proof for line-ref drift class complete absorption
  (= verify_line_refs.py drift detection vs Iteration v17 third-party review findings)
- Path B split adoption (2026-05-11): formal regression-tested utility lock-in via PRD
  I-D-pre Cell 4 (= bootstrapping circularity 構造的解消 base、Iteration v17 plateau
  bootstrap utility correctness ceiling resolution)

================================================================================
Usage
================================================================================

    python3 scripts/verify_line_refs.py <prd_path>

Output: list of detected drifts (= claim line N → actual heading at line M, suggested fix).

================================================================================
Limitations (= known structural false-positive class、preservation policy 適用)
================================================================================

- Heuristic noun phrase extraction; may miss novel phrasings
- Does not auto-fix; reports drifts for human review
- False positives possible for intentionally-historical refs (= "v8 当時 line X claim was wrong")
  → such claims are typically wrapped in quotes / phrases like "旧 X" / "当時"; flagged but
    annotated for human triage (= `is_historical_claim` predicate filters)
- Iteration log entries (= `### Iteration vN` headings 直下) は historical preservation policy
  対象 = drift detected でも本 PRD I-D-pre 完了基準には含めない (= PRD I-D-main で Spec
  Review Iteration Log preservation policy formal lock-in 後 cross-PRD invariant)
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import NamedTuple


HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")
LINE_REF_RE = re.compile(
    r"(?P<context>[^\n]{0,80}?)"  # up to 80 chars of context before the line ref
    r"\(?\s*(?:line|lines)\s+(?P<line_num>\d+)(?:[-–]\s*(?P<line_end>\d+))?",
)


class HeadingEntry(NamedTuple):
    line: int
    level: int
    title: str
    title_lower: str


class LineRefClaim(NamedTuple):
    source_line: int
    context: str
    target_line: int
    target_line_end: int | None
    raw_match: str


class Drift(NamedTuple):
    source_line: int
    context: str
    claimed_line: int
    actual_line: int | None
    matched_heading: str
    confidence: str  # "high" / "medium" / "low" / "historical"


def parse_headings(lines: list[str]) -> list[HeadingEntry]:
    """Extract all markdown headings (## / ###) with their line numbers."""
    headings: list[HeadingEntry] = []
    for i, line in enumerate(lines, start=1):
        m = HEADING_RE.match(line)
        if m:
            level = len(m.group(1))
            title = m.group(2).strip()
            headings.append(
                HeadingEntry(line=i, level=level, title=title, title_lower=title.lower())
            )
    return headings


def parse_line_refs(lines: list[str]) -> list[LineRefClaim]:
    """Extract all 'line N' / 'lines N-M' claims with surrounding context."""
    claims: list[LineRefClaim] = []
    for i, line in enumerate(lines, start=1):
        for m in LINE_REF_RE.finditer(line):
            context = m.group("context").strip()
            target_line = int(m.group("line_num"))
            line_end = m.group("line_end")
            target_line_end = int(line_end) if line_end is not None else None
            claims.append(
                LineRefClaim(
                    source_line=i,
                    context=context,
                    target_line=target_line,
                    target_line_end=target_line_end,
                    raw_match=m.group(0),
                )
            )
    return claims


def is_historical_claim(context: str) -> bool:
    """Detect if a line-ref claim is intentionally historical (= preserved as historical record)."""
    historical_markers = [
        "旧 ",
        "旧wording",
        "旧 wording",
        "当時",
        "claim was wrong",
        "claim、actual",
        "claim is wrong",
        "factual lie",
        "claim です",
        "claim は",
        "Iteration v8 当時",
        "Iteration v9 当時",
        "Iteration v10 当時",
        "は当時",
    ]
    context_lower = context.lower()
    return any(marker.lower() in context_lower for marker in historical_markers)


def heading_keywords(title: str) -> set[str]:
    """Extract keyword set from a heading title for fuzzy match."""
    # Remove markdown formatting
    cleaned = re.sub(r"[`*_~()\[\]:]", " ", title)
    # Split on whitespace and CJK boundaries (loose; Japanese / English mix)
    tokens = re.findall(r"[A-Za-z0-9_-]+|[぀-ヿ一-鿿]{2,}", cleaned)
    return {t.lower() for t in tokens if len(t) >= 2}


def context_keywords(context: str) -> set[str]:
    """Extract keyword set from context phrase that precedes a line-ref."""
    cleaned = re.sub(r"[`*_~()\[\]:,。、]", " ", context)
    tokens = re.findall(r"[A-Za-z0-9_-]+|[぀-ヿ一-鿿]{2,}", cleaned)
    return {t.lower() for t in tokens if len(t) >= 2}


def find_drifts(
    lines: list[str],
    headings: list[HeadingEntry],
    claims: list[LineRefClaim],
) -> list[Drift]:
    """For each claim, check if the referenced line is the start of a heading whose
    keywords match the context phrase. Report drift if the nearest matching heading is
    at a different line.
    """
    drifts: list[Drift] = []

    # Build a quick lookup by line number
    headings_by_line = {h.line: h for h in headings}

    for claim in claims:
        # Skip claims that are part of "lines N-M" range references (= matrix table line ranges,
        # not heading refs)
        if claim.target_line_end is not None:
            continue

        # Skip if context is empty or too generic
        if not claim.context or len(claim.context) < 5:
            continue

        # Check if the claim is intentionally historical (= preserved as historical record)
        if is_historical_claim(claim.context):
            continue

        ctx_kw = context_keywords(claim.context)
        if not ctx_kw:
            continue

        # Check direct hit: is target_line a heading whose keywords overlap ctx?
        target_heading = headings_by_line.get(claim.target_line)
        if target_heading:
            tk = heading_keywords(target_heading.title)
            overlap = ctx_kw & tk
            if overlap:
                # Direct hit; no drift
                continue

        # Search nearby headings (within ±10 lines) for keyword match
        best_heading: HeadingEntry | None = None
        best_overlap_size = 0
        for h in headings:
            if abs(h.line - claim.target_line) > 10:
                continue
            tk = heading_keywords(h.title)
            overlap = ctx_kw & tk
            if len(overlap) > best_overlap_size and len(overlap) >= 2:
                # Require at least 2-keyword overlap to avoid false positives
                best_overlap_size = len(overlap)
                best_heading = h

        if best_heading and best_heading.line != claim.target_line:
            # Drift detected: claim says line N, actual heading is at line M
            confidence = "high" if best_overlap_size >= 3 else "medium"
            drifts.append(
                Drift(
                    source_line=claim.source_line,
                    context=claim.context[:60],
                    claimed_line=claim.target_line,
                    actual_line=best_heading.line,
                    matched_heading=best_heading.title[:80],
                    confidence=confidence,
                )
            )
        elif not target_heading and claim.target_line not in headings_by_line:
            # Target line is not a heading at all; check if any nearby heading matches the
            # context strongly. If yes, that's a drift; if no, skip (= line-ref points to
            # non-heading content like a table row, prose, etc.)
            if best_heading and best_overlap_size >= 3:
                drifts.append(
                    Drift(
                        source_line=claim.source_line,
                        context=claim.context[:60],
                        claimed_line=claim.target_line,
                        actual_line=best_heading.line,
                        matched_heading=best_heading.title[:80],
                        confidence="medium",
                    )
                )

    return drifts


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: verify_line_refs.py <prd_path>", file=sys.stderr)
        return 2

    prd_path = Path(sys.argv[1])
    if not prd_path.exists():
        print(f"PRD path not found: {prd_path}", file=sys.stderr)
        return 2

    text = prd_path.read_text(encoding="utf-8")
    lines = text.splitlines()

    headings = parse_headings(lines)
    claims = parse_line_refs(lines)
    drifts = find_drifts(lines, headings, claims)

    print(f"Headings: {len(headings)}")
    print(f"Line refs: {len(claims)}")
    print(f"Drifts (heuristic-detected, requires human triage): {len(drifts)}")
    print()

    if drifts:
        print("=== Detected drifts ===")
        for d in drifts:
            print(
                f"PRD line {d.source_line}: "
                f"claim='{d.context}' line {d.claimed_line} "
                f"→ actual heading '{d.matched_heading}' at line {d.actual_line} "
                f"(confidence={d.confidence})"
            )
        return 1

    print("PASS: No drifts detected.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
