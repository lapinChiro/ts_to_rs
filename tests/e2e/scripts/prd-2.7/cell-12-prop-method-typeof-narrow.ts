// PRD 2.7 Matrix Cell #12: Prop::Method × typeof narrow inside body
//
// Pre-PRD 2.7 (audit 2026-04-27):
// - TypeResolver expressions.rs:331+ ast::Expr::Object arm の `_ => { count++ }`
//   暗黙 silent drop で Prop::Method body 内の type-resolve / narrow event push 不在
// - Transformer convert_object_lit (data_literals.rs:259-263) の `_ => Err(anyhow!(...))`
//   で format 不整合 broken window
//
// Post-PRD 2.7 (T9 + T10 改修):
// - TypeResolver: visit_method_function 同等処理 (function-level scope + visit_block_stmt
//   経由 body walk + return type setup) で Prop::Method body 内 narrow event push
// - Transformer: `UnsupportedSyntaxError::new("Prop::Method", method_prop.function.span)` で
//   honest error return (Tier 2 honest、format 統一)
// - 完全 Tier 1 化 (= Transformer で Rust 等価 emission) は I-202 別 PRD で達成
//
// E2E framework note: 本 fixture は cargo run で Tier 2 honest error が expected
// (post-PRD 2.7、Transformer で UnsupportedSyntaxError を return)。runtime stdout 比較
// は不可、spec-traceable evidence として保持。
// red 状態 verify: 現状 (pre-PRD 2.7) は Tier 2 error (anyhow!) を出力するが format
// 不整合 (`unsupported object literal property`)、post-PRD 2.7 で
// `UnsupportedSyntaxError::new("Prop::Method", span)` 経由 format 統一。

const handler = {
    process(x: string | number): number {
        if (typeof x === "string") {
            // narrow scope: x: string (post-PRD 2.7 で TypeResolver narrow event push)
            return x.length;
        }
        // narrow scope: x: number (else branch)
        return x * 2;
    }
};

function main(): void {
    console.log(handler.process("hello"));  // 5 (string.length)
    console.log(handler.process(42));       // 84 (number * 2)
}
