#!/usr/bin/env python3
"""PRD 2.7 — Rule 10/11/12 + Rule 4 (4-3) compliance audit.

PRD doc に対し以下 2 audit を実施:
  1. Rule 12 application section 検証 (Q5 application、`## Rule 10 Application`
     section + machine-parseable yaml format + Permitted/Prohibited keywords)
  2. Rule 4 (4-3) doc-first dependency order 検証 (Q6 application、Task List で
     code 改修 task の Prerequisites に doc update task ID 存在 verify)

Usage:
    python3 scripts/audit-prd-rule10-compliance.py [PRD_PATH ...] [--verbose]

PRD_PATH: 監査対象 PRD doc (markdown)。複数指定可。省略時は backlog/*.md 全 file。

Exit code:
    0: audit pass (全 PRD compliance OK)
    非 0: audit fail (compliance 違反 detected)

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

REPO_ROOT = Path(__file__).resolve().parent.parent
BACKLOG_DIR = REPO_ROOT / "backlog"

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


def audit_prd(prd_path: Path) -> list[str]:
    """1 PRD doc の audit を実施、violation list を返す。"""
    content = prd_path.read_text(encoding="utf-8")
    violations: list[str] = []
    violations.extend(verify_rule10_application(prd_path, content))
    violations.extend(verify_rule4_doc_first(prd_path, content))
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
        # Default: 新 framework 適用 PRD (PRD-* prefix) のみ audit。
        # Legacy PRD (I-* prefix) は本 framework 適用前の historical record として
        # 別途 archive (本 PRD 2.7 post-completion)、本 audit 対象外。
        prds = sorted(BACKLOG_DIR.glob("PRD-*.md"))

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
