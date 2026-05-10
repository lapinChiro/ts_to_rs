#!/usr/bin/env python3
"""verify_prd_self_audits.py

Path E bootstrap utility for PRD I-D Spec stage convergence (Iteration v15 → v16 transition).

Implements 4 audit axes that complement `verify_line_refs.py` (Method A = Cell 19 v11-7) for
multi-axis bootstrap of v12-2 / v11-7 / v13-5 / v3-6 / v4-2 / v5-1 / v11-5 self-applied gap
classes (= post-v15 NEW dominant defect classes empirically demonstrated):

1. verify_cross_reference_cell_consistency (Cell 10 / v5-1)
   = matrix vs Scope vs Mapping vs Test Plan で cell # appearance consistency
2. verify_status_pending_verdict (Cell 6+8 / v3-6 / v4-2)
   = current spec section の status field staleness detect
3. verify_label_namespace_collision (Cell 28 / v13-5)
   = namespace prefix (R-x / C-x / M-x / etc.) の multi-referent collision detect
4. verify_external_file_drift (Cell 17 / v11-5)
   = Impact Area table claim vs actual wc -l / stat cross-check

Usage:
    python3 scripts/verify_prd_self_audits.py <prd_path>

Output: list of detected drifts grouped by audit axis.

Exit codes:
    0 = all axes PASS (no CURRENT spec drifts)
    1 = at least one CURRENT spec drift detected (HISTORICAL drifts excluded per preservation policy)
    2 = invocation error (missing argument / file not found)
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


def find_section_range(headings: list[HeadingEntry], title_pattern: str, level: int = 2) -> SectionRange | None:
    """Find the line range of a section by heading title pattern (substring match)."""
    rx = re.compile(title_pattern)
    for i, h in enumerate(headings):
        if h.level == level and rx.search(h.title):
            # End at next heading of same or higher level
            end = len(headings) and headings[-1].line + 1
            for j in range(i + 1, len(headings)):
                if headings[j].level <= level:
                    end = headings[j].line
                    break
            return SectionRange(title=h.title, level=level, start=h.line, end=end)
    return None


def is_historical_iteration_log_line(line_num: int, headings: list[HeadingEntry]) -> bool:
    """Determine if a line falls inside a `### Iteration v*` historical entry block."""
    iteration_log_root = find_section_range(headings, r"^Spec Review Iteration Log", level=2)
    if iteration_log_root is None:
        return False
    return iteration_log_root.start <= line_num < iteration_log_root.end


# ---------------------------------------------------------------------------
# Axis 1: verify_cross_reference_cell_consistency (Cell 10 / v5-1)
# ---------------------------------------------------------------------------

CELL_NUM_RANGE_RE = re.compile(r"\b(\d{1,2})\s*[-–]\s*(\d{1,2})\b")
CELL_LIST_RE = re.compile(r"cells?\s+([\d,\s\-–/]+?)(?=[)。、]|\s*=|\s*\(|$)")


def expand_cell_list(text: str) -> set[int]:
    """Extract cell numbers from a phrase like 'cells 1, 4, 5, 6, 7-9, 12'."""
    cells: set[int] = set()
    # Find all "cells N..." patterns
    for m in CELL_LIST_RE.finditer(text):
        body = m.group(1)
        # Handle ranges
        for r in CELL_NUM_RANGE_RE.finditer(body):
            lo, hi = int(r.group(1)), int(r.group(2))
            if 1 <= lo <= hi <= 30:
                cells.update(range(lo, hi + 1))
        # Handle individual numbers (after stripping ranges)
        body_no_range = CELL_NUM_RANGE_RE.sub(" ", body)
        for num_match in re.finditer(r"\b(\d{1,2})\b", body_no_range):
            n = int(num_match.group(1))
            if 1 <= n <= 30:
                cells.add(n)
    return cells


def collect_section_cells(lines: list[str], section: SectionRange) -> set[int]:
    """Collect all cell numbers mentioned in a given section's body."""
    body = "\n".join(lines[section.start - 1 : section.end - 1])
    return expand_cell_list(body)


