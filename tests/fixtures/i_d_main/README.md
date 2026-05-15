# PRD I-D-main Test Fixtures

Synthetic PRD doc fixtures for PRD I-D-main audit function tests
(= `tests/i_d_main_audit_extensions_test.rs`、T1 phase audit script extensions
の per-function positive + boundary + specific PASS path + Option α gate test
coverage)。

## Directory structure

- `positive/` — PRD doc fixtures intentionally containing violation pattern (= audit function が detect 想定)
- `negative/` — PRD doc fixtures without violation pattern (= audit function PASS 想定):
  - **Boundary fixtures**: cartesian_complete.md (= Option α gate pass、function-specific section absent → early return PASS)
  - **Specific PASS path fixtures**: distinct per-function fixtures (= function-specific section present + actual logic runs + no violation pattern PASS)
  - **Gate fixture**: option_alpha_gate_skips_pre_compliance.md (= `## Cell Numbering Convention` section absent → Option α gate で全 NEW functions が skip)

## Positive fixtures (= violation pattern intentional)

各 fixture は対応 audit function の **single concern** に focus (= isolated violation detection)。
他 audit function に対しては trivially PASS となるよう設計 (= cross-axis test
contamination avoidance)。各 fixture は `## Cell Numbering Convention` section
を embed し Option α gate を pass する。

| Fixture | Audit function | Violation ID | Test |
|---------|----------------|--------------|------|
| `cartesian_implicit_omission.md` | `verify_cartesian_product_completeness` | `Cartesian product completeness violation` | `test_audit_cartesian_completeness_detects_implicit_omission` |
| `duplicate_top_level_matrix.md` | `verify_no_duplicate_top_level_matrix` | `v3-4 violation` | `test_audit_detects_duplicate_top_level_matrix` |
| `dispatch_tree_duplicate_arms.md` | `verify_dispatch_tree_pseudocode_syntactic` | `v3-5 violation` | `test_audit_detects_dispatch_tree_duplicate_match_arms` |
| `dispatch_tree_axis_tuple_mismatch.md` | `verify_dispatch_tree_axis_tuple_consistency` | `v4-1 violation` | `test_audit_dispatch_tree_axis_tuple_definition_match` |
| `dispatch_arm_mapping_incomplete.md` | `verify_dispatch_arm_mapping_table` | `v4-3 violation` | `test_audit_dispatch_arm_mapping_completeness_one_to_one` |
| `pseudocode_underscore_arm.md` | `verify_pseudocode_underscore_arm_self_applied` | `v6-1 violation` | `test_audit_pseudocode_predicate_underscore_arm_compliance` |
| `invariant_cell_coverage_inconsistent.md` | `verify_invariant_cell_coverage_double_partition` | `v6-2 violation` | `test_audit_invariant_double_partition_coverage` |
| `pending_verdict_severity_missing.md` | `verify_pending_verdict_severity_default` | `v11-8 violation` | `test_audit_pending_verdict_severity_default` |
| `completion_criteria_no_probe.md` | `verify_completion_criteria_probe_pattern` | `v13-1 violation` | `test_audit_completion_criteria_probe_pattern` |
| `oracle_fixture_missing.md` | `verify_fixture_oracle_byte_consistency` | `v13-6 violation` | `test_audit_fixture_oracle_byte_consistency` |

## Negative fixtures

### Shared boundary fixture (= function-specific section absent → early return PASS)

| Fixture | 役割 | Tests |
|---------|------|-------|
| `cartesian_complete.md` | **Shared "fully shaped" negative fixture**: Option α gate pass (= Cell Numbering Convention section embed) + 各 audit function の function-specific section が **不在** (no pseudocode / no `全 N cells` wording / no Pending verdict wording / no Completion Criteria section / no Oracle fixture paths) → function-specific early return PASS。例外: Spec→Impl Mapping section は present + 4 cells covered = **T1-6 specific PASS path verify** (not boundary)。 | `test_audit_cartesian_completeness_passes_with_documented_gaps` (T1-1 specific) / `test_audit_no_false_positive_on_single_matrix` (T1-2 specific) / `test_audit_no_v3_5_false_positive_on_no_pseudocode` (T1-3 boundary) / `test_audit_no_v4_1_false_positive_on_pseudocode_absent` (T1-5 boundary) / `test_audit_no_v4_3_false_positive_on_complete_mapping` (T1-6 specific) / `test_audit_no_v6_1_false_positive_on_pseudocode_absent` (T1-8 boundary) / `test_audit_no_v6_2_false_positive_on_no_claim_wording` (T1-9 boundary) / `test_audit_no_v11_8_false_positive_on_no_pending_verdict` (T1-11 boundary) / `test_audit_no_v13_1_false_positive_on_no_completion_criteria` (T1-12 boundary) / `test_audit_no_v13_6_false_positive_on_no_fixture_paths` (T1-14 boundary) |

### Specific PASS path fixtures (= function-specific section present + actual logic runs + no violation pattern PASS)

