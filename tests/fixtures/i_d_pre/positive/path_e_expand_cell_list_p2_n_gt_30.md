# Test fixture: expand_cell_list Pattern 2 (CELL_STANDALONE_RE) cells 31-99 bug fix outcome

PRD I-D-c11 test target = Pattern 2 (capitalized "Cell N" standalone form) cells 31-99 detection。
Pre-PRD: filter `<= 30` で cells 31-99 を silent skip。Post-PRD: filter `<= 99` で正しく detect。

Cell 5, Cell 30, Cell 31, Cell 38, Cell 70, Cell 99

Expected detect (Pre-PRD I-D-c11): {5, 30}
Expected detect (Post-PRD I-D-c11): {5, 30, 31, 38, 70, 99}
Diff (newly detected via bug fix): {31, 38, 70, 99}

Note: 本 fixture を `expand_cell_list` direct invocation で test、CELL_LIST_RE (P1、IGNORECASE flag) が前 catch する場合あり (= "Cell 5, Cell 30..." を `cells?\s+` で match attempt 可能)、Pattern 2 individual regex direct test は `test_path_e_p2_p4_reject_negative_individual` で separate verify。
