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


def verify_no_duplicate_top_level_matrix(
    prd_path: Path, content: str
) -> list[str]:
    """T1-2 (cell 4 / v3-4): Duplicate top-level matrix sub-section detection
    (framework v1.10、PRD I-D-main Implementation T1-2 で establish)。

    `## Problem Space` section 内に **複数 `### 組合せマトリクス` sub-section が
    共存** する状態 (= iteration 移行時の旧 matrix 残存 pattern) を syntactic
    detect。最初の組合せマトリクス sub-section のみ accepted、後続は audit fail。

    Heading anchor based design (= `### 組合せマトリクス` canonical signal):
      - Primary Axis enumeration table / Auxiliary Axis enumeration table 等の
        sibling tables は `### 組合せマトリクス` heading anchor を持たないため
        false positive を生まない
      - active backlog/ PRDs (I-D-main / I-D-pre / I-205 等) で `組合せマトリクス`
        命名 convention 確立済

    Verification logic:
      1. `## Problem Space` section 抽出
      2. `^###\\s+組合せマトリクス\\b` heading occurrence count
      3. count > 1 で violation report (= duplicate combinatorial matrix
         sub-section detected)
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    ps_section = get_section(content, r"^##\s+Problem Space\s*$")
    if ps_section is None:
        return violations  # Problem Space 不在は別 audit で detect

    cm_pattern = re.compile(r"^###\s+組合せマトリクス\b", re.MULTILINE)
    cm_matches = cm_pattern.findall(ps_section)
    if len(cm_matches) > 1:
        violations.append(
            f"{prd_path.name}: v3-4 violation: duplicate `### 組合せマトリクス` "
            f"sub-section detected in `## Problem Space` section "
            f"(count={len(cm_matches)}, expected 1 = most recent only)。"
            f"iteration 移行時の旧 matrix 残存 pattern、cleanup 必要"
        )
    return violations


def verify_dispatch_tree_pseudocode_syntactic(
    prd_path: Path, content: str
) -> list[str]:
    """T1-3 (cell 5 / v3-5): Dispatch tree pseudocode syntactic validation
    (framework v1.10、PRD I-D-main Implementation T1-3 で establish)。

    `## Design` section 内 Rust pseudocode (= ```rust fenced code block) の各
    `match` arm を parse、duplicate pattern (comment-only disambiguation で
    hiding する pattern を含む) を syntactic detect。

    Verification logic:
      1. `## Design` section 抽出
      2. ```rust ... ``` fenced code blocks 全 extract
      3. 各 block 内で `<pattern> => <body>` 形式の arm を抽出
      4. pattern を normalize (= inline `/* ... */` comments を strip) して
         duplicate detection
      5. duplicate detected で violation report

    Rust pseudocode 不在 (= framework PRD で typical) の場合は trivially PASS。
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    design_section = get_section(content, r"^##\s+Design\s*$")
    if design_section is None:
        return violations

    rust_block_pattern = re.compile(r"```rust\s*\n(.*?)\n```", re.DOTALL)
    rust_blocks = rust_block_pattern.findall(design_section)
    if not rust_blocks:
        return violations  # Pseudocode 不在 = trivially PASS

    # Match arm pattern: leading whitespace + pattern + `=>` + body (anything after)
    arm_pattern = re.compile(r"^\s*([^=\n]+?)\s*=>")
    for block_idx, block in enumerate(rust_blocks):
        normalized_patterns: list[str] = []
        for line in block.splitlines():
            m = arm_pattern.match(line)
            if not m:
                continue
            raw_pattern = m.group(1).strip()
            # Strip inline `/* ... */` comments (= comment-only disambiguation
            # で hidden duplicate detection の core mechanism)
            normalized = re.sub(r"/\*.*?\*/", "", raw_pattern).strip()
            # Skip wildcard `_` arm (= exhaustiveness fallback、duplicate 概念
            # 不適用)
            if not normalized or normalized == "_":
                continue
            normalized_patterns.append(normalized)

        seen: set[str] = set()
        duplicates: list[str] = []
        for p in normalized_patterns:
            if p in seen and p not in duplicates:
                duplicates.append(p)
            seen.add(p)

        if duplicates:
            violations.append(
                f"{prd_path.name}: v3-5 violation: duplicate match arm pattern(s) "
                f"detected in `## Design` pseudocode block #{block_idx + 1}: "
                f"{duplicates}。comment-only disambiguation で hidden duplicate "
                f"の可能性、各 arm の semantic 区別を pattern level で明示必須"
            )
    return violations


def verify_dispatch_tree_axis_tuple_consistency(
    prd_path: Path, content: str
) -> list[str]:
    """T1-5 (cell 7 / v4-1): Dispatch tree axis-tuple semantic consistency check
    (framework v1.10、PRD I-D-main Implementation T1-5 で establish、/check_job
    Action Item #3 で count-based → semantic verify に upgrade 2026-05-15)。

    `### 組合せマトリクス` matrix table の各 active cell から **axis values
    tuple** を抽出 (= header row で `Axis N` を含む columns の値) + Design Rust
    pseudocode `match` arm の `(<axis_a>, <axis_b>, ...) =>` tuple pattern を
    抽出、**set inclusion check** で matrix axis-tuple が arm tuple set に存在
    することを verify。`_` wildcard fallback が存在し、かつ matrix axis-tuple が
    arm set 不在 cells があれば、それら cells が `unreachable!()` に fall-through
    する状態 = audit fail。

    Verification logic:
      1. `## Design` section の ```rust pseudocode block 抽出
      2. Pseudocode 不在なら trivially PASS (framework PRD typical)
      3. `### 組合せマトリクス` header row から axis columns (= header に `Axis`
         wording 含む) を identify
      4. axis columns 不在なら count-based fallback (= 後方互換、count comparison)
      5. axis columns 存在なら各 data row から axis-tuple を tuple 化、matrix_tuples set 構築
      6. Pseudocode arm の `(<a>, <b>, ...) =>` pattern を tuple 化、arm_tuples set 構築
      7. `_` wildcard 検出
      8. set inclusion: matrix_tuples - arm_tuples = fall-through cells set
      9. wildcard 存在 + fall-through cells 非空 で violation report (= 各 cell の
         axis-tuple identify)

    Spec compliance: spec wording の "axis values から (axis-tuple) tuple を
    derive + dispatch tree pseudocode の各 arm pattern と match、fall-through を
    syntactic detect" を semantic verify として実装。
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    design_section = get_section(content, r"^##\s+Design\s*$")
    if design_section is None:
        return violations

    rust_block_pattern = re.compile(r"```rust\s*\n(.*?)\n```", re.DOTALL)
    rust_blocks = rust_block_pattern.findall(design_section)
    if not rust_blocks:
        return violations  # No pseudocode = trivially PASS

    ps_section = get_section(content, r"^##\s+Problem Space\s*$")
    if ps_section is None:
        return violations
    cm_match = re.search(r"^###\s+組合せマトリクス\b", ps_section, re.MULTILINE)
    if not cm_match:
        return violations
    cm_start = cm_match.end()
    next_h = re.search(r"^(###|##)\s", ps_section[cm_start:], re.MULTILINE)
    cm_section = (
        ps_section[cm_start : cm_start + next_h.start()]
        if next_h
        else ps_section[cm_start:]
    )

    # Extract Rust pseudocode wildcard presence + explicit arm count + arm tuples
    # generic_arm_pattern catches any non-`_` arm (= tuple or single-axis pattern)
    # for explicit_arms counting (= count-based fallback path correctness)。
    # arm_tuple_pattern も同時 apply で tuple arms のみ arm_tuples set に追加
    # (= semantic verify path で matrix axis-tuples との set inclusion check)。
    generic_arm_pattern = re.compile(r"^\s*([^=\n]+?)\s*=>")
    underscore_arm_pattern = re.compile(r"^\s*_\s*=>")
    arm_tuples: set[tuple[str, ...]] = set()
    has_wildcard = False
    explicit_arms = 0
    for block in rust_blocks:
        for line in block.splitlines():
            if underscore_arm_pattern.match(line):
                has_wildcard = True
                continue
            m = generic_arm_pattern.match(line)
            if not m:
                continue
            raw = re.sub(r"/\*.*?\*/", "", m.group(1)).strip()
            if not raw or raw == "_":
                continue
            explicit_arms += 1
            # Tuple arm semantic parsing for matrix axis-tuple set inclusion check
            if raw.startswith("(") and raw.endswith(")"):
                inner = raw[1:-1].strip()
                components = tuple(c.strip() for c in inner.split(","))
                arm_tuples.add(components)

    # Parse matrix header row to identify axis columns
    header_match = re.search(r"^\|\s*#\s*\|(.+)$", cm_section, re.MULTILINE)
    axis_col_indices: list[int] = []
    if header_match:
        # header columns post `| # |` split
        header_cols = [c.strip() for c in header_match.group(1).split("|")]
        for idx, col in enumerate(header_cols):
            if re.search(r"\bAxis\b", col):
                axis_col_indices.append(idx)

    if axis_col_indices:
        # Semantic verify path: extract matrix axis-tuples and compare with arm tuples
        matrix_axis_tuples: list[tuple[int, tuple[str, ...]]] = []
        for row_match in re.finditer(
            r"^\|\s*(\d+)\s*\|(.+)$", cm_section, re.MULTILINE
        ):
            cell_n = int(row_match.group(1))
            cols = [c.strip() for c in row_match.group(2).split("|")]
            if max(axis_col_indices) >= len(cols):
                continue
            axis_tuple = tuple(cols[i] for i in axis_col_indices)
            matrix_axis_tuples.append((cell_n, axis_tuple))

        fall_through_cells: list[tuple[int, tuple[str, ...]]] = []
        for cell_n, axis_tuple in matrix_axis_tuples:
            if axis_tuple not in arm_tuples:
                fall_through_cells.append((cell_n, axis_tuple))

        if has_wildcard and fall_through_cells:
            details = ", ".join(
                f"cell {n} (axis-tuple {t})" for n, t in fall_through_cells
            )
            violations.append(
                f"{prd_path.name}: v4-1 violation: matrix cell(s) fall through "
                f"to `_` arm (semantic axis-tuple ↔ arm pattern mismatch): "
                f"{details}。explicit arms 必須 for spec→impl 1-to-1 verify"
            )
    else:
        # Count-based fallback (= axis columns not identifiable in matrix header)
        active_cells = len(re.findall(r"^\|\s*\d+\s*\|", cm_section, re.MULTILINE))
        if (
            has_wildcard
            and active_cells > 0
            and active_cells > explicit_arms
        ):
            violations.append(
                f"{prd_path.name}: v4-1 violation: matrix has {active_cells} "
                f"active cells but dispatch tree pseudocode covers only "
                f"{explicit_arms} explicit match arms (+ `_` wildcard "
                f"fallback)。{active_cells - explicit_arms} cells potentially "
                f"fall through to `unreachable!()` (count-based fallback、axis "
                f"columns 不在 header)、explicit arms 必須 for spec→impl 1-to-1 "
                f"verify"
            )
    return violations


def verify_dispatch_arm_mapping_table(
    prd_path: Path, content: str
) -> list[str]:
    """T1-6 (cell 9 / v4-3): Spec→Impl Dispatch Arm Mapping table 1-to-1 completeness
    (framework v1.10、PRD I-D-main Implementation T1-6 で establish)。

    `### Spec→Impl Dispatch Arm Mapping` sub-section 内 mapping table と
    `### 組合せマトリクス` matrix table の cell # 1-to-1 correspondence を verify。
    Mapping table 内 duplicate cell # (= 1-to-1 invariant 違反) + matrix cells が
    mapping から missing (= dispatch 不在) を detect。

    Verification logic:
      1. `### Spec→Impl Dispatch Arm Mapping` heading 検索 (= 本 sub-section
         がない PRD では trivially PASS = framework PRD typical 状態)
      2. Mapping table cell # 列 extract (= `| <int> | ... |` rows)
      3. Duplicate detection (= 1-to-1 invariant)
      4. `### 組合せマトリクス` 内 matrix cells と cross-reference (= matrix
         cells が全 mapping にあるか verify、MIGRATED rows は mapping 側 only
         で legitimate exception)
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    mapping_match = re.search(
        r"^###\s+Spec→Impl Dispatch Arm Mapping\b", content, re.MULTILINE
    )
    if not mapping_match:
        return violations  # No mapping table = trivially PASS

    section_start = mapping_match.end()
    next_h = re.search(r"^(###|##)\s", content[section_start:], re.MULTILINE)
    mapping_section = (
        content[section_start : section_start + next_h.start()]
        if next_h
        else content[section_start:]
    )

    mapping_cell_strs = re.findall(
        r"^\|\s*(\d+)\s*\|", mapping_section, re.MULTILINE
    )
    mapping_cells_list = [int(c) for c in mapping_cell_strs]
    mapping_cells_set = set(mapping_cells_list)

    # 1-to-1 invariant (duplicate detection)
    if len(mapping_cells_list) != len(mapping_cells_set):
        counter: dict[int, int] = {}
        for c in mapping_cells_list:
            counter[c] = counter.get(c, 0) + 1
        dups = sorted(c for c, n in counter.items() if n > 1)
        violations.append(
            f"{prd_path.name}: v4-3 violation: Spec→Impl Mapping table has "
            f"duplicate cell #(s) {dups}、1-to-1 invariant violated"
        )

    # Matrix cells cross-reference
    ps_section = get_section(content, r"^##\s+Problem Space\s*$")
    if ps_section is None:
        return violations
    cm_match = re.search(r"^###\s+組合せマトリクス\b", ps_section, re.MULTILINE)
    if not cm_match:
        return violations
    cm_start = cm_match.end()
    cm_next_h = re.search(r"^(###|##)\s", ps_section[cm_start:], re.MULTILINE)
    cm_section = (
        ps_section[cm_start : cm_start + cm_next_h.start()]
        if cm_next_h
        else ps_section[cm_start:]
    )
    matrix_cells = {
        int(m.group(1))
        for m in re.finditer(r"^\|\s*(\d+)\s*\|", cm_section, re.MULTILINE)
    }

    missing_in_mapping = sorted(matrix_cells - mapping_cells_set)
    if missing_in_mapping:
        violations.append(
            f"{prd_path.name}: v4-3 violation: matrix cells {missing_in_mapping} "
            f"absent from Spec→Impl Mapping table (1-to-1 mapping incomplete、"
            f"dispatch 不在 cells)"
        )
    return violations


def verify_pseudocode_underscore_arm_self_applied(
    prd_path: Path, content: str
) -> list[str]:
    """T1-8 (cell 12 / v6-1): PRD doc 内 Rust pseudocode の `_` arm self-applied
    compliance check (framework v1.10、PRD I-D-main Implementation T1-8 で
    establish)。

    `## Design` section 内 ```rust pseudocode block の各 match arm を scan、
    `_ =>` arm (= Rule 11 (11-1) `_` arm prohibition 違反) を syntactic detect。

    Verification logic:
      1. `## Design` section 抽出
      2. ```rust pseudocode blocks 全 extract
      3. 各 block line scan、`^_\\s*=>` pattern を violation report

    Rule 11 (11-1) rationale:
      Spec pseudocode が `_` arm を許容すると Implementation level の `_` arm
      全廃 enforcement と乖離、framework rule self-applied integrity 失墜。
      Spec wording 段階で `_` arm 不在を mandate することで Implementation
      level dispatch arm exhaustiveness を structurally derive 可能化。
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    design_section = get_section(content, r"^##\s+Design\s*$")
    if design_section is None:
        return violations

    rust_block_pattern = re.compile(r"```rust\s*\n(.*?)\n```", re.DOTALL)
    underscore_arm_pattern = re.compile(r"^\s*_\s*=>")
    for block_idx, block in enumerate(
        rust_block_pattern.findall(design_section), start=1
    ):
        for line_idx, line in enumerate(block.splitlines(), start=1):
            if underscore_arm_pattern.match(line):
                stripped = line.strip()
                violations.append(
                    f"{prd_path.name}: v6-1 violation: Rule 11 (11-1) `_` arm "
                    f"detected in `## Design` pseudocode block #{block_idx} "
                    f"line {line_idx}: `{stripped}`。`_` arm 全廃 self-applied "
                    f"compliance 違反"
                )
    return violations


def verify_invariant_cell_coverage_double_partition(
    prd_path: Path, content: str
) -> list[str]:
    """T1-9 (cell 13 / v6-2 part): INV entries の `全 N cells/candidates` claim と
    actual matrix active cells count の cross-reference consistency verify
    (framework v1.10、PRD I-D-main Implementation T1-9 で establish)。

    Existing `verify_invariants_test_contracts` の strengthening として、INV-N
    body 内 "全 N cells" / "全 N candidates" / "全 N variants" claim を抽出、
    `### 組合せマトリクス` matrix table の active cells count と一致するか
    verify。

    Verification logic:
      1. `## Invariants` section 内 各 INV-N entry body を抽出
      2. `全 N (cells|candidates|variants)` pattern match
      3. matrix active cells count と diff、不一致で violation report

    Single-partition / dual-partition both supported (= 各 INV entry 独立 verify、
    library mode + executable mode の dual claim 両方が actual cells と一致する
    場合は OK)。
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    section = get_section(content, r"^##\s+Invariants\b.*$")
    if section is None:
        return violations

    ps_section = get_section(content, r"^##\s+Problem Space\s*$")
    actual_cells = 0
    if ps_section is not None:
        cm_match = re.search(r"^###\s+組合せマトリクス\b", ps_section, re.MULTILINE)
        if cm_match:
            cm_start = cm_match.end()
            cm_next_h = re.search(
                r"^(###|##)\s", ps_section[cm_start:], re.MULTILINE
            )
            cm_section = (
                ps_section[cm_start : cm_start + cm_next_h.start()]
                if cm_next_h
                else ps_section[cm_start:]
            )
            actual_cells = len(
                re.findall(r"^\|\s*\d+\s*\|", cm_section, re.MULTILINE)
            )
    if actual_cells == 0:
        return violations

    inv_pattern = re.compile(
        r"^###\s+INV-(\d+)\b"
        r"(?P<body>(?:(?!^###\s+INV-\d+|^##\s+(?!#)).)*)",
        re.MULTILINE | re.DOTALL,
    )
    claim_pattern = re.compile(
        r"全\s*(\d+)\s*(?:cells|candidates|variants)"
    )
    for inv_m in inv_pattern.finditer(section):
        inv_num = inv_m.group(1)
        body = inv_m.group(0)
        for claim_m in claim_pattern.finditer(body):
            claimed = int(claim_m.group(1))
            if claimed != actual_cells:
                violations.append(
                    f"{prd_path.name}: v6-2 violation: INV-{inv_num} claims "
                    f"'全 {claimed}' but actual matrix has {actual_cells} "
                    f"active cells (cross-reference inconsistency)"
                )
    return violations


def verify_pending_verdict_severity_default(
    prd_path: Path, content: str
) -> list[str]:
    """T1-11 (cell 20 / v11-8): Pending verdict severity default = Critical
    declaration check (framework v1.10、PRD I-D-main Implementation T1-11 で
    establish)。

    Spec Review Iteration Log の各 Iteration entry で `Pending verdict N`
    (N > 0) wording がある場合、severity default = Critical declaration が
    body 内に存在することを syntactic verify。

    Verification logic:
      1. `## Spec Review Iteration Log` section 抽出
      2. 各 `### Iteration v<num>` entry body 抽出
      3. `Pending verdict <N>` / `Pending <N>` wording 検索 (N > 0)
      4. body 内 `severity (default)? = Critical` / `Critical default`
         declaration 検索
      5. declaration 不在で violation report (= Rule 13 (v11-8) violation)

    Rationale:
      Pending verdict は Spec stage 移行 block する severity = Critical default
      適用が Rule 13 (v11-8) で確立。declaration 不在 = severity default 適用
      不在 = pending verdict が implicit に Medium/Low severity と誤分類される
      risk、Critical default 明示で structural enforcement。
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    section = get_section(content, r"^##\s+Spec Review Iteration Log\b.*$")
    if section is None:
        return violations

    iter_pattern = re.compile(
        r"^###\s+Iteration v(\d+)\b"
        r"(?P<body>(?:(?!^###\s+Iteration v\d+|^##\s+(?!#)).)*)",
        re.MULTILINE | re.DOTALL,
    )
    severity_decl_pattern = re.compile(
        r"severity\s+default\s*=?\s*Critical|severity\s+Critical\s+default|Critical\s+default",
        re.IGNORECASE,
    )
    pv_pattern = re.compile(
        r"Pending\s+verdict\s+(\d+)|(?<!Critical\s)Pending\s+(\d+)"
    )

    for m in iter_pattern.finditer(section):
        iter_num = m.group(1)
        body = m.group(0)
        for pv_m in pv_pattern.finditer(body):
            pv_count = int(pv_m.group(1) or pv_m.group(2) or 0)
            if pv_count > 0 and not severity_decl_pattern.search(body):
                violations.append(
                    f"{prd_path.name}: v11-8 violation: Iteration v{iter_num} "
                    f"has 'Pending verdict {pv_count}' but lacks 'severity "
                    f"default = Critical' declaration (Rule 13 sub-rule "
                    f"violation、Spec stage 移行 block default 適用必須)"
                )
                break  # 1 violation per iteration 十分
    return violations


def verify_completion_criteria_probe_pattern(
    prd_path: Path, content: str
) -> list[str]:
    """T1-12 (cell 26 / v13-1 audit part): Completion Criteria に empirical probe
    pattern (= cargo / python3 / audit function / file path reference) 存在
    verify (framework v1.10、PRD I-D-main Implementation T1-12 で establish)。

    `## Completion Criteria` section の各 numbered criterion (`1. **Title**: ...`)
    body 内に empirical probe signature が存在することを syntactic verify。
    probe signature 不在 = manual cross-check 依存 = structural enforcement
    不在 = violation。

    Probe signatures (= structural enforcement evidence):
      - `cargo (test|clippy|fmt)` invocation
      - `python3 scripts/...` invocation
      - `./scripts/...` shell script invocation
      - `.github/workflows/` CI reference
      - `grep` / `verify_<name>` audit function reference
      - File path reference (`<path>.{py,sh,toml,md,yml}`)
      - `exit code 0` / numeric exit code probe

    Rationale:
      Completion criterion が "manual review" / "developer 確認" wording のみで
      probe command 不在の場合、completion verify が developer 主観依存 →
      structural enforcement 不在。各 criterion に CLI command / file path /
      audit function reference 形式で probe pattern を embed することで
      mechanical verification を constructional に保証。
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    section = get_section(content, r"^##\s+Completion Criteria\b.*$")
    if section is None:
        return violations

    # Limit scope to numbered criteria block (= `1.` 〜 next `###` heading or
    # section end)
    body_until_subheading = re.split(r"^###\s", section, maxsplit=1, flags=re.MULTILINE)[
        0
    ]

    criterion_pattern = re.compile(
        r"^(\d+)\.\s+\*\*([^*]+)\*\*:\s*(.+?)(?=^\d+\.\s\*\*|\Z)",
        re.MULTILINE | re.DOTALL,
    )

    probe_signatures = [
        r"cargo\s+test",
        r"cargo\s+clippy",
        r"cargo\s+fmt",
        r"python3\s+scripts/",
        r"\./scripts/",
        r"\.github/workflows/",
        r"\bgrep\b",
        r"verify_\w+",
        r"`[^`]*\.(?:py|sh|toml|md|yml)`",
        r"exit\s+code\s+\d+",
    ]
    probe_regex = re.compile("|".join(probe_signatures), re.IGNORECASE)

    for m in criterion_pattern.finditer(body_until_subheading):
        crit_num = m.group(1)
        crit_title = m.group(2).strip()
        crit_body = m.group(3)
        if not probe_regex.search(crit_body):
            violations.append(
                f"{prd_path.name}: v13-1 violation: Completion Criterion "
                f"{crit_num} ('{crit_title[:50]}...') lacks empirical probe "
                f"pattern (= cargo / python3 / audit function reference / "
                f"file path)。manual cross-check 依存、structural enforcement "
                f"不在"
            )
    return violations


def verify_fixture_oracle_byte_consistency(
    prd_path: Path, content: str
) -> list[str]:
    """T1-14 (cell 29 / v13-6 audit part): Oracle Observations 内 TS fixture path
    の file existence + byte-level consistency verify (framework v1.10、PRD
    I-D-main Implementation T1-14 で establish)。

    `## Oracle Observations` section 内 `tests/...\\.(ts|tsx)` path references
    を抽出、各 path file 存在を verify。実 file 不在 = Oracle re-grounding gap
    = violation report (= `spec-first-prd.md` 「Spec への逆戻り」 procedure
    step 5-a 適用 required)。

    Future enhancement: 近接 ```typescript fenced code block の embedded content
    と file content byte-level compare (= 現実装 = path existence のみ verify、
    content divergence は future audit extension で coordinated implement)。

    Repo root resolution = `BACKLOG_DIR.parent` (= `BACKLOG_DIR` is `backlog/`,
    parent = repo root)。`tests/...` paths are relative to repo root.
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    section = get_section(content, r"^##\s+Oracle Observations\b.*$")
    if section is None:
        return violations

    repo_root = BACKLOG_DIR.parent
    fixture_path_pattern = re.compile(r"`(tests/[\w/.\-]+\.(?:ts|tsx))`")
    seen_paths: set[str] = set()
    for m in fixture_path_pattern.finditer(section):
        rel_path = m.group(1)
        if rel_path in seen_paths:
            continue  # Same path mentioned multiple times、1 violation per path
        seen_paths.add(rel_path)
        abs_path = repo_root / rel_path
        if not abs_path.exists():
            violations.append(
                f"{prd_path.name}: v13-6 violation: Oracle Observations "
                f"references `{rel_path}` but file does not exist。Oracle "
                f"re-grounding gap (= `spec-first-prd.md` 「Spec への逆戻り」 "
                f"procedure step 5-a mandatory: fixture content 変更時 Oracle "
                f"Observations section 更新必須)"
            )
    return violations


def verify_cartesian_product_completeness(
    prd_path: Path, content: str
) -> list[str]:
    """T1-1 (cell 1 / R-1): Cartesian product completeness audit (framework v1.10、
    PRD I-D-main Implementation T1-1 で establish)。

    yaml `## Rule 10 Application` block 内の optional `Cartesian product completeness:`
    mapping field (= `Expected cell count:` int + `Documented gaps:` list[int]) を
    canonical source として、Problem Space matrix table の actual cell # 集合との
    integrity を verify。implicit omission (= expected_active から absent) + range
    overflow (= expected_total を超える cell # 出現) を detect。

    Optional field design rationale: 既存 PRDs (= I-050 legacy / I-205 retroactive
    cell numbering pending) は本 field 不在で audit out-of-scope に自動分類、INV-4
    3-tuple post-close baseline preserve。本 field 宣言は self-applied integration
    evidence として I-D-main / 後続 PRDs で adopt。

    Verification logic:
      1. Rule 10 yaml dict から `Cartesian product completeness:` 取得
      2. expected_total = `Expected cell count` (positive int)
      3. documented_gaps_set = `Documented gaps` (list[int]) を set 化
      4. expected_active = {1..expected_total} - documented_gaps_set
      5. Problem Space section matrix table から actual cell # 集合 (= 1..expected_total
         範囲内) を抽出
      6. implicit_omitted = expected_active - actual = violation if non-empty
      7. extra_cells = {n in actual where n > expected_total} = violation if non-empty
    """
    violations: list[str] = []
    if not has_cell_numbering_convention_section(content):
        return violations  # Retroactive compliance pending PRDs (= I-205 etc) audit out-of-scope
    data, _err = parse_rule10_section(content)
    if data is None:
        return violations  # Rule 10 section 不在は別 audit で detect

    cp = data.get("Cartesian product completeness")
    if cp is None:
        return violations  # Optional field: 不在で skip (backward compatible)

    if not isinstance(cp, dict):
        violations.append(
            f"{prd_path.name}: Cartesian product completeness violation: "
            f"`Cartesian product completeness` must be a yaml mapping, "
            f"got {type(cp).__name__}"
        )
        return violations

    expected_total = cp.get("Expected cell count")
    if not isinstance(expected_total, int) or expected_total < 1:
        violations.append(
            f"{prd_path.name}: Cartesian product completeness violation: "
            f"`Expected cell count` must be a positive integer, "
            f"got {expected_total!r}"
        )
        return violations

    documented_gaps = cp.get("Documented gaps", [])
    if not isinstance(documented_gaps, list) or not all(
        isinstance(g, int) for g in documented_gaps
    ):
        violations.append(
            f"{prd_path.name}: Cartesian product completeness violation: "
            f"`Documented gaps` must be a list of integers, "
            f"got {documented_gaps!r}"
        )
        return violations

    documented_gaps_set = set(documented_gaps)
    expected_active = set(range(1, expected_total + 1)) - documented_gaps_set

    ps_section = get_section(content, r"^##\s+Problem Space\s*$")
    if ps_section is None:
        violations.append(
            f"{prd_path.name}: Cartesian product completeness violation: "
            f"`## Problem Space` section missing for matrix table parsing"
        )
        return violations

    # Matrix table の最初 column (= cell #) を抽出。Header row (`| # | ... |`) と
    # separator (`|---| ... |`) は数値 column ではないため自動除外。
    actual_cells: set[int] = set()
    for m in re.finditer(r"^\|\s*(\d+)\s*\|", ps_section, re.MULTILINE):
        n = int(m.group(1))
        if 1 <= n <= expected_total:
            actual_cells.add(n)

    implicit_omitted = sorted(expected_active - actual_cells)
    if implicit_omitted:
        violations.append(
            f"{prd_path.name}: Cartesian product completeness violation: "
            f"implicit omission detected, cells {implicit_omitted} expected "
            f"(Expected cell count={expected_total}, "
            f"Documented gaps={sorted(documented_gaps_set)}) "
            f"but absent from matrix table"
        )

    # Range overflow detection (= matrix に Expected cell count 超過 cell # が出現)
    # `actual_cells` は既に 1..expected_total 範囲で filter 済のため、別途 raw scan
    raw_cells: set[int] = set()
    for m in re.finditer(r"^\|\s*(\d+)\s*\|", ps_section, re.MULTILINE):
        raw_cells.add(int(m.group(1)))
    overflow_cells = sorted(c for c in raw_cells if c > expected_total)
    if overflow_cells:
        violations.append(
            f"{prd_path.name}: Cartesian product completeness violation: "
            f"unexpected cells {overflow_cells} in matrix table beyond "
            f"Expected cell count={expected_total}"
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
        # PRD I-D-main T1-1 (cell 1 / R-1): Cartesian product completeness audit
        # Optional field、未宣言 PRD は skip (= existing PRDs backward compatible)
        violations.extend(verify_cartesian_product_completeness(prd_path, content))
        # PRD I-D-main T1-2 (cell 4 / v3-4): Duplicate top-level matrix detection
        violations.extend(verify_no_duplicate_top_level_matrix(prd_path, content))
        # PRD I-D-main T1-3 (cell 5 / v3-5): Dispatch tree pseudocode syntactic validation
        violations.extend(
            verify_dispatch_tree_pseudocode_syntactic(prd_path, content)
        )
        # PRD I-D-main T1-5 (cell 7 / v4-1): Dispatch tree axis-tuple consistency
        violations.extend(
            verify_dispatch_tree_axis_tuple_consistency(prd_path, content)
        )
        # PRD I-D-main T1-6 (cell 9 / v4-3): Spec→Impl Mapping table 1-to-1 completeness
        violations.extend(verify_dispatch_arm_mapping_table(prd_path, content))
        # PRD I-D-main T1-8 (cell 12 / v6-1): Pseudocode `_` arm self-applied compliance
        violations.extend(
            verify_pseudocode_underscore_arm_self_applied(prd_path, content)
        )
        # PRD I-D-main T1-9 (cell 13 / v6-2 part): INV cell coverage double-partition
        violations.extend(
            verify_invariant_cell_coverage_double_partition(prd_path, content)
        )
        # PRD I-D-main T1-11 (cell 20 / v11-8): Pending verdict severity default check
        violations.extend(
            verify_pending_verdict_severity_default(prd_path, content)
        )
        # PRD I-D-main T1-12 (cell 26 / v13-1 audit part): Completion Criteria probe pattern
        violations.extend(
            verify_completion_criteria_probe_pattern(prd_path, content)
        )
        # PRD I-D-main T1-14 (cell 29 / v13-6 audit part): Oracle fixture byte consistency
        violations.extend(
            verify_fixture_oracle_byte_consistency(prd_path, content)
        )
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
