#!/usr/bin/env python3
"""audit-handoff-doc-line-refs.py — Handoff doc line-ref drift detection
(PRD I-D-pre Cell 3 v11-5、Phase 4 T1-pre-3a)

================================================================================
Formal lock-in metadata (PRD I-D-pre Cell 3 / v11-5、Path B split adoption 2026-05-11)
================================================================================

**Status**: Formal audit utility (NEW、PRD I-D-pre Implementation Phase 4 T1-pre-3a で新設)。

**Purpose**: Detect line-ref drift in handoff docs (= `doc/handoff/*.md`) by extracting
`<path>:<line>` cross-references and verifying (a) file existence + (b) line number in
bounds + (c) bare basename disambiguation. Cross-references that point to non-existent
files, out-of-bounds lines, or ambiguous bare basenames are reported as drifts.

**Coverage scope (regression-tested via tests/i_d_pre_handoff_audit_test.rs)**:
- code-level `<path>:<line>` and `<path>:<start>-<end>` cross-references
- supported file extensions: `.rs` / `.md` / `.py` / `.sh` / `.yml` / `.yaml` / `.toml` / `.json`
- Out of scope: heading-based PRD-internal line refs (= sibling utility
  `scripts/verify_line_refs.py` covers those)

**PRD I-D-pre binding**:
- Cell 3 (v11-5): NEW audit script + CI integration = handoff doc cross-reference
  structural automated detection
- Test contract: `tests/i_d_pre_handoff_audit_test.rs::test_audit_handoff_doc_line_refs_drift_detection`
  + `test_audit_handoff_doc_line_refs_standalone_baseline`
- CI integration: `.github/workflows/ci.yml` (T1-pre-3b) で `python3
  scripts/audit-handoff-doc-line-refs.py doc/handoff/` を PR merge gate 化

================================================================================
Usage
================================================================================

    python3 scripts/audit-handoff-doc-line-refs.py <path>

`<path>` may be a single `.md` file or a directory (recursively walks `*.md`).

Exit codes:
    0 = no drifts detected
    1 = drifts detected (= MISSING_FILE / OUT_OF_BOUNDS / AMBIGUOUS)
    2 = invocation error (path not found, etc.)

================================================================================
Drift categories
================================================================================

- **INVALID_RANGE**: a range-form ref `<path>:<start>-<end>` has `start > end`
  (= convention violation, e.g., typo `mod.rs:100-50` instead of `50-100`)。
- **MISSING_FILE**: no file matches the referenced path (neither as-is nor via
  glob fallback under common roots).
- **OUT_OF_BOUNDS**: the referenced file exists but the line number exceeds the
  file's total line count.
- **AMBIGUOUS**: a bare basename resolves to >1 in-bounds candidate file; the
  author must qualify the path explicitly (= section-context-implicit prefix
  pattern is fragile, force structural fix).

Resolution strategy:
    1. As-is: resolve `<path>` relative to repo root.
    2. Glob fallback (only if as-is missing): rglob common roots
       (`src/` / `scripts/` / `tests/` / `.claude/` / `doc/` / `.github/`).
    3. OOB filter: drop candidates whose line count is below the upper claim.
    4. Classify: 0 → MISSING_FILE / OUT_OF_BOUNDS, 1 → VERIFIED, >1 → AMBIGUOUS.

================================================================================
Limitations
================================================================================

- Does not interpret section-heading context (= bare basenames trigger AMBIGUOUS
  to force explicit-path discipline).
- "Line content syntactic verify" is bounds-only (= line within file length);
  the script does not match line content against handoff doc claims.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import NamedTuple

# `<path>.<ext>:<start>(-<end>)?`
# Leading boundary avoids mid-token matches (URL path segments, etc.).
REF_RE = re.compile(
    r"(?<![A-Za-z0-9_])"
    r"(?P<path>[A-Za-z0-9_./-]+\.(?:rs|md|py|sh|yml|yaml|toml|json))"
    r":(?P<start>\d+)(?:-(?P<end>\d+))?"
)

# Common roots searched on glob fallback (when as-is path doesn't exist under repo root).
GLOB_ROOTS = ("src", "scripts", "tests", ".claude", "doc", ".github")


class Ref(NamedTuple):
    handoff: Path
    source_line: int
    path: str
    start: int
    end: int  # = start if single-line ref


class Drift(NamedTuple):
    handoff: Path
    source_line: int
    path: str
    line_spec: str
    category: str  # "INVALID_RANGE" / "MISSING_FILE" / "OUT_OF_BOUNDS" / "AMBIGUOUS"
    detail: str


def find_repo_root() -> Path:
    """Walk up from cwd to find `Cargo.toml` (= repo root marker)."""
    cur = Path.cwd().resolve()
    while cur != cur.parent:
        if (cur / "Cargo.toml").exists():
            return cur
        cur = cur.parent
    return Path.cwd().resolve()


def parse_refs(doc: Path) -> list[Ref]:
    refs: list[Ref] = []
    text = doc.read_text(encoding="utf-8", errors="replace")
    for lineno, line in enumerate(text.splitlines(), start=1):
        for m in REF_RE.finditer(line):
            start = int(m.group("start"))
            end = int(m.group("end")) if m.group("end") else start
            refs.append(
                Ref(handoff=doc, source_line=lineno, path=m.group("path"), start=start, end=end)
            )
    return refs


def file_line_count(p: Path) -> int:
    with p.open(encoding="utf-8", errors="replace") as f:
        return sum(1 for _ in f)


def collect_glob_candidates(path_str: str, repo_root: Path) -> list[Path]:
    """rglob common roots for `path_str`; returns deduped absolute Paths."""
    seen: set[Path] = set()
    out: list[Path] = []
    for root in GLOB_ROOTS:
        base = repo_root / root
        if not base.is_dir():
            continue
        for c in base.rglob(path_str):
            if c.is_file():
                resolved = c.resolve()
                if resolved not in seen:
                    seen.add(resolved)
                    out.append(resolved)
    return out


def classify_ref(ref: Ref, repo_root: Path) -> Drift | None:
    """Return Drift if ref is a drift; None if VERIFIED."""
    line_spec = f"{ref.start}-{ref.end}" if ref.end != ref.start else str(ref.start)

    # Backwards-range detection (= invalid input format precedes bounds check
    # since `upper = ref.end` would otherwise silently underestimate when
    # author wrote `start > end` by typo)。
    if ref.end < ref.start:
        return Drift(
            ref.handoff, ref.source_line, ref.path, line_spec,
            "INVALID_RANGE",
            f"backwards range: start={ref.start} > end={ref.end}; "
            "convention requires start <= end",
        )

    upper = ref.end

    as_is = repo_root / ref.path
    if as_is.exists() and as_is.is_file():
        lc = file_line_count(as_is)
        if upper > lc:
            return Drift(
                ref.handoff, ref.source_line, ref.path, line_spec,
                "OUT_OF_BOUNDS",
                f"as-is file has {lc} lines, claim ends at {upper}",
            )
        return None  # VERIFIED

    # As-is missing; try glob fallback
    raw = collect_glob_candidates(ref.path, repo_root)
    if not raw:
        return Drift(
            ref.handoff, ref.source_line, ref.path, line_spec,
            "MISSING_FILE",
            f"no file matches '{ref.path}' as-is or under {list(GLOB_ROOTS)}",
        )

    in_bounds = [c for c in raw if file_line_count(c) >= upper]
    if not in_bounds:
        # All glob candidates OOB
        sample = raw[0]
        return Drift(
            ref.handoff, ref.source_line, ref.path, line_spec,
            "OUT_OF_BOUNDS",
            f"all {len(raw)} glob candidate(s) below line {upper} "
            f"(e.g., {sample.relative_to(repo_root)} has {file_line_count(sample)} lines)",
        )
    if len(in_bounds) > 1:
        rels = sorted(str(c.relative_to(repo_root)) for c in in_bounds)
        return Drift(
            ref.handoff, ref.source_line, ref.path, line_spec,
            "AMBIGUOUS",
            f"{len(in_bounds)} in-bounds candidate(s): {rels}; qualify path explicitly",
        )
    return None  # exactly 1 in-bounds candidate = VERIFIED


def audit(doc: Path, repo_root: Path) -> tuple[int, list[Drift]]:
    refs = parse_refs(doc)
    drifts: list[Drift] = [d for d in (classify_ref(r, repo_root) for r in refs) if d is not None]
    return len(refs), drifts


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: audit-handoff-doc-line-refs.py <path>", file=sys.stderr)
        print("  <path> = .md file or directory (recursively walked for *.md)", file=sys.stderr)
        return 2

    target = Path(sys.argv[1])
    if not target.exists():
        print(f"path not found: {target}", file=sys.stderr)
        return 2

    repo_root = find_repo_root()
    docs = sorted(target.rglob("*.md")) if target.is_dir() else [target]

    total_refs = 0
    all_drifts: list[Drift] = []
    for doc in docs:
        n, drifts = audit(doc, repo_root)
        total_refs += n
        all_drifts.extend(drifts)

    print(f"Handoff docs scanned: {len(docs)}")
    print(f"Line refs found: {total_refs}")
    print(f"Total drifts: {len(all_drifts)}")

    if all_drifts:
        print()
        print("=== Detected drifts ===")
        for d in all_drifts:
            try:
                rel = d.handoff.relative_to(repo_root)
            except ValueError:
                rel = d.handoff
            print(f"{rel}:{d.source_line}: [{d.category}] {d.path}:{d.line_spec}", file=sys.stderr)
            print(f"  {d.detail}", file=sys.stderr)
        return 1

    print("PASS: No drifts detected.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