| Fixture | Audit function | Specific PASS path | Test |
|---------|----------------|---------------------|------|
| `dispatch_tree_no_duplicate_arms.md` | T1-3 verify_dispatch_tree_pseudocode_syntactic | pseudocode present + all match arms distinct | `test_audit_no_v3_5_false_positive_on_distinct_arms` |
| `dispatch_tree_axis_tuple_full_coverage.md` | T1-5 verify_dispatch_tree_axis_tuple_consistency | pseudocode present + matrix axis-tuples ⊆ arm tuple set (exhaustive enumeration) | `test_audit_no_v4_1_false_positive_on_full_axis_coverage` |
| `pseudocode_no_underscore_arm.md` | T1-8 verify_pseudocode_underscore_arm_self_applied | pseudocode present + no `_` arm (Rule 11 (11-1) compliance) | `test_audit_no_v6_1_false_positive_on_no_underscore_arm` |
| `invariant_matching_claim.md` | T1-9 verify_invariant_cell_coverage_double_partition | INV `全 N cells` wording present + N matches actual matrix cells count | `test_audit_no_v6_2_false_positive_on_matching_claim` |
| `pending_verdict_with_severity.md` | T1-11 verify_pending_verdict_severity_default | `Pending verdict N>0` present + `severity default = Critical` declaration present | `test_audit_no_v11_8_false_positive_on_severity_declared` |
| `completion_criteria_with_probe.md` | T1-12 verify_completion_criteria_probe_pattern | Completion Criteria section present + probe pattern in each criterion | `test_audit_no_v13_1_false_positive_on_probe_present` |
| `oracle_fixture_existing.md` | T1-14 verify_fixture_oracle_byte_consistency | Oracle Observations references existing TS fixture path | `test_audit_no_v13_6_false_positive_on_existing_fixture` |

### Option α gate direct test fixture

| Fixture | 役割 | Test |
|---------|------|------|
| `option_alpha_gate_skips_pre_compliance.md` | **Option α auto-detect gate direct test fixture**: cartesian_implicit_omission.md と同一の violation pattern を含むが `## Cell Numbering Convention` section が **不在** = retroactive compliance pending state。期待: 全 NEW audit functions が Option α gate で early-return = skip = exit 0 / no violations。 | `test_audit_option_alpha_gate_skips_pre_compliance_prds` |

## Test coverage matrix (post /check_job Round 1 + Round 2 fix)

各 audit function に対して **3 directions of coverage** を達成: positive detection + boundary verify (function-specific early return) + specific PASS path (function-specific section present + actual logic runs + no violation)。

| Audit function | Positive | Boundary (function-specific early return) | Specific PASS path (actual logic + no violation) |
|----------------|----------|-------------------------------------------|-------------------------------------------------|
| T1-1 verify_cartesian_product_completeness | ✓ | (yaml field absence は Option α gate test で cover) | ✓ (`cartesian_complete.md` documented gaps absorb) |
| T1-2 verify_no_duplicate_top_level_matrix | ✓ | (1 heading only は specific PASS と等価) | ✓ (`cartesian_complete.md` 1 heading) |
| T1-3 verify_dispatch_tree_pseudocode_syntactic | ✓ | ✓ (no pseudocode) | ✓ (`dispatch_tree_no_duplicate_arms.md`) |
| T1-5 verify_dispatch_tree_axis_tuple_consistency | ✓ | ✓ (no pseudocode) | ✓ (`dispatch_tree_axis_tuple_full_coverage.md`) |
| T1-6 verify_dispatch_arm_mapping_table | ✓ | (no mapping section は Option α gate test で cover) | ✓ (`cartesian_complete.md` complete mapping) |
| T1-8 verify_pseudocode_underscore_arm_self_applied | ✓ | ✓ (no pseudocode) | ✓ (`pseudocode_no_underscore_arm.md`) |
| T1-9 verify_invariant_cell_coverage_double_partition | ✓ | ✓ (no claim wording) | ✓ (`invariant_matching_claim.md`) |
| T1-11 verify_pending_verdict_severity_default | ✓ | ✓ (no Pending verdict wording) | ✓ (`pending_verdict_with_severity.md`) |
| T1-12 verify_completion_criteria_probe_pattern | ✓ | ✓ (no Completion Criteria section) | ✓ (`completion_criteria_with_probe.md`) |
| T1-14 verify_fixture_oracle_byte_consistency | ✓ | ✓ (no fixture paths) | ✓ (`oracle_fixture_existing.md`) |
| **All NEW functions** | — | — | **Option α gate skip** (`option_alpha_gate_skips_pre_compliance.md`) |

**Total tests**: 28 (= 10 positive + 10 boundary/specific via cartesian_complete.md + 7 specific via distinct fixtures + 1 gate)

## Reference

PRD doc: [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](../../../backlog/I-D-main-framework-rule-integration-cohesive-batch.md) `## Implementation Stage Tasks` section T1 sub-tasks。

Test helpers: `tests/common/mod.rs` (= /check_job Round 1 Action Item #5 DRY refactor 由来、共有 `run_audit` / `count_violations_containing` etc.)。
