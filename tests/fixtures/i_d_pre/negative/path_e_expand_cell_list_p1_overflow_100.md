# Test fixture: expand_cell_list Pattern 1 cells 100+ structural rejection (INV-5)

PRD I-D-c11 INV-5 test target = Pattern 1 cells 100+ structural rejection。
3-digit numbers are rejected via regex word boundary failure、Pre/Post PRD 共に empty set return。

cells 100, 200, 500

Expected detect (Pre/Post-PRD I-D-c11): empty set (regex word boundary fail で structural reject)
