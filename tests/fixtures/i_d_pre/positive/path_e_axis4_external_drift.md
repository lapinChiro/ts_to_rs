# Synthetic PRD: Axis 4 external file drift (positive fixture)

Impact Area table claims external file size mismatching actual = Axis 4 should flag.
Other axes clean.

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 1 variant
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Scope

- Cell 1 included

## Invariants

### INV-1 stub

Coverage: Cell 1.

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |

## Implementation Stage Tasks

### T1: stub

Covers cell 1.

## Test Plan

Tests for cell 1.

## Impact Area Audit Findings

### Empirical file path verify

| File | Status | Size (bytes) | LOC | Last modified | Empirical verify |
|------|--------|--------------|-----|---------------|------------------|
| `Cargo.toml` | exists | 999999999 | 99999 | 2099-01-01 | ✓ verified |
