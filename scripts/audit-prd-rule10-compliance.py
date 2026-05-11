#!/usr/bin/env python3
"""PRD 2.7 — Rule 10/11/12 + Rule 4 (4-3) compliance audit.

PRD doc に対し以下 2 audit を実施:
  1. Rule 12 application section 検証 (Q5 application、`## Rule 10 Application`
     section + machine-parseable yaml format + Permitted/Prohibited keywords)
  2. Rule 4 (4-3) doc-first dependency order 検証 (Q6 application、Task List で
     code 改修 task の Prerequisites に doc update task ID 存在 verify)

Usage:
    python3 scripts/audit-prd-rule10-compliance.py [PRD_PATH ...] [--verbose]

PRD_PATH: 監査対象 PRD doc (markdown)。複数指定可。省略時は backlog/ 配下で
**`## Rule 10 Application` section を含む全 PRD doc** (= post-PRD-2.7 framework
適用 PRD) を auto-detect。File name prefix (`PRD-*` / `I-*`) は判定基準にしない
(= PRD 2.7 完了後の新 PRD は I-* prefix で命名されているため、prefix-based
filter では false-skip が発生する。`## Rule 10 Application` section が
post-PRD-2.7 framework の mandatory marker であることを利用した content-based
auto-detect が structural な解決)。

Exit code:
    0: audit pass (全 PRD compliance OK)
    非 0: audit fail (compliance 違反 detected、または auto-detect で audit 対象 0 件)

Dependencies:
    pip install pyyaml
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

import yaml

# PRD I-D-pre Phase 3 (T1-pre-1 + T1-pre-2 + T1-pre-4) source-of-truth import:
# Path E utility (`scripts/verify_prd_self_audits.py`) の verify functions を本 audit
# script 側 mirror として library import 経由で共有 (= DRY、single source of truth)。
# Path E utility 改善 (= F6/F7 fix + Axis 3 cell-slot extension 等) が audit script
# 側に auto sync する。Mirror wrapper 関数 (= `verify_pending_verdict_findings_consistency`
# + `verify_cross_reference_cell_consistency` + `verify_cell_numbering_drift_detection`)
# は Option α auto-detect (= `has_cell_numbering_convention_section()` early-return) で
# I-205 audit out-of-scope に自動分類、INV-4 4-tuple baseline preserve。
#
# Note: Python が `python3 script.py` 実行時に script directory を自動的に sys.path[0]
# に追加するため、同 directory 内の `verify_prd_self_audits` への top-level import は
# 自然に解決される (= sys.path.insert は redundant、PEP 8 compliant top-level import)。
from verify_prd_self_audits import (
    parse_headings as _parse_headings,
    verify_cross_reference_cell_consistency as _path_e_verify_cross_reference,
    verify_label_namespace_collision as _path_e_verify_label_namespace,
    verify_status_pending_verdict as _path_e_verify_status_pending,
)

REPO_ROOT = Path(__file__).resolve().parent.parent
BACKLOG_DIR = REPO_ROOT / "backlog"

# post-PRD-2.7 framework の mandatory section marker。`## Rule 10 Application`
# section の存在で matrix-driven (= 本 audit 対象) PRD を判定する (file name prefix
# ではなく content で判定するための structural marker、`spec-stage-adversarial-checklist.md`
# Rule 12 (e-5) `## Rule 10 Application` heading 必須 spec を参照)。
MATRIX_DRIVEN_MARKER = "## Rule 10 Application"


def is_matrix_driven_prd(path: Path) -> bool:
    """Returns True if the PRD doc contains the `## Rule 10 Application` section
    (= post-PRD-2.7 framework marker).

    Used by the default discovery mode (no PRD_PATH args) to auto-select audit
    targets from `backlog/`. Legacy partial-framework PRDs (e.g., `I-050`
    umbrella that uses `## Problem Space` but predates the Rule 12 mandatory
    section) are skipped because they would unconditionally fail this audit.
    """
    try:
        content = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return False
    return any(
        line.strip() == MATRIX_DRIVEN_MARKER
        for line in content.splitlines()
    )

# Q5 確定: matrix 不在 PRD で許容される structural reason (Permitted reasons)
PERMITTED_REASONS_KEYWORDS = (
    "infra",
    "AST input dimension irrelevant",
    "refactor",
    "機能 emission decision なし",
    "pure doc 改修",
)

# Q5 確定: 明示禁止 keyword (Prohibited reasons、`feedback_no_dev_cost_judgment.md` 違反)
PROHIBITED_KEYWORDS = (
    "scope 小",
    "scope 狭",
    "scope 限",
    "light spec",
    "pragmatic",
    "loc",  # case-insensitive で matches "LOC" も
    "短時間",
    "短期間",
    "manageable",
    "effort 大",
    "実装 trivial",
    "quick",
    "easy",
    "simple",
)

# Doc update task identification keywords (Rule 4 (4-3) verification 用)
DOC_UPDATE_KEYWORDS = (
    "ast-variants.md",
    "rust-type-variants.md",
    "emission-contexts.md",
    "doc/grammar/",
    "reference doc",
    "single source of truth",
)

# Code 改修 task identification keywords (Rule 4 (4-3) verification 用)
CODE_CHANGE_KEYWORDS = (
    "src/",
    "TypeResolver",
    "Transformer",
    "Generator",
    "convert_",
    "visit_",
    "resolve_",
)

# Pure audit/investigation task title keywords (= code 改修対象から除外)
# 例: "Decorator dispatch audit" は src/ を grep するが code 改修なし、Rule 4 (4-3) 対象外
AUDIT_TASK_TITLE_KEYWORDS = (
    "audit",
    "report",
    "investigation",
    "self-applied verification",
    "/check_job",
    "review",
    "quality check",
)


@dataclass
class TaskEntry:
    task_id: str  # 例: "T11"
    title: str
    work: str  # Work field の content
    depends_on: list[str]  # Depends on の task IDs
    prerequisites: list[str]  # Prerequisites の task IDs (line content)


def parse_rule10_section(content: str) -> tuple[dict | None, str | None]:
    """PRD doc の `## Rule 10 Application` section + 内部 yaml fenced code block を parse。

    Returns:
        (parsed_yaml_dict, error_message)
        section 不在 → (None, error)
        yaml parse fail → (None, error)
        success → (dict, None)
    """
    heading_pattern = re.compile(
        r"^##\s+Rule 10 Application\s*$", re.MULTILINE
    )
    m = heading_pattern.search(content)
    if not m:
        return None, "missing `## Rule 10 Application` heading"

    # heading から次の `## ` heading 直前まで section content
    section_start = m.end()
    next_heading = re.search(r"^##\s", content[section_start:], re.MULTILINE)
    section_end = (
        section_start + next_heading.start()
        if next_heading
        else len(content)
    )
    section_content = content[section_start:section_end]

    # fenced code block (```yaml ... ```) を抽出
    code_block_pattern = re.compile(
        r"```yaml\s*\n(.*?)\n```", re.DOTALL
    )
    cb_match = code_block_pattern.search(section_content)
    if not cb_match:
        return None, "missing yaml fenced code block in Rule 10 Application section"

    yaml_text = cb_match.group(1)
    try:
        data = yaml.safe_load(yaml_text)
    except yaml.YAMLError as e:
        return None, f"yaml parse error: {e}"

    if not isinstance(data, dict):
        return None, "yaml content is not a mapping"

    return data, None


def verify_rule10_application(prd_path: Path, content: str) -> list[str]:
    """1 PRD の Rule 10 Application section を verify。"""
    violations: list[str] = []
    data, err = parse_rule10_section(content)
    if err is not None:
        violations.append(f"{prd_path.name}: {err}")
        return violations

    # Note: PyYAML 1.1 では `yes`/`no` を bool True/False に自動変換。
    # 本 audit は yes/no string と True/False bool の両方を accept。
    def normalize_yes_no(v) -> str | None:
        if v is True or v == "yes":
            return "yes"
        if v is False or v == "no":
            return "no"
        return None

    matrix_driven = normalize_yes_no(data.get("Matrix-driven"))
    if matrix_driven is None:
        violations.append(
            f"{prd_path.name}: `Matrix-driven` value must be yes/no, got "
            f"{data.get('Matrix-driven')!r}"
        )
        return violations

    cross_axis = normalize_yes_no(data.get("Cross-axis orthogonal direction enumerated"))
    if cross_axis is None:
        violations.append(
            f"{prd_path.name}: `Cross-axis orthogonal direction enumerated` value "
            f"must be yes/no, got "
            f"{data.get('Cross-axis orthogonal direction enumerated')!r}"
        )

    if matrix_driven == "yes":
        axes = data.get("Rule 10 axes enumerated")
        if not isinstance(axes, list) or len(axes) == 0:
            violations.append(
                f"{prd_path.name}: `Rule 10 axes enumerated` must be a non-empty list "
                f"for matrix-driven PRD, got {axes!r}"
            )
        if cross_axis != "yes":
            violations.append(
                f"{prd_path.name}: matrix-driven PRD must have "
                f"`Cross-axis orthogonal direction enumerated: yes`"
            )

    if matrix_driven == "no":
        reason = data.get("Structural reason for matrix absence", "")
        reason_lower = (reason or "").lower()
        # Prohibited keyword check (case-insensitive substring)
        for kw in PROHIBITED_KEYWORDS:
            if kw.lower() in reason_lower:
                violations.append(
                    f"{prd_path.name}: `Structural reason for matrix absence` contains "
                    f"prohibited keyword '{kw}' (Anti-pattern、"
                    f"`feedback_no_dev_cost_judgment.md` 違反)。reason={reason!r}"
                )

    return violations


def parse_task_list(content: str) -> list[TaskEntry]:
    """PRD doc の `## Task List` section を parse、task entries を抽出。"""
    tl_match = re.search(r"^##\s+Task List\s*$", content, re.MULTILINE)
    if not tl_match:
        return []

    section_start = tl_match.end()
    next_top_heading = re.search(
        r"^##\s+(?!##)", content[section_start:], re.MULTILINE
    )
    section_end = (
        section_start + next_top_heading.start()
        if next_top_heading
        else len(content)
    )
    section_content = content[section_start:section_end]

    # `### TN: title` heading で task entry 分割
    task_heading_pattern = re.compile(
        r"^###\s+(T[\w.]+):\s*(.+?)$", re.MULTILINE
    )
    matches = list(task_heading_pattern.finditer(section_content))
    tasks: list[TaskEntry] = []
    for i, m in enumerate(matches):
        task_id = m.group(1)
        title = m.group(2).strip()
        body_start = m.end()
        body_end = (
            matches[i + 1].start() if i + 1 < len(matches) else len(section_content)
        )
        body = section_content[body_start:body_end]

        # Work field 抽出
        work_match = re.search(r"\*\*Work\*\*:\s*(.*?)(?=\*\*\w|$)", body, re.DOTALL)
        work = work_match.group(1).strip() if work_match else ""

        # Depends on 抽出 (line 形式)
        depends_match = re.search(
            r"\*\*Depends on\*\*:\s*([^\n]*)", body
        )
        depends_text = depends_match.group(1) if depends_match else ""
        depends_on = re.findall(r"T[\w.]+", depends_text) if depends_text else []

        # Prerequisites 抽出
        prereq_match = re.search(
            r"\*\*Prerequisites\*\*:\s*([^\n]*)", body
        )
        prereq_text = prereq_match.group(1) if prereq_match else ""
        prerequisites = re.findall(r"T[\w.]+", prereq_text) if prereq_text else []
        # Prerequisites field の raw text も保持 (= "なし" / file path 等の non-task content)
        # ここでは task ID list のみ抽出

        tasks.append(
            TaskEntry(
                task_id=task_id,
                title=title,
                work=work,
                depends_on=depends_on,
                prerequisites=prerequisites,
            )
        )

    return tasks


def task_text(task: TaskEntry) -> str:
    """task の identification 用の検索対象 text (title + work) を返す。"""
    return f"{task.title}\n{task.work}".lower()


def is_doc_update_task(task: TaskEntry) -> bool:
    text = task_text(task)
    return any(kw.lower() in text for kw in DOC_UPDATE_KEYWORDS)


def is_code_change_task(task: TaskEntry) -> bool:
    title_lower = task.title.lower()
    # Pure audit/review/report tasks are not code change tasks (false positive 排除)
    if any(kw in title_lower for kw in AUDIT_TASK_TITLE_KEYWORDS):
        return False
    text = task_text(task)
    return any(kw.lower() in text for kw in CODE_CHANGE_KEYWORDS)


def verify_rule4_doc_first(prd_path: Path, content: str) -> list[str]:
    """Rule 4 (4-3) doc-first dependency order を verify。

    各 code 改修 task の Prerequisites + Depends on 集合に、doc update task ID が
    存在することを check。
    """
    violations: list[str] = []
    tasks = parse_task_list(content)
    if not tasks:
        return violations  # Task List 不在 PRD は Rule 4 (4-3) 対象外 (= 一般 PRD でない)

    doc_task_ids = {t.task_id for t in tasks if is_doc_update_task(t)}
    code_tasks = [t for t in tasks if is_code_change_task(t) and not is_doc_update_task(t)]

    if not doc_task_ids:
        # doc update task 不在 = code 改修 task が doc 不要 (= refactor 等) かもしれない
        # この場合 Rule 4 (4-3) は適用外
        return violations

    for code_task in code_tasks:
        deps = set(code_task.depends_on) | set(code_task.prerequisites)
        if not (deps & doc_task_ids):
            violations.append(
                f"{prd_path.name}: Rule 4 (4-3) violation: code change task "
                f"`{code_task.task_id}` ({code_task.title}) lacks prerequisite doc "
                f"update task. Doc tasks: {sorted(doc_task_ids)}. "
                f"`{code_task.task_id}` deps: {sorted(deps)}"
            )

    return violations


def is_active_prd(content: str) -> bool:
    """PRD doc が active (Spec stage / Implementation stage / Draft) か判定。

    Active = 新 framework rules (Rule 1 (1-2) abbreviation / Rule 2 (2-2) oracle /
    Rule 5 (5-2) stage tasks / Rule 6 (6-2) scope 3-tier / Rule 8 (8-5) invariants /
    Rule 11 (d-5) impact area audit findings / Rule 13 spec review log) を enforce。

    Closed PRD (= 既存 grandfathered) は新 rule audit 対象外。
    Status header の有無で判定。
    """
    # Top 50 lines の Status header を検査
    top = "\n".join(content.split("\n")[:50])
    if re.search(r"\*\*Status\*\*[:：]\s*(Closed|完了|Done)", top, re.IGNORECASE):
        return False
    if re.search(
        r"\*\*Status\*\*[:：]\s*(Spec\s*stage|Implementation\s*stage|Draft)",
        top,
        re.IGNORECASE,
    ):
        return True
    # Status header 不在 = 既存 grandfathered PRD として skip
    return False


def is_matrix_driven(content: str) -> bool:
    """PRD doc が matrix-driven か Rule 10 Application yaml から判定。"""
    data, _ = parse_rule10_section(content)
    if data is None:
        return False
    val = data.get("Matrix-driven")
    return val is True or val == "yes"


def has_failure_cells(content: str) -> bool:
    """Problem Space matrix table 内に ✗ or 要調査 cell が存在するか判定。"""
    # `## Problem Space` section 内のみ search
    m = re.search(r"^##\s+Problem Space\s*$", content, re.MULTILINE)
    if not m:
        return False
    section_start = m.end()
    next_h = re.search(r"^##\s+(?!#)", content[section_start:], re.MULTILINE)
    section = (
        content[section_start : section_start + next_h.start()]
        if next_h
        else content[section_start:]
    )
    return ("✗" in section) or ("要調査" in section)


def has_na_cells(content: str) -> bool:
    """Problem Space matrix table 内に NA cell が存在するか判定。"""
    m = re.search(r"^##\s+Problem Space\s*$", content, re.MULTILINE)
    if not m:
        return False
    section_start = m.end()
    next_h = re.search(r"^##\s+(?!#)", content[section_start:], re.MULTILINE)
    section = (
        content[section_start : section_start + next_h.start()]
        if next_h
        else content[section_start:]
    )
    # NA cell: matrix row 内 `| NA |` or `| NA (...)` pattern
    return bool(re.search(r"\|\s*NA(\s|\|)", section))


def get_section(content: str, heading_regex: str) -> str | None:
    """指定 heading 以降、次の同 level heading 直前までの section content を返す。"""
    m = re.search(heading_regex, content, re.MULTILINE)
    if not m:
        return None
    start = m.end()
    next_h = re.search(r"^##\s+(?!#)", content[start:], re.MULTILINE)
    return content[start : start + next_h.start()] if next_h else content[start:]


def verify_rule1_abbreviation_prohibition(prd_path: Path, content: str) -> list[str]:
    """Rule 1 (1-2): matrix abbreviation pattern 全面禁止 (RC-1 source)。"""
    violations: list[str] = []
    section = get_section(content, r"^##\s+Problem Space\s*$")
    if section is None:
        return violations  # Rule 1 audit は Problem Space 不在で skip

    # `...` ellipsis cell (matrix table 行内 `| ... |` pattern)
    ellipsis_pattern = re.compile(r"\|\s*\.\.\.\s*\|")
    for m in ellipsis_pattern.finditer(section):
        violations.append(
            f"{prd_path.name}: Rule 1 (1-2) violation: matrix table contains `...` ellipsis cell "
            f"(abbreviation prohibition)。完全 enumerate 必須"
        )
        break  # 1 つで report 十分

    # Range row number cell (`| 30-35 |` 等の grouping)
    range_pattern = re.compile(r"^\|\s*\d+-\d+\s*\|", re.MULTILINE)
    for m in range_pattern.finditer(section):
        violations.append(
            f"{prd_path.name}: Rule 1 (1-2) violation: matrix table contains range "
            f"row grouping (`{m.group().strip()}`)。各 cell 独立 row 必須"
        )
        break

    # Anti-pattern keywords (Rule 1 (1-2) abbreviation detection)
    # Note (framework v1.4 stance): D 全 / B 全 / Bn-Bm wording は **Rule 10 Step 2
    # orthogonality merge** として legitimate (dispatch logic 同一の場合のみ)。
    # 本 audit は **真の abbreviation** (information hiding without orthogonality
    # justification) のみ detect、orthogonality merge wording は flag しない。
    # B-variant grouping with **divergent dispatch** (e.g., B5=NA / B6=Tier 2 / B8=Tier 1
    # mixed in single row) は **manual review responsibility** (audit script では
    # detect 不能、4-layer review Layer 3 cross-axis verify で catch)。
    anti_keywords = [
        ("(各別 cell)", "(各別 cell)"),
        ("(同上)", "(同上)"),
        (r"\bvaries\b", "varies"),
        (r"\(\.\.\.\s*と同\s*logic\)", "(... と同 logic)"),
        ("代表的", "代表的"),
        ("省略", "省略"),
        (r"\babbreviated\b", "abbreviated"),
        (r"\brepresentative\b", "representative"),
    ]
    for pattern, label in anti_keywords:
        if re.search(pattern, section):
            violations.append(
                f"{prd_path.name}: Rule 1 (1-2) violation: matrix section contains "
                f"abbreviation keyword '{label}' (Anti-pattern)。完全 enumerate 必須"
            )
    return violations


def verify_rule2_oracle_observations(prd_path: Path, content: str) -> list[str]:
    """Rule 2 (2-2/2-3): `## Oracle Observations` section embed mandatory (RC-2 source)。"""
    violations: list[str] = []
    if not has_failure_cells(content):
        return violations  # ✗ / 要調査 cell 不在なら skip
    section = get_section(content, r"^##\s+Oracle Observations\b.*$")
    if section is None:
        violations.append(
            f"{prd_path.name}: Rule 2 (2-2) violation: matrix has ✗/要調査 cells but "
            f"`## Oracle Observations` section is missing"
        )
        return violations
    # Section content が "TBD" or 空 のみなら fail
    stripped = section.strip()
    if not stripped or len(stripped) < 50 or stripped.lower().startswith("tbd"):
        violations.append(
            f"{prd_path.name}: Rule 2 (2-2) violation: `## Oracle Observations` section "
            f"is empty or placeholder"
        )
    return violations


def verify_rule5_stage_tasks_separation(prd_path: Path, content: str) -> list[str]:
    """Rule 5 (5-2/5-4): Task List 2-section split (RC-4 source)。"""
    violations: list[str] = []
    has_spec_stage = bool(re.search(r"^##\s+Spec Stage Tasks\b.*$", content, re.MULTILINE))
    has_impl_stage = bool(
        re.search(r"^##\s+Implementation Stage Tasks\b.*$", content, re.MULTILINE)
    )
    has_legacy_task_list = bool(re.search(r"^##\s+Task List\b.*$", content, re.MULTILINE))
    if not (has_spec_stage and has_impl_stage):
        violations.append(
            f"{prd_path.name}: Rule 5 (5-2/5-4) violation: matrix-driven PRD must have "
            f"both `## Spec Stage Tasks` and `## Implementation Stage Tasks` sections "
            f"(found Spec={has_spec_stage}, Impl={has_impl_stage})"
        )
    if has_legacy_task_list and not (has_spec_stage and has_impl_stage):
        violations.append(
            f"{prd_path.name}: Rule 5 (5-2) violation: legacy `## Task List` section "
            f"detected without 2-section split (use Spec Stage / Implementation Stage)"
        )
    return violations


def verify_rule6_scope_3tier(prd_path: Path, content: str) -> list[str]:
    """Rule 6 (6-2): Scope 3-tier hard-code (RC-5 source)。"""
    violations: list[str] = []
    scope_section = get_section(content, r"^##\s+Scope\b.*$")
    if scope_section is None:
        violations.append(
            f"{prd_path.name}: Rule 6 (6-2) violation: `## Scope` section missing"
        )
        return violations
    # 3-tier 全 sub-heading 確認
    has_in = bool(re.search(r"^###\s+In Scope\b", scope_section, re.MULTILINE))
    has_out = bool(re.search(r"^###\s+Out of Scope\b", scope_section, re.MULTILINE))
    has_tier2 = bool(
        re.search(r"^###\s+Tier 2 honest error reclassify\b", scope_section, re.MULTILINE)
    )
    missing = []
    if not has_in:
        missing.append("`### In Scope`")
    if not has_out:
        missing.append("`### Out of Scope`")
    if not has_tier2:
        missing.append("`### Tier 2 honest error reclassify`")
    if missing:
        violations.append(
            f"{prd_path.name}: Rule 6 (6-2) violation: Scope section missing 3-tier "
            f"sub-heading(s): {', '.join(missing)}"
        )
    return violations


def verify_rule8_invariants_section(prd_path: Path, content: str) -> list[str]:
    """Rule 8 (8-5): `## Invariants` section audit verify (RC-6 source)。"""
    violations: list[str] = []
    section = get_section(content, r"^##\s+Invariants\b.*$")
    if section is None:
        violations.append(
            f"{prd_path.name}: Rule 8 (8-5) violation: matrix-driven PRD must have "
            f"`## Invariants` section (independent section、not in Spec Review checklist)"
        )
        return violations
    # 最低 1 つ INV-N entry 必要
    if not re.search(r"^###\s+INV-\d+\b", section, re.MULTILINE):
        violations.append(
            f"{prd_path.name}: Rule 8 (8-5) violation: `## Invariants` section is empty "
            f"(no `### INV-N` entries found)"
        )
    return violations


def verify_rule11_d5_impact_area_audit_findings(
    prd_path: Path, content: str
) -> list[str]:
    """Rule 11 (d-5): `## Impact Area Audit Findings` section embed (RC-8 source)。"""
    violations: list[str] = []
    section = get_section(content, r"^##\s+Impact Area Audit Findings\b.*$")
    if section is None:
        violations.append(
            f"{prd_path.name}: Rule 11 (d-5) violation: matrix-driven PRD must have "
            f"`## Impact Area Audit Findings` section "
            f"(`audit-ast-variant-coverage.py --files <impact-area>` 結果 embed)"
        )
    return violations


def verify_rule13_spec_review_iteration_log(prd_path: Path, content: str) -> list[str]:
    """Rule 13 (13-2/13-4): `## Spec Review Iteration Log` section (RC-9 source)。"""
    violations: list[str] = []
    section = get_section(content, r"^##\s+Spec Review Iteration Log\b.*$")
    if section is None:
        violations.append(
            f"{prd_path.name}: Rule 13 (13-4) violation: matrix-driven PRD must have "
            f"`## Spec Review Iteration Log` section (skill workflow Step 4.5 history)"
        )
        return violations
    # "self-review not performed" placeholder のみ → fail
    stripped = section.strip()
    if (
        "self-review not performed" in stripped.lower()
        or len(stripped) < 50
    ):
        violations.append(
            f"{prd_path.name}: Rule 13 (13-4) violation: `## Spec Review Iteration Log` "
            f"section is empty or 'self-review not performed' placeholder only"
        )
    return violations


# Uncertain expression patterns for Impact Area (RC-3 source)
UNCERTAIN_EXPR_PATTERNS = [
    (r"\(or\s+該当", "(or 該当)"),
    (r"\(or\s+別\s*file", "(or 別 file)"),
    (r"\bTBD\b", "TBD"),
    (r"要確認", "要確認"),
    (r"？(?!\?)", "？ (full-width question mark)"),
]


def verify_impact_area_uncertain_expressions(
    prd_path: Path, content: str
) -> list[str]:
    """RC-3: `## Impact Area` (or `### Impact Area`) section の uncertain expression 検出。"""
    violations: list[str] = []
    # `### Impact Area` (under `## Design`) or `## Impact Area`
    section: str | None = None
    for heading in (r"^###\s+Impact Area\s*$", r"^##\s+Impact Area\s*$"):
        m = re.search(heading, content, re.MULTILINE)
        if m:
            start = m.end()
            # next ### or ## heading
            next_h = re.search(r"^#{2,3}\s+(?!#)", content[start:], re.MULTILINE)
            section = (
                content[start : start + next_h.start()]
                if next_h
                else content[start:]
            )
            break
    if section is None:
        return violations  # Impact Area 不在は他の rule で別途 check
    for pattern, label in UNCERTAIN_EXPR_PATTERNS:
        if re.search(pattern, section):
            violations.append(
                f"{prd_path.name}: RC-3 violation: `## Impact Area` contains uncertain "
                f"expression '{label}' — empirical verify (find/Read) で確定後 commit"
            )
    return violations


def verify_orthogonality_merge_consistency(prd_path: Path, content: str) -> list[str]:
    """Rule 1 (1-4-b)(1-4-c) Spec-stage orthogonality merge structural verify
    (framework v1.5、I-205 deep review v3 final v3 で v1.4 Implementation Stage defer
    stance を Spec stage structural verify に revise)。

    matrix table 内 axis-merge cells (D 全 / B 全 / Bn/Bm 等) を検出、各 cell の
    Ideal output / Scope 列に "orthogonality-equivalent to cells N1-N2" 等
    referenced source cell を含む claim があるかをチェック、source cell が matrix
    内に存在 + Scope 列値が compatible (= 両 cells が同 Scope category) を verify。

    検出 pattern:
    - axis-merge wording: `D 全`、`B 全`、`Bn/Bm`、`Bn-Bm` (in Dimension column)
    - referenced source claim: "cells N-M" / "cell N" / "cells 24-28" 等
    """
    violations: list[str] = []
    section = get_section(content, r"^##\s+Problem Space\s*$")
    if section is None:
        return violations

    # matrix table 内の axis-merge wording を含む rows を抽出
    # rows are like: `| 35 | A4 ... | B5/B6/B7/B8/B9 | * | ideal | ... |`
    merge_pattern = re.compile(
        r"^\|\s*([\w-]+)\s*\|"  # cell # (group 1)
        r".*?"  # context columns
        r"\|\s*("  # axis-merge wording column (group 2)
        r"D\s*全|B\s*全|"
        r"B\d+(?:/B\d+)+|"  # B5/B6/B7
        r"B\d+-B\d+"  # B5-B9
        r")\s*\|",
        re.MULTILINE,
    )

    # all cell IDs in matrix (for referenced cell existence check)
    cell_id_pattern = re.compile(r"^\|\s*([\w-]+)\s*\|", re.MULTILINE)
    all_cell_ids: set[str] = set()
    for m in cell_id_pattern.finditer(section):
        cid = m.group(1).strip()
        # filter out non-cell rows (header `# | A | B | ...` or separator `---|---|...`)
        if cid in ("#", "Cell", "---", "----", "-----") or all(c == "-" for c in cid):
            continue
        all_cell_ids.add(cid)

    for m in merge_pattern.finditer(section):
        cell_id = m.group(1).strip()
        merge_wording = m.group(2).strip()
        # この cell の row 全体を取得 (line)
        row_line = section[m.start() : section.find("\n", m.start())]
        # referenced source cell # を抽出
        ref_pattern = re.compile(r"cells?\s+([\d\w-]+(?:[-〜~][\d\w-]+)?)", re.IGNORECASE)
        refs = ref_pattern.findall(row_line)
        if not refs:
            # (1-4-a): "orthogonality-equivalent" claim 不在 → Rule 1 (1-4-a) violation
            if "orthogonality-equivalent" not in row_line and "orthogonality merge" not in row_line:
                violations.append(
                    f"{prd_path.name}: Rule 1 (1-4-a) violation: cell `{cell_id}` "
                    f"contains axis-merge wording `{merge_wording}` but lacks "
                    f"`orthogonality-equivalent` justification statement"
                )
                continue
        # (1-4-b): referenced source cell の存在 verify
        for ref in refs:
            # ref might be "24-28" (range) or "12" (single)
            if "-" in ref or "〜" in ref or "~" in ref:
                # range reference: split and check first only (representative)
                first = re.split(r"[-〜~]", ref)[0].strip()
                if first not in all_cell_ids:
                    violations.append(
                        f"{prd_path.name}: Rule 1 (1-4-b) violation: cell `{cell_id}` "
                        f"references source cell range `{ref}` but cell `{first}` "
                        f"not found in matrix"
                    )
            else:
                if ref.strip() not in all_cell_ids:
                    violations.append(
                        f"{prd_path.name}: Rule 1 (1-4-b) violation: cell `{cell_id}` "
                        f"references source cell `{ref}` but not found in matrix"
                    )

    return violations


def verify_rule11_d6_relevance_compliance(prd_path: Path, content: str) -> list[str]:
    """Rule 11 (d-6) architectural-concern-relevance audit auto-verify
    (framework v1.6、F-deep-deep-1 fix 2026-04-28、deep deep review で v1.5 の
    Rule 11 (d-6) audit asymmetry を発見 → v1.6 で symmetry 確立)。

    `## Impact Area Audit Findings` section の defer 対象 `_ => ` arms に対し、
    (d-6-b-1) Architectural concern orthogonality declaration + (d-6-b-2)
    Non-interference probe verification statement の存在を structural detect。

    Detection patterns (各 defer arm row 内):
    - (d-6-b-1) marker: "orthogonality" / "orthogonal to" / "別 architectural concern"
    - (d-6-b-2) marker: "non-interference" / "non-dependent" / "probe" / "verification"

    Defer rows = "Decision" 列に "I-203 defer" / "別 PRD defer" 含む rows。
    """
    violations: list[str] = []
    section = get_section(content, r"^##\s+Impact Area Audit Findings\b.*$")
    if section is None:
        return violations  # Rule 11 (d-5) で別途 detect

    # `## Impact Area Audit Findings` section の defer rows (markdown table) を抽出
    # Row format: `| Violation | Location | Phase | Decision | Rationale |`
    # defer pattern: Decision 列に "I-203 defer" / "別 PRD defer" / "defer to" 等
    defer_pattern = re.compile(
        r"^\|\s*[^|]+\|\s*[^|]+\|\s*[^|]+\|\s*([^|]*defer[^|]*)\|\s*([^|]+)\|",
        re.MULTILINE | re.IGNORECASE,
    )
    for m in defer_pattern.finditer(section):
        decision = m.group(1).strip()
        rationale = m.group(2).strip()
        # (d-6-b-1) orthogonality declaration check
        ortho_markers = ["orthogonality", "orthogonal", "別 architectural concern", "別 architectural"]
        if not any(marker in rationale.lower() or marker in rationale for marker in ortho_markers):
            violations.append(
                f"{prd_path.name}: Rule 11 (d-6-b-1) violation: defer row "
                f"`{decision[:40]}` lacks orthogonality declaration "
                f"(architectural concern relevance verification statement) in rationale: "
                f"`{rationale[:80]}`"
            )
        # (d-6-b-2) non-interference probe marker check
        # 緩い check: rationale に "本 PRD" 関連性記述があれば許容、より厳密には probe location 必要
        nonint_markers = ["本 PRD", "non-interference", "non-dependent", "probe", "control flow"]
        if not any(marker in rationale for marker in nonint_markers):
            violations.append(
                f"{prd_path.name}: Rule 11 (d-6-b-2) violation: defer row "
                f"`{decision[:40]}` lacks non-interference probe marker in rationale: "
                f"`{rationale[:80]}`"
            )

    return violations


def verify_invariants_test_contracts(prd_path: Path, content: str) -> list[str]:
    """Rule 8 (8-c) Invariants verification test contracts audit auto-verify
    (framework v1.6、F-deep-deep-2 fix)。

    `## Invariants` section の各 INV-N entry に対し (c) Verification method の
    test fn name reference (`test_invariant_N_*`) が記載されていることを structural
    detect。spec text のみ (test code stub 不在) は spec gap = 別途 audit。
    """
    violations: list[str] = []
    section = get_section(content, r"^##\s+Invariants\b.*$")
    if section is None:
        return violations  # Rule 8 (8-5) で別途 detect

    # 各 INV-N entry を extract.
    # Bug fix (PRD I-D Iteration v3 third-party review F1 source、2026-05-10):
    # 旧 regex `[^#]*?` は body 内に literal `#` (= `cell #` / `# 1-30` 等) が
    # 含まれると early stop し、後続 INV-N が silent skip される構造的 bug。
    # `(?s)(?:(?!###\s+INV-\d+|^##\s).)*?` で 任意文字 (改行含、negative lookahead で
    # 次 INV / ## section / EOF を terminator) に置換、誤った `#` literal stop を排除。
    inv_pattern = re.compile(
        r"^###\s+INV-(\d+)\b"
        r"(?P<body>(?:(?!^###\s+INV-\d+|^##\s+(?!#)).)*)",
        re.MULTILINE | re.DOTALL,
    )
    for m in inv_pattern.finditer(section):
        inv_num = m.group(1)
        inv_body = m.group(0)
        # test fn name reference check
        if "test_invariant_" not in inv_body and "test fn" not in inv_body.lower():
            violations.append(
                f"{prd_path.name}: Rule 8 (8-c) violation: INV-{inv_num} entry lacks "
                f"`test fn` reference (test contract test_invariant_{inv_num}_* 必須)"
            )
    return violations


# ---------------------------------------------------------------------------
# PRD I-D-pre Phase 3 (T1-pre-1 + T1-pre-2 + T1-pre-4) audit script extensions.
# Path E utility verify functions の audit-prd-rule10-compliance.py 側 mirror。
# Option α auto-detect (= `## Cell Numbering Convention` section 有無) で I-205 等
# retroactive framework rule compliance 未達 PRD を新 verify functions の audit
# out-of-scope に自動分類 (= INV-4 4-tuple baseline preserve)。
# 詳細: TODO `[I-205-retroactive-cell-numbering-section]` + I-D-pre PRD doc Iteration v3
# ---------------------------------------------------------------------------


def has_cell_numbering_convention_section(content: str) -> bool:
    """Option α auto-detect helper: 新 verify functions の audit scope 判定。

    `## Cell Numbering Convention` section 有無で判定。section 存在 = 該当 PRD doc は
    framework rule retroactive compliance 完了 = audit scope 内、不在 = retroactive
    compliance 未達 = audit out-of-scope (= 新 verify functions skip)。

    Lesson source (PRD I-D-pre Iteration v3、2026-05-11): I-205 PRD doc は本 section +
    `## Spec→Impl Mapping` section 不在 + documented gaps 21 cells を持つ PRD pattern。
    案 γ Phase 2 T15 で section 追加 (= TODO `[I-205-retroactive-cell-numbering-section]`)
    で audit scope 内に自動 promote = future-proof design。
    """
    return bool(re.search(r"^##\s+Cell Numbering Convention\b", content, re.MULTILINE))


def _format_path_e_drifts(prd_path: Path, t_id: str, source: str, drifts: list[str]) -> list[str]:
    """Path E utility drifts を audit script convention violation message 形式へ変換。

    Format: "{prd_name}: {t_id} violation ({source}): {drift}"
    例: "I-D-pre-...md: T1-pre-1 violation (pending verdict): verify_status_pending_verdict: ..."
    """
    return [f"{prd_path.name}: {t_id} violation ({source}): {d}" for d in drifts]


def verify_pending_verdict_findings_consistency(
    prd_path: Path, content: str
) -> list[str]:
    """T1-pre-1 (Cell 1 / v3-6+v4-2 consolidated、F7 fix integrated)。

    Path E Axis 2 (`verify_status_pending_verdict`) の audit-prd-rule10-compliance.py 側
    mirror wrapper。v3-6 (pending verdict pattern detect) + v4-2 (Critical=0 claim ↔
    stale verdict consistency) を consolidated batch + F7 fix (= TS-X heading context
    内でも post-v15 wording 含む = stale late-stage claim flag) で structural detect。

    Option α auto-detect: `## Cell Numbering Convention` section 不在 PRD (= I-205) は
    audit out-of-scope に自動分類 (early-return)、INV-4 4-tuple baseline preserve。
    """
    if not has_cell_numbering_convention_section(content):
        return []  # I-205 等 retroactive compliance 未達 PRD は audit out-of-scope

    lines = content.splitlines()
    headings = _parse_headings(lines)
    drifts = _path_e_verify_status_pending(lines, headings)
    return _format_path_e_drifts(prd_path, "T1-pre-1", "pending verdict", drifts)


def verify_cross_reference_cell_consistency(
    prd_path: Path, content: str
) -> list[str]:
    """T1-pre-2 (Cell 2 / v5-1、F6 fix integrated)。

    Path E Axis 1 (`verify_cross_reference_cell_consistency`) の audit-prd-rule10-compliance.py
    側 mirror wrapper。matrix と各 cross-reference context (= Scope sub-sections /
    Spec→Impl Mapping / Invariants / Test Plan / Implementation Stage Tasks) の cell #
    appearance consistency を SECTION_COVERAGE_POLICY 5 sections allow-list で verify。
    F6 fix = threshold "5" arbitrary heuristic を spec-traceable allow-list に置換。

    Option α auto-detect: `## Cell Numbering Convention` section 不在 PRD (= I-205) は
    audit out-of-scope に自動分類 (early-return)、INV-4 4-tuple baseline preserve。
    """
    if not has_cell_numbering_convention_section(content):
        return []  # I-205 等 retroactive compliance 未達 PRD は audit out-of-scope

    lines = content.splitlines()
    headings = _parse_headings(lines)
    drifts = _path_e_verify_cross_reference(lines, headings)
    return _format_path_e_drifts(prd_path, "T1-pre-2", "cross-reference cell", drifts)


def verify_cell_numbering_drift_detection(
    prd_path: Path, content: str
) -> list[str]:
    """T1-pre-4 (Cell 5 / v13-5 audit part)。

    Path E Axis 3 (`verify_label_namespace_collision`、CELL_SLOT_AS_IDENTIFIER_RE narrow
    detection) の audit-prd-rule10-compliance.py 側 mirror wrapper。matrix # canonical
    identifier ↔ Spec→Impl Mapping table cell # ↔ 各 cross-reference context cell # の
    三者 1-to-1 mapping verify + cell-slot vocabulary fork drift detection (=
    identifier-level fork detection、broader vocabulary fork は別 PRD
    `[I-D-future-vocab-fork]` で deferred)。

    Option α auto-detect: `## Cell Numbering Convention` section 不在 PRD (= I-205) は
    audit out-of-scope に自動分類 (early-return)、INV-4 4-tuple baseline preserve。
    """
    if not has_cell_numbering_convention_section(content):
        return []  # I-205 等 retroactive compliance 未達 PRD は audit out-of-scope

    lines = content.splitlines()
    headings = _parse_headings(lines)
    drifts = _path_e_verify_label_namespace(lines, headings)
    return _format_path_e_drifts(prd_path, "T1-pre-4", "cell numbering drift", drifts)


def audit_prd(prd_path: Path) -> list[str]:
    """1 PRD doc の audit を実施、violation list を返す。"""
    content = prd_path.read_text(encoding="utf-8")
    violations: list[str] = []
    # 既存 Rule 10 + Rule 4 (4-3) audit (全 PRD 適用)
    violations.extend(verify_rule10_application(prd_path, content))
    violations.extend(verify_rule4_doc_first(prd_path, content))

    # 新 framework rules (RC-1〜9) は active PRD のみ enforce、
    # closed PRD (PRD-2.7 等) は grandfathered として skip
    if not is_active_prd(content):
        return violations

    # matrix-driven PRD のみの新 rules
    if is_matrix_driven(content):
        violations.extend(verify_rule1_abbreviation_prohibition(prd_path, content))
        violations.extend(verify_orthogonality_merge_consistency(prd_path, content))
        violations.extend(verify_rule2_oracle_observations(prd_path, content))
        violations.extend(verify_rule5_stage_tasks_separation(prd_path, content))
        violations.extend(verify_rule6_scope_3tier(prd_path, content))
        violations.extend(verify_rule8_invariants_section(prd_path, content))
        violations.extend(verify_rule11_d5_impact_area_audit_findings(prd_path, content))
        violations.extend(verify_rule11_d6_relevance_compliance(prd_path, content))
        violations.extend(verify_invariants_test_contracts(prd_path, content))
        violations.extend(verify_rule13_spec_review_iteration_log(prd_path, content))
        # PRD I-D-pre Phase 3 audit script extensions (T1-pre-1 + T1-pre-2 + T1-pre-4)
        # Option α auto-detect で I-205 等 retroactive compliance 未達 PRD を early-return
        violations.extend(verify_pending_verdict_findings_consistency(prd_path, content))
        violations.extend(verify_cross_reference_cell_consistency(prd_path, content))
        violations.extend(verify_cell_numbering_drift_detection(prd_path, content))
    # 全 active PRD で uncertain expr 検出
    violations.extend(verify_impact_area_uncertain_expressions(prd_path, content))
    return violations


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit PRD Rule 10/11/12 + Rule 4 (4-3) compliance"
    )
    parser.add_argument(
        "prd_paths",
        nargs="*",
        type=Path,
        help="PRD doc file paths (default: all backlog/*.md)",
    )
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    if args.prd_paths:
        prds = args.prd_paths
    else:
        # Default: backlog/ 配下で `## Rule 10 Application` section を含む全 PRD を
        # 自動検出 (= post-PRD-2.7 framework 適用 PRD)。File name prefix
        # (`PRD-*` / `I-*`) を判定基準にしない理由: PRD 2.7 完了後の新 PRD (I-205,
        # I-224 等) は I-* prefix で命名されているため、prefix-based filter では
        # **false-skip** が発生する (= I-224 T7 /check_problem 由来 2026-05-08
        # 発覚 issue)。`## Rule 10 Application` section の存在を structural marker
        # として content-based 検出することで、命名 convention に依存しない
        # 構造的な audit 対象選定が可能。Legacy partial-framework PRD (= `I-050`
        # umbrella、本 section 不在) は自動 skip される。
        prds = sorted(p for p in BACKLOG_DIR.glob("*.md") if is_matrix_driven_prd(p))

    if not prds:
        print(f"ERROR: no PRD doc found in {BACKLOG_DIR}", file=sys.stderr)
        return 2

    all_violations: list[str] = []
    for prd in prds:
        if args.verbose:
            print(f"[audit] {prd.name}", file=sys.stderr)
        violations = audit_prd(prd)
        all_violations.extend(violations)

    if all_violations:
        print(f"FAIL: {len(all_violations)} compliance violation(s):", file=sys.stderr)
        for v in all_violations:
            print(f"  - {v}", file=sys.stderr)
        return 1

    print(f"PASS: PRD Rule 10/11/12 + Rule 4 (4-3) compliance audit ({len(prds)} PRD(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
