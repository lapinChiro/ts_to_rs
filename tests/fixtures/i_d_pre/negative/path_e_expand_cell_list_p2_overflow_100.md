# Test fixture: expand_cell_list Pattern 2 cells 100+ structural rejection (INV-5)

PRD I-D-c11 INV-5 test target = Pattern 2 cells 100+ structural rejection。
3-digit numbers are rejected via regex word boundary failure、Pre/Post PRD 共に empty set return。

Cell 100, Cell 200, Cell 500

Expected detect via P2 individual regex (CELL_STANDALONE_RE direct): empty set (regex digit-only 上限 reject)
Note: expand_cell_list overall level で CELL_LIST_RE (IGNORECASE) が catch 試行するが、Pattern 1 内 body parse でも 3-digit number は word boundary fail で reject = overall empty set
