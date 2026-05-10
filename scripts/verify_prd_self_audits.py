#!/usr/bin/env python3
"""verify_prd_self_audits.py — Path E formal lock-in utility (PRD I-D-pre Cells 1, 2, 5)

================================================================================
Formal lock-in metadata (PRD I-D-pre Cells 1, 2, 5、Path B split adoption 2026-05-11)
================================================================================

**Status**: Formal regression-tested utility lock-in via PRD I-D-pre Implementation
Phase 2 T1-pre-6 (= Path B split 2026-05-11、Iteration v16 bootstrap origin promoted to
formal utility status with F6 fix + F7 fix + Axis 3 extension integrated)。

**Purpose**: Multi-axis self-applied audit utility that detects PRD doc structural drifts
across 4 axes (cross-reference consistency / status pending verdict / label namespace
collision / external file drift)、complement `verify_line_refs.py` (Method A = Cell 19
v11-7) for multi-axis bootstrap of v12-2 / v11-7 / v13-5 / v3-6 / v4-2 / v5-1 / v11-5
self-applied gap classes。

**Coverage scope (regression-tested)**:
1. **Axis 1 (Cell 2 / v5-1)**: verify_cross_reference_cell_consistency
   = matrix vs **5 cross-reference sections** (= 7 contexts grouped) で cell #
   appearance consistency
   - **F6 fix integrated (Path B split 2026-05-11、Iteration v17 F6 由来)**: Axis 1
     threshold "5" arbitrary heuristic を **spec-traceable allow-list** (=
     SECTION_COVERAGE_POLICY) に置換。Expected cell count を hardcoded "30" から
     auto-detect (= matrix table actual row count)。
   - **A2 fix integrated (Path B split 2026-05-11、/check_job L3-3 由来)**:
     SECTION_COVERAGE_POLICY を 2 → 5 sections (= 7 contexts grouped) に拡張、Cell 2
     v5-1 oracle observation の 7 enumerated contexts (= In Scope / Out of Scope /
     Tier 2 reclassify / INV-N verification lists / dispatch tree comments / Test Plan /
     TN completion criteria) を natural parent-level grouping で全 cover。policy 分類:
     `full` (= Scope + Spec→Impl Mapping = 2 sections) / `partition_ok` (= Invariants +
     Test Plan + Implementation Stage Tasks = 3 sections)
2. **Axis 2 (Cell 1 / v3-6 + v4-2 consolidated)**: verify_status_pending_verdict
   = current spec section の status field staleness detect
   - **F7 fix integrated (Path B split 2026-05-11、Iteration v17 F7 由来)**: Axis 2
     TS-X over-exclusion を **post-v15 wording presence 要求** に refine (= TS-X heading
     内でも v15+ wording (= "v17 期待" / "v18+ で完成" 等の post-v15 specific iteration
     reference) なら flag = blanket exclude 解消)
   - **A4 fix integrated (Path B split 2026-05-11、/check_job L3-5 由来)**:
     I-D-parent-specific legacy `STALE_STATUS_PATTERNS` (= "IN PROGRESS で convergence
     verify" wording) は dead code 削除 (= grep 0 hits empirical confirm)
3. **Axis 3 (Cell 5 / v13-5)**: verify_label_namespace_collision
   = namespace prefix (R-x / C-x / M-x / etc.) の multi-referent collision detect
   - **Axis 3 extension integrated (Path B split 2026-05-11)**: cell-slot
     **identifier-level** vocabulary fork detection (= "cell-slot N" / "cell-slot #N"
     numeric identifier 用法のみ flag、descriptive uses ("cell-slot occurrence" /
     "cell-slot vocabulary fork") は legitimate concept descriptor として allow)
   - **Note (/check_job L3-2 reconciled)**: identifier-level fork が本 Axis 3 extension
     の正規 scope。"cell # / candidate ID / matrix #" 間の broader vocabulary fork
     detection は別 framework concern (= TODO `[I-D-future-vocab-fork]` 候補参照)
4. **Axis 4 (Cell 3 / v11-5)**: verify_external_file_drift
   = Impact Area table claim vs actual wc -l / stat cross-check (= I-D-pre Cell 3 で
     `scripts/audit-handoff-doc-line-refs.py` NEW として完全 absorption、本 Axis 4 は
     PRD-internal Impact Area table coverage のみ)
   - **A7 fix integrated (Path B split 2026-05-11、/check_job L1-4 由来)**:
     IMPACT_AREA_BYTES_RE byte count 9-digit → 12-digit (~ 1 TB) 拡張

**PRD I-D-pre binding**:
- Cell 1 (v3-6+v4-2): consolidated `verify_pending_verdict_findings_consistency` audit
  function 新設 + Path E Axis 2 F7 fix
- Cell 2 (v5-1): `verify_cross_reference_cell_consistency` audit function 新設 + Path E
  Axis 1 F6 fix (= allow-list 置換)
- Cell 5 (v13-5): `verify_cell_numbering_drift_detection` audit function 新設 + Path E
  Axis 3 cell-slot vocabulary fork coverage extension
- Test contract: `tests/i_d_pre_path_e_test.rs` (Axis 1/2/3/4 各 positive + negative +
  metadata header verify)

**Origin (= bootstrap chain history)**:
- Iteration v16 (2026-05-10): Bootstrap implementation as Path E utility for PRD I-D
  Spec stage convergence (= Cell 10/6+8/17/28 multi-axis early implementation)
- Iteration v17 (2026-05-10): Empirical detection of utility self-correctness ceiling
  (= F6 Axis 1 threshold "5" under-detection + F7 Axis 2 TS-X over-exclusion under-detection)
- Path B split adoption (2026-05-11): formal regression-tested utility lock-in via PRD
  I-D-pre Cells 1+2+5 with F6/F7/Axis 3 fix integrated (= bootstrapping circularity
  構造的解消、Iteration v17 plateau bootstrap utility correctness ceiling resolution)

================================================================================
Usage
================================================================================

    python3 scripts/verify_prd_self_audits.py <prd_path>

Output: list of detected drifts grouped by audit axis.

Exit codes:
    0 = all axes PASS (no CURRENT spec drifts)
    1 = at least one CURRENT spec drift detected (HISTORICAL drifts excluded per preservation policy)
    2 = invocation error (missing argument / file not found)

================================================================================
Cross-PRD verification matrix (= INV-4 4-tuple baseline post Path B split)
================================================================================

| PRD doc                         | Expected exit | Notes                                       |
|---------------------------------|---------------|---------------------------------------------|
| backlog/I-050-...md             | (not target)  | I-050 lacks Rule 10 Application section     |
| backlog/I-205-...md             | 0 (PASS)      | Pre-existing baseline preserve              |
| backlog/I-D-pre-...md           | 0 (PASS)      | Self-applied target post F6/F7/Axis 3 fix   |
| backlog/I-D-main-...md          | 0 (PASS)      | Path B split scope                          |
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple


# ---------------------------------------------------------------------------
# Common parsers (shared across audit axes)
# ---------------------------------------------------------------------------

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")
HISTORICAL_SECTION_HEADING_RE = re.compile(r"^### Iteration v\d+\b")


class HeadingEntry(NamedTuple):
    line: int
    level: int
    title: str


class SectionRange(NamedTuple):
    """Range of lines for a section, inclusive of start line, exclusive of end line."""

    title: str
    level: int
    start: int
    end: int


def parse_headings(lines: list[str]) -> list[HeadingEntry]:
    headings: list[HeadingEntry] = []
    for i, line in enumerate(lines, start=1):
        m = HEADING_RE.match(line)
        if m:
            headings.append(
                HeadingEntry(line=i, level=len(m.group(1)), title=m.group(2).strip())
            )
    return headings


def find_section_range(
    headings: list[HeadingEntry],
    title_pattern: str,
    level: int,
    total_lines: int,
) -> SectionRange | None:
    """Find the line range of a section by heading title pattern (substring match).

    **Path B split 2026-05-11 / A5 fix (/check_job L1-2)**: `total_lines` is now
    mandatory for consistent API (= former sentinel `10**9` magic number 排除)。
    All callers compute `total_lines = len(lines)` and pass explicitly。
    """
    rx = re.compile(title_pattern)
    for i, h in enumerate(headings):
        if h.level == level and rx.search(h.title):
            # End at next heading of same or higher level; fall back to EOF if none
            end = total_lines + 1
            for j in range(i + 1, len(headings)):
                if headings[j].level <= level:
                    end = headings[j].line
                    break
            return SectionRange(title=h.title, level=level, start=h.line, end=end)
    return None


def is_historical_iteration_log_line(
    line_num: int, headings: list[HeadingEntry], total_lines: int
) -> bool:
    """Determine if a line falls inside a `### Iteration v*` historical entry block.

    A5 fix (/check_job L1-2): `total_lines` parameter 追加 = `find_section_range` API
    整合、sentinel `10**9` 排除。
    """
    iteration_log_root = find_section_range(
        headings, r"^Spec Review Iteration Log", level=2, total_lines=total_lines
    )
    if iteration_log_root is None:
        return False
    return iteration_log_root.start <= line_num < iteration_log_root.end


# ---------------------------------------------------------------------------
# Axis 1: verify_cross_reference_cell_consistency (Cell 10 / v5-1)
# ---------------------------------------------------------------------------

CELL_NUM_RANGE_RE = re.compile(r"\b(\d{1,2})\s*[-–]\s*(\d{1,2})\b")
# F6 fix (Path B split 2026-05-11): case-insensitive + multi-pattern cell extraction
# Pattern 1: "cells N, M, ..." (lowercase historical form for I-D parent)
CELL_LIST_RE = re.compile(r"cells?\s+([\d,\s\-–/]+?)(?=[)。、]|\s*=|\s*\(|$)", re.IGNORECASE)
# Pattern 2: "**Cell N**" or "Cell N" capitalized standalone (= I-D-pre / I-D-main bullet list form)
CELL_STANDALONE_RE = re.compile(r"\bCell\s+(\d{1,2})\b")
# Pattern 3: "{N, M, ..., K}" bracket-list (= explicit cell # set enumeration)
CELL_BRACKET_LIST_RE = re.compile(r"\{([\d,\s\-–]+?)\}")
# Pattern 4 (A2 fix follow-up、Path B split 2026-05-11): markdown table first column "| N |"
# (= Spec→Impl Dispatch Arm Mapping table の cell # 列 / matrix table の cell # 列)。
# re.MULTILINE で per-line `^` anchor 適用、only match data rows (not header / separator)。
TABLE_FIRST_COL_NUM_RE = re.compile(r"^\|\s*(\d{1,2})\s*\|", re.MULTILINE)


def expand_cell_list(text: str) -> set[int]:
    """Extract cell numbers from common phrasings.

    **Supported patterns (F6 fix integrated)**:
    - "cells 1, 4, 5, 6, 7-9, 12" (lowercase, original I-D parent form)
    - "**Cell 1**" / "Cell 12" (capitalized standalone, I-D-pre / I-D-main bullet list form)
    - "{1, 2, 3, ..., 30}" (bracket-list, explicit cell # set enumeration)
    """
    cells: set[int] = set()

    # Pattern 1: "cells N..." (case-insensitive)
    for m in CELL_LIST_RE.finditer(text):
        body = m.group(1)
        # Handle ranges
        for r in CELL_NUM_RANGE_RE.finditer(body):
            lo, hi = int(r.group(1)), int(r.group(2))
            if 1 <= lo <= hi <= 999:
                cells.update(range(lo, hi + 1))
        # Handle individual numbers (after stripping ranges)
        body_no_range = CELL_NUM_RANGE_RE.sub(" ", body)
        for num_match in re.finditer(r"\b(\d{1,2})\b", body_no_range):
            n = int(num_match.group(1))
            if 1 <= n <= 999:
                cells.add(n)

    # Pattern 2: "Cell N" capitalized standalone
    for m in CELL_STANDALONE_RE.finditer(text):
        n = int(m.group(1))
        if 1 <= n <= 30:
            cells.add(n)

    # Pattern 3: "{N, M, ..., K}" bracket-list
    for m in CELL_BRACKET_LIST_RE.finditer(text):
        body = m.group(1)
        for r in CELL_NUM_RANGE_RE.finditer(body):
            lo, hi = int(r.group(1)), int(r.group(2))
            if 1 <= lo <= hi <= 999:
                cells.update(range(lo, hi + 1))
        body_no_range = CELL_NUM_RANGE_RE.sub(" ", body)
        for num_match in re.finditer(r"\b(\d{1,2})\b", body_no_range):
            n = int(num_match.group(1))
            if 1 <= n <= 999:
                cells.add(n)

    # Pattern 4: markdown table first column "| N |" (= Spec→Impl Mapping cell # 列)
    for m in TABLE_FIRST_COL_NUM_RE.finditer(text):
        n = int(m.group(1))
        if 1 <= n <= 30:
            cells.add(n)

    return cells


def collect_section_cells(lines: list[str], section: SectionRange) -> set[int]:
    """Collect all cell numbers mentioned in a given section's body."""
    body = "\n".join(lines[section.start - 1 : section.end - 1])
    return expand_cell_list(body)


def collect_matrix_cells(lines: list[str], headings: list[HeadingEntry]) -> set[int]:
    """Collect cell # values from the canonical matrix table in `## Problem Space > 組合せマトリクス`."""
    matrix_section = find_section_range(headings, r"^Problem Space", level=2, total_lines=len(lines))
    if matrix_section is None:
        return set()
    cells: set[int] = set()
    in_table = False
    for i in range(matrix_section.start, matrix_section.end):
        line = lines[i - 1] if i - 1 < len(lines) else ""
        if line.startswith("|"):
            # Match table row starting with cell #
            m = re.match(r"\|\s*(\d{1,2})\s*\|", line)
            if m:
                n = int(m.group(1))
                if 1 <= n <= 999:
                    cells.add(n)
                    in_table = True
        else:
            # Non-table line; if we already entered table and now exited, stop
            if in_table and not line.strip().startswith("|"):
                continue
    return cells


def verify_cross_reference_cell_consistency(
    lines: list[str], headings: list[HeadingEntry]
) -> list[str]:
    """Cell 10 (v5-1) audit: matrix vs Scope vs Mapping vs Test Plan で cell # appearance consistency.

    **F6 fix integrated (Path B split 2026-05-11、Iteration v17 F6 由来)**:
    Axis 1 threshold "5" arbitrary heuristic を **spec-traceable allow-list** に置換:
    - Expected cell count を hardcoded `30` から auto-detect (= matrix table actual row count)
    - Per-section coverage policy を SECTION_COVERAGE_POLICY allow-list で formal declare:
      * `"full"` = matrix 全 cells appearance 期待 (= ANY missing = flag、threshold 0)
      * `"partition_ok"` = legitimate subset (= partition by Layer / test category 等)、no flag
      * 未列挙 section = default strict (= ANY missing = flag)
    - Allow-list は spec-traceable (= 各 section の semantic role を explicitly enumerate)
    """
    drifts: list[str] = []
    matrix_cells = collect_matrix_cells(lines, headings)

    # F6 fix: auto-detect expected cell count (= matrix actual size、no hardcoded "30")
    if not matrix_cells:
        drifts.append(
            "verify_cross_reference_cell_consistency: matrix cells empty (= "
            "no `## 組合せマトリクス` section found or no cells extracted)"
        )
        return drifts

    # F6 fix + A2 fix (/check_job L3-3): spec-traceable per-section coverage policy allow-list
    # Cell 2 (v5-1) oracle observation enumerated **7 cross-reference contexts** =
    # In Scope / Out of Scope / Tier 2 reclassify / INV-N verification lists /
    # dispatch tree comments / Test Plan / TN completion criteria。
    # Natural parent-level grouping = 5 sections covering all 7 contexts:
    #   - Scope (level 2) = In/Out/Tier 2 sub-sections (= 3 contexts)
    #   - Spec→Impl Dispatch Arm Mapping (level 3) = dispatch tree comments
    #   - Invariants (level 2) = INV-N verification lists
    #   - Test Plan (level 2) = test category partition
    #   - Implementation Stage Tasks (level 2) = TN completion criteria
    # Section semantic role determines coverage expectation:
    #   "full"          = full matrix enumeration expected (= ANY missing = flag)
    #   "partition_ok"  = legitimate partition by sub-category (= no flag for subset)
    # Section heading patterns use word-boundary `\b` after section name = match both
    # bare form ("## Scope") and annotated form ("## Scope (3-tier 形式、Rule 6 (6-2) 適用)")
    # ↑ I-205 等 既存 PRD の annotated heading 互換性確保
    SECTION_COVERAGE_POLICY = [
        # (label, pattern, level, policy)
        # full enumeration expected: Scope (= In/Out/Tier 2 全 sub-sections の union が matrix)
        ("Scope (= In/Out/Tier 2 sub-sections)", r"^Scope\b", 2, "full"),
        # full 1-to-1 mapping expected: Spec→Impl Mapping (each matrix cell maps to a Task)
        # `Spec\s*→\s*Impl` で space variants 許容 (= I-D-pre "Spec→Impl" / I-205 "Spec → Impl")
        (
            "Spec→Impl Mapping (= dispatch tree comments)",
            r"^Spec\s*→\s*Impl",
            3,
            "full",
        ),
        # partition_ok: Invariants section = each INV-N covers a subset of cells
        ("Invariants (= INV-N verification lists)", r"^Invariants\b", 2, "partition_ok"),
        # partition_ok: Test Plan = partition by test category (audit_extensions / rule_wording / etc.)
        ("Test Plan (= test category partition)", r"^Test Plan\b", 2, "partition_ok"),
        # partition_ok: Implementation Stage Tasks = TN completion criteria per task
        (
            "Implementation Stage Tasks (= TN completion criteria)",
            r"^Implementation Stage Tasks\b",
            2,
            "partition_ok",
        ),
    ]

    for label, pattern, level, policy in SECTION_COVERAGE_POLICY:
        section = find_section_range(headings, pattern, level=level, total_lines=len(lines))
        if section is None:
            drifts.append(
                f"verify_cross_reference_cell_consistency: section '{label}' not found"
            )
            continue
        section_cells = collect_section_cells(lines, section)
        # Filter cells outside matrix range (false positives like "30 cells" / "12 candidates")
        max_cell = max(matrix_cells)
        relevant_cells = section_cells & set(range(1, max_cell + 1))
        missing = matrix_cells - relevant_cells

        if policy == "full":
            # Full coverage expected: ANY missing = flag (threshold 0 = F6 strict)
            if missing:
                drifts.append(
                    f"verify_cross_reference_cell_consistency: section '{label}' "
                    f"missing cells {sorted(missing)} (policy=full, expected full enumeration)"
                )
        elif policy == "partition_ok":
            # Legitimate partition: no flag (= F6 spec-traceable allow-list exception)
            pass
        else:
            # Unknown policy: default to strict = flag
            if missing:
                drifts.append(
                    f"verify_cross_reference_cell_consistency: section '{label}' "
                    f"missing cells {sorted(missing)} (policy={policy}, default strict)"
                )
    return drifts


# ---------------------------------------------------------------------------
# Axis 2: verify_status_pending_verdict (Cell 6+8 / v3-6 / v4-2)
# ---------------------------------------------------------------------------

# A4 fix (Path B split 2026-05-11、/check_job review L3-5): I-D-parent-specific legacy
# `STALE_STATUS_PATTERNS` removed = dead code (= grep -rE on backlog/ shows 0 matches in
# current PRDs)。Pre-empty list preserved as `[]` for forward extensibility (= future
# PRD-pattern-specific stale claims may be added)。
STALE_STATUS_PATTERNS: list[re.Pattern[str]] = []
TS_STATUS_RE = re.compile(r"^- \*\*Status\*\*:\s*(IN PROGRESS|PENDING|PARTIAL|TBD)\b")
# F7 fix: heading regex extended to match both "TS-N" and "TS-pre-N" (= I-D-pre / I-D-main split)
TS_HEADING_RE = re.compile(r"^### TS-(?:pre-)?\d+\b")
# F7 fix: post-v15 wording detection (= specific iteration reference v15+)
POST_V15_WORDING_RE = re.compile(r"\b(?:v|Iteration\s+v)(1[5-9]|[2-9]\d+)\b")


def verify_status_pending_verdict(
    lines: list[str], headings: list[HeadingEntry]
) -> list[str]:
    """Cell 6+8 (v3-6 / v4-2) audit: current spec section の status field staleness detect.

    **F7 fix integrated (Path B split 2026-05-11、Iteration v17 F7 由来)**:
    Axis 2 TS-X over-exclusion を **post-v15 wording presence 要求** に refine:
    - Heading regex を `TS-N` から `TS-(?:pre-)?N` に拡張 (= I-D-pre / I-D-main split sync)
    - TS-X heading 内でも post-v15 wording (= v15+ specific iteration reference: "v17 期待" /
      "v18+ で完成" / "Iteration v15 以降" 等) なら flag (= blanket exclude 解消)
    - Pre-v15 wording (= v1〜v14 reference) は legitimate early-stage spec として allow
    - I-D-parent-specific hardcoded "Iteration v1" stale pattern を削除 (= 過剰 false-positive)
    """
    drifts: list[str] = []
    for i, line in enumerate(lines, start=1):
        # Skip historical iteration log entries (preservation policy)
        if is_historical_iteration_log_line(i, headings, total_lines=len(lines)):
            continue
        for pat in STALE_STATUS_PATTERNS:
            if pat.match(line.strip()):
                drifts.append(
                    f"verify_status_pending_verdict: stale Status at line {i}: '{line.strip()[:120]}'"
                )
                break
        # Also flag generic "Status: IN PROGRESS" without sync indicator outside iteration log
        m = TS_STATUS_RE.match(line.strip())
        if m:
            # F7 fix: detect TS-X heading context (= TS-N or TS-pre-N) within 8 preceding lines
            in_ts_task = False
            for j in range(max(1, i - 8), i):
                jl = lines[j - 1] if j - 1 < len(lines) else ""
                if TS_HEADING_RE.match(jl):
                    in_ts_task = True
                    break
                if re.match(r"^### Iteration v\d+", jl):
                    in_ts_task = True  # historical iteration entry
                    break

            if in_ts_task:
                # F7 fix: even in TS-X context, flag if Status contains post-v15 wording
                # (= late-stage stale claim, not legitimate early-stage spec authoring)
                if POST_V15_WORDING_RE.search(line):
                    drifts.append(
                        f"verify_status_pending_verdict (F7 fix): TS-X Status at line {i} "
                        f"contains post-v15 wording (= stale late-stage claim, not legitimate "
                        f"early-stage spec): '{line.strip()[:120]}'"
                    )
            else:
                # Outside TS-X / Iteration context: flag bare Status (original behavior)
                drifts.append(
                    f"verify_status_pending_verdict: bare Status='{m.group(1)}' at line {i} "
                    f"(not in TS-X task / iteration entry context)"
                )
    return drifts


# ---------------------------------------------------------------------------
# Axis 3: verify_label_namespace_collision (Cell 28 / v13-5)
# ---------------------------------------------------------------------------

# Known label namespaces and their semantic referents
LABEL_NAMESPACES = {
    # prefix → expected referent class
    "R-": ("candidate ID (R-1 = Cartesian product completeness, R-5 = Spec gap PRD procedure)",),
    "C-": ("Convergence final rule label (C-1 Critical=0 / C-2 High=0 / C-3 trajectory diminishing / C-4 meta-finding ratio)",),
    "M-": ("Hybrid mechanism label (M-1 Convergence criterion / M-2 Diminishing returns / M-3 Meta-finding tracking)",),
    "T1-": ("Implementation Stage Task (T1-X = audit script extension)",),
    "T2-": ("Implementation Stage Task (T2-X = rule wording strengthening)",),
    "T3-": ("Implementation Stage Task (T3-X = procedure step addition)",),
    "T4-": ("Implementation Stage Task (T4-X = skill workflow integration)",),
    "T5-": ("Implementation Stage Task (T5-X = command workflow integration)",),
    "INV-": ("Invariant ID (INV-1 〜 INV-5)",),
    "F-": ("Iteration finding ID (F1, F2, ... in iteration entries)",),
}

R_FINAL_RULE_RE = re.compile(r"\bR-[1-4]\b\s+(Critical|High|trajectory|meta-finding|Third-party)")

# Axis 3 extension (Path B split 2026-05-11): cell-slot vocabulary fork coverage extension
# Detect mixed canonical naming for the same concept (= cell # canonical identifier).
# Vocabulary fork = multiple identifiers used for same concept in CURRENT spec sections =
# single-source-of-truth violation per Cell 28 v13-5.
#
# **Narrow detection scope (= avoid over-detection)**: only flag identifier-level fork:
# - "cell-slot N" / "cell-slot #N" pattern (= numeric identifier 用法 = canonical 違反)
# - Descriptive uses like "cell-slot occurrence" / "cell-slot vocabulary fork" are
#   legitimate (= concept descriptor, not identifier)
CELL_SLOT_AS_IDENTIFIER_RE = re.compile(r"\bcell-slot\s+#?\d+\b")


def verify_label_namespace_collision(
    lines: list[str], headings: list[HeadingEntry]
) -> list[str]:
    """Cell 28 (v13-5) audit: detect namespace collision + cell-slot vocabulary fork.

    **Axis 3 extension integrated (Path B split 2026-05-11)**:
    Cell-slot vocabulary fork coverage extension (= Cell 28 v13-5 single-source-of-truth =
    matrix # canonical identifier per PRD)。Detect 以下 vocabulary fork drift:
    - "cell-slot occurrence" 使用 (= multi-layer slot 表現) と "cell #" canonical の併用
      は legitimate (= cross-cutting cells multi-layer slot 概念表現)
    - "cell-slot" alone (= "cell-slot occurrence" 以外の用法) は vocabulary fork drift
      candidate (= matrix cell # との semantic relation 不明確)
    - 既存 R-x final-rule reuse 検出 (= post-v10 F1 fix で C-x rename 後の regression detect)
    """
    drifts: list[str] = []

    # Specific check: post-v10 F1 fix renamed R-x final-rule → C-x. Any R-x with final-rule
    # context (Critical / High / trajectory / meta-finding) in CURRENT spec sections is a regression.
    for i, line in enumerate(lines, start=1):
        if is_historical_iteration_log_line(i, headings, total_lines=len(lines)):
            continue  # preservation policy
        if R_FINAL_RULE_RE.search(line):
            drifts.append(
                f"verify_label_namespace_collision: R-x final-rule reuse at line {i} (post-v10 should use C-x): "
                f"'{line.strip()[:120]}'"
            )

    # Axis 3 extension: cell-slot identifier-level vocabulary fork detection
    # Pattern: "cell-slot N" / "cell-slot #N" = identifier-level fork (= canonical 違反)
    # Descriptive uses (= "cell-slot occurrence", "cell-slot vocabulary fork") are legitimate
    # concept descriptors, not flagged.
    for i, line in enumerate(lines, start=1):
        if is_historical_iteration_log_line(i, headings, total_lines=len(lines)):
            continue
        for m in CELL_SLOT_AS_IDENTIFIER_RE.finditer(line):
            drifts.append(
                f"verify_label_namespace_collision (Axis 3 extension): cell-slot used as "
                f"identifier '{m.group(0)}' at line {i} (= matrix cell # canonical 違反、"
                f"single-source-of-truth principle 適用要)"
            )
            break  # one drift per line is sufficient
    return drifts


# ---------------------------------------------------------------------------
# Axis 4: verify_external_file_drift (Cell 17 / v11-5)
# ---------------------------------------------------------------------------

# Pattern: "X (NNNN bytes / ~NNNN 行)" or "(NNN 行)" or "X 行"
EXTERNAL_FILE_LINE_CLAIM_RE = re.compile(
    r"`([A-Za-z0-9_./-]+\.(?:md|py|rs|yml|yaml|sh|toml|json))`(?:\s*\([^)]*?(\d{2,5})\s*行)?"
)
# A7 fix (Path B split 2026-05-11、/check_job L1-4): byte count limit を 9-digit → 12-digit
# (= max 999,999,999,999 bytes ~ 1 TB) に拡張。Practical safety margin、large LFS files /
# generated artifacts も cover。
IMPACT_AREA_BYTES_RE = re.compile(
    r"\|\s*`([A-Za-z0-9_./-]+\.(?:md|py|rs|yml|yaml|toml|json))`\s*\|\s*[^|]*\|\s*(\d{3,12})\b"
)


def find_repo_root(start_path: Path) -> Path:
    """Locate repo root by walking up looking for Cargo.toml marker.

    **Robustness vs `start_path.parent.parent`**: works regardless of PRD location
    (backlog/<prd>.md / tests/fixtures/<group>/<prd>.md / etc.) — needed for test
    fixture-based positive/negative testing of Axis 4 (= Path B split 2026-05-11 fix).
    """
    current = start_path.resolve().parent
    while current != current.parent:
        if (current / "Cargo.toml").exists():
            return current
        current = current.parent
    # Fallback to original heuristic (= 2 levels up, backlog/<prd>.md compatibility)
    return start_path.parent.parent


def verify_external_file_drift(
    lines: list[str], headings: list[HeadingEntry], prd_path: Path
) -> list[str]:
    """Cell 17 (v11-5) audit: Impact Area table claim vs actual wc -l / stat cross-check."""
    drifts: list[str] = []
    repo_root = find_repo_root(prd_path)
    impact_section = find_section_range(headings, r"^Impact Area Audit Findings", level=2, total_lines=len(lines))
    if impact_section is None:
        return drifts

    for i in range(impact_section.start, impact_section.end):
        line = lines[i - 1] if i - 1 < len(lines) else ""
        # Match Impact Area table rows: | `path` | status | bytes | mtime | verify |
        m = IMPACT_AREA_BYTES_RE.search(line)
        if m:
            file_rel = m.group(1)
            claimed_bytes = int(m.group(2))
            file_path = repo_root / file_rel
            if file_path.exists():
                try:
                    actual_bytes = file_path.stat().st_size
                except OSError:
                    continue
                if abs(actual_bytes - claimed_bytes) > 100:
                    drifts.append(
                        f"verify_external_file_drift: line {i} '{file_rel}' claims {claimed_bytes} bytes, "
                        f"actual {actual_bytes} bytes (drift {actual_bytes - claimed_bytes:+d})"
                    )
    return drifts


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: verify_prd_self_audits.py <prd_path>", file=sys.stderr)
        return 2

    prd_path = Path(sys.argv[1])
    if not prd_path.exists():
        print(f"PRD path not found: {prd_path}", file=sys.stderr)
        return 2

    text = prd_path.read_text(encoding="utf-8")
    lines = text.splitlines()
    headings = parse_headings(lines)

    all_drifts: list[tuple[str, list[str]]] = []
    for axis_name, axis_fn in [
        ("Axis 1 (Cell 10 cross-reference cell consistency)", verify_cross_reference_cell_consistency),
        ("Axis 2 (Cell 6+8 status pending verdict)", verify_status_pending_verdict),
        ("Axis 3 (Cell 28 label namespace collision)", verify_label_namespace_collision),
    ]:
        drifts = axis_fn(lines, headings)
        all_drifts.append((axis_name, drifts))
    # Axis 4 needs prd_path
    drifts = verify_external_file_drift(lines, headings, prd_path)
    all_drifts.append(("Axis 4 (Cell 17 external file drift)", drifts))

    total = sum(len(d) for _, d in all_drifts)
    print(f"PRD: {prd_path}")
    print(f"Headings: {len(headings)}")
    print(f"Total drifts (CURRENT spec sections only, HISTORICAL excluded per preservation policy): {total}")
    print()
    for axis_name, drifts in all_drifts:
        print(f"=== {axis_name}: {len(drifts)} drifts ===")
        for d in drifts:
            print(f"  {d}")
        print()

    return 1 if total > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
