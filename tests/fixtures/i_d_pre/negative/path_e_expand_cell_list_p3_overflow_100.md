# Test fixture: expand_cell_list Pattern 3 cells 100+ structural rejection (INV-5)

PRD I-D-c11 INV-5 test target = Pattern 3 cells 100+ structural rejection。
3-digit numbers are rejected via regex word boundary failure、Pre/Post PRD 共に empty set return。

The bracket-list set with overflow values: 100, 200, 500 in a bracketed form is structurally rejected。

Expected detect (Pre/Post-PRD I-D-c11): empty set (regex word boundary fail で structural reject)
