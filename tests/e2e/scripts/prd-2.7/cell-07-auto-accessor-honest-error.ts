// PRD 2.7 Matrix Cell #7: ClassMember::AutoAccessor × Tier 2 honest error
//
// AutoAccessor (TC39 Stage 3 / TS 5.0+ stable) `accessor x: T = init` 構文。
// PRD 2.7 では (b) Tier 2 error report 化 (silent drop 排除) のみ実施。
// 完全 Tier 1 化 (= Rust struct field + getter/setter pair emission) は
// I-201-A (decorator なし subset、L3) + I-201-B (decorator framework、L1) で別 PRD 達成。
//
// Pre-PRD 2.7 状態 (audit 2026-04-27):
// - TypeResolver: `visit_class_body` の `_ => {}` で AutoAccessor を silent drop
// - Transformer: `classes/mod.rs:165-171` で `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` を return (既実装、Tier 2 honest error)
//
// Post-PRD 2.7:
// - TypeResolver: 明示 no-op (Rule 10(d-2) compliance、reason comment 付き empty arm)
// - Transformer: 既存 UnsupportedSyntaxError 維持
// - ast-variants.md AutoAccessor entry を Tier 2 (Unsupported, honest error reported via UnsupportedSyntaxError) に明示更新
//
// E2E framework note: 本 fixture は cargo run で **Tier 2 honest error 出力** が expected
// (= ts_to_rs 変換 fail、stderr に UnsupportedSyntaxError、exit code 非 0)。
// runtime stdout 比較は不可、spec-traceable evidence として保持。
// red 状態 verify: 現状の error message format (`anyhow!("AutoAccessor")` 等) と
// post-PRD 2.7 の `UnsupportedSyntaxError::new("AutoAccessor", span)` format との diff。

class Counter {
    accessor count: number = 0;

    increment(): void {
        this.count += 1;
    }
}

function main(): void {
    const c = new Counter();
    c.increment();
    c.increment();
    console.log(c.count);  // tsc/tsx runtime: 2
                            // ts_to_rs: Tier 2 UnsupportedSyntaxError (post-PRD 2.7) or anyhow! (pre-PRD 2.7)
}
