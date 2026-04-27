// PRD 2.7 Matrix Cell #17: Transformer convert_object_lit `_ => Err(anyhow!(...))` 改修
//
// Pre-PRD 2.7 (audit 2026-04-27):
// - data_literals.rs:259-263 で `_ => Err(anyhow!("unsupported object literal property"))`
// - format 不整合 broken window: 他 module は `UnsupportedSyntaxError::new()` 経由
//
// Post-PRD 2.7 (T10 + C6 修正):
// - `_` arm 削除 (Rule 10(d-1) compliance)
// - Prop::Method/Getter/Setter/Assign 各 variant explicit enumerate
// - Prop::Method/Getter/Setter は `UnsupportedSyntaxError::new("Prop::*", span)` で error return
// - Prop::Assign は `unreachable!()` (cell 15 と整合)
// - format 統一: 全 Tier 2 error が UnsupportedSyntaxError 経由、resolve_unsupported() で line/col 含む user-facing message
//
// 本 fixture は cell 12-14 と内容類似だが、focus は **Transformer の Tier 2 error format
// 統一** (cell 12-14 は TypeResolver visit + Transformer error report の双方を verify)。
// E2E framework note: cargo run で Tier 2 honest error を出力 (post-PRD 2.7 で format 統一済)。

const formatter = {
    format(value: string | number): string {
        if (typeof value === "string") {
            return `"${value}"`;
        }
        return String(value);
    },

    parse(s: string): number {
        return Number(s);
    }
};

function main(): void {
    console.log(formatter.format("hello"));  // "\"hello\""
    console.log(formatter.format(42));       // "42"
    console.log(formatter.parse("100"));     // 100
}
