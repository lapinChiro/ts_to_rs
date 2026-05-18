# Test fixture: expand_cell_list Pattern 1 (CELL_LIST_RE) cells 1-99 behavior preservation

PRD I-D-c11 test target = Pattern 1 (lowercase "cells N, M, ..." form) cells 1-99 detection、
Pre/Post PRD I-D-c11 で behavior 不変 (= filter `<= 999` → `<= 99` uniform 化、effective behavior 不変)。

cells 5, 30, 31, 38, 70, 99

Expected detect: {5, 30, 31, 38, 70, 99}