def collect_matrix_cells(lines: list[str], headings: list[HeadingEntry]) -> set[int]:
    """Collect cell # values from the canonical matrix table in `## Problem Space > 組合せマトリクス`."""
    matrix_section = find_section_range(headings, r"^Problem Space", level=2)
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
                if 1 <= n <= 30:
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
    """Cell 10 (v5-1) audit: matrix vs Scope vs Mapping vs Test Plan で cell # appearance consistency."""
    drifts: list[str] = []
    matrix_cells = collect_matrix_cells(lines, headings)
    if len(matrix_cells) != 30:
        drifts.append(
            f"verify_cross_reference_cell_consistency: matrix cells extracted = {len(matrix_cells)} ≠ 30 expected"
        )
        return drifts

    sections_to_check = [
        ("Scope (In Scope)", r"^Scope$"),
        ("Test Plan", r"^Test Plan"),
    ]
    for label, pattern in sections_to_check:
        section = find_section_range(headings, pattern, level=2)
        if section is None:
            drifts.append(f"verify_cross_reference_cell_consistency: section '{label}' not found")
            continue
        section_cells = collect_section_cells(lines, section)
        # Filter cells outside 1-30 (false positives like "30 cells", "12 candidates" etc)
        # by comparing to matrix_cells overlap
        relevant_cells = section_cells & set(range(1, 31))
        missing = matrix_cells - relevant_cells
        # Tolerate sections that mention only a subset (e.g., Scope partition by Layer)
        if len(missing) > 5:
            drifts.append(
                f"verify_cross_reference_cell_consistency: section '{label}' missing cells "
                f"{sorted(missing)} (likely scope-partition; review needed)"
            )
    return drifts


# ---------------------------------------------------------------------------
# Axis 2: verify_status_pending_verdict (Cell 6+8 / v3-6 / v4-2)
# ---------------------------------------------------------------------------

STALE_STATUS_PATTERNS = [
    re.compile(r"^\*\*Status\*\*:\s*Spec stage Iteration v1\b"),  # top-level frontmatter stale
    re.compile(
        r"^- \*\*Status\*\*:\s*IN PROGRESS\s*\(=[^)]*Iteration v\d+\s*で convergence verify[^)]*\)$"
    ),  # forward-reference IN PROGRESS in current sections
]
TS_STATUS_RE = re.compile(r"^- \*\*Status\*\*:\s*(IN PROGRESS|PENDING|PARTIAL|TBD)\b")


def verify_status_pending_verdict(
    lines: list[str], headings: list[HeadingEntry]
) -> list[str]:
    """Cell 6+8 (v3-6 / v4-2) audit: current spec section の status field staleness detect."""
    drifts: list[str] = []
    for i, line in enumerate(lines, start=1):
        # Skip historical iteration log entries (preservation policy)
        if is_historical_iteration_log_line(i, headings):
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
            # Only flag if not in TS-X task headings (which legitimately have IN PROGRESS during work)
            # Quick heuristic: if line is preceded (within 5 lines) by `### TS-` heading, allow
            in_ts_task = False
            for j in range(max(1, i - 8), i):
                jl = lines[j - 1] if j - 1 < len(lines) else ""
                if re.match(r"^### TS-\d+", jl):
                    in_ts_task = True
                    break
                if re.match(r"^### Iteration v\d+", jl):
                    in_ts_task = True  # historical iteration entry
                    break
            if not in_ts_task:
                drifts.append(
                    f"verify_status_pending_verdict: bare Status='{m.group(1)}' at line {i} (not in TS-X task / iteration entry context)"
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


def verify_label_namespace_collision(
    lines: list[str], headings: list[HeadingEntry]
) -> list[str]:
    """Cell 28 (v13-5) audit: detect namespace collision (e.g., R-x reused for both candidate ID + final rule label)."""
    drifts: list[str] = []
    # Specific check: post-v10 F1 fix renamed R-x final-rule → C-x. Any R-x with final-rule
    # context (Critical / High / trajectory / meta-finding) in CURRENT spec sections is a regression.
    for i, line in enumerate(lines, start=1):
        if is_historical_iteration_log_line(i, headings):
            continue  # preservation policy
        if R_FINAL_RULE_RE.search(line):
            drifts.append(
                f"verify_label_namespace_collision: R-x final-rule reuse at line {i} (post-v10 should use C-x): "
                f"'{line.strip()[:120]}'"
            )
    return drifts


# ---------------------------------------------------------------------------
# Axis 4: verify_external_file_drift (Cell 17 / v11-5)
# ---------------------------------------------------------------------------

# Pattern: "X (NNNN bytes / ~NNNN 行)" or "(NNN 行)" or "X 行"
EXTERNAL_FILE_LINE_CLAIM_RE = re.compile(
    r"`([A-Za-z0-9_./-]+\.(?:md|py|rs|yml|yaml|sh|toml|json))`(?:\s*\([^)]*?(\d{2,5})\s*行)?"
)
IMPACT_AREA_BYTES_RE = re.compile(
    r"\|\s*`([A-Za-z0-9_./-]+\.(?:md|py|rs|yml|yaml))`\s*\|\s*[^|]*\|\s*(\d{3,7})\b"
)


def verify_external_file_drift(
    lines: list[str], headings: list[HeadingEntry], prd_path: Path
) -> list[str]:
    """Cell 17 (v11-5) audit: Impact Area table claim vs actual wc -l / stat cross-check."""
    drifts: list[str] = []
    repo_root = prd_path.parent.parent  # backlog/<prd>.md → repo root
    impact_section = find_section_range(headings, r"^Impact Area Audit Findings", level=2)
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
