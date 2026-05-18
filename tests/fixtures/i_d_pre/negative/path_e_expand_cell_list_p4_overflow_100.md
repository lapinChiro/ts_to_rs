# Test fixture: expand_cell_list Pattern 4 cells 100+ structural rejection (INV-5)

PRD I-D-c11 INV-5 test target = Pattern 4 cells 100+ structural rejection。
3-digit numbers are rejected via regex word boundary failure、Pre/Post PRD 共に empty set return。

| 100 | overflow |
| 200 | overflow |
| 500 | overflow |

Expected detect (Pre/Post-PRD I-D-c11): empty set (regex digit-only 上限 reject)
