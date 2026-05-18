# Test fixture: expand_cell_list Pattern 4 (TABLE_FIRST_COL_NUM_RE) cells 31-99 bug fix outcome

PRD I-D-c11 test target = Pattern 4 (markdown table first column "| N |" form) cells 31-99 detection。
Pre-PRD: filter `<= 30` で cells 31-99 を silent skip。Post-PRD: filter `<= 99` で正しく detect。

| 5  | small |
| 30 | boundary |
| 31 | newly detected |
| 38 | mid-range |
| 70 | high-range |
| 99 | upper-bound |

Expected detect (Pre-PRD I-D-c11): {5, 30}
Expected detect (Post-PRD I-D-c11): {5, 30, 31, 38, 70, 99}
Diff (newly detected via bug fix): {31, 38, 70, 99}
