// PRD 2.7 Matrix Cell #6: ClassMember::StaticBlock × typeof narrow inside body
//
// Pre-PRD 2.7: TypeResolver `visit_class_body` の `_ => {}` 黙殺で StaticBlock body
// 内 typeof guard が narrow event push されない → silent type widening risk
//
// Post-PRD 2.7 (T8 改修): visit_block_stmt 経由 walk + scope 管理で StaticBlock
// body 内 narrow event push、TypeResolver narrowed_type query 正確化
//
// 本 fixture は static block 内の **local 変数の typeof narrow** に focus
// (= class static field access の type resolution は別 concern として除外、
//  cell 6 の core issue = StaticBlock body の walk 経路追加 を sharply isolate)
//
// Spec stage E2E fixture (red 状態): TypeResolver の narrow event push 不在 →
// Transformer に narrowed_type 提供されず → typeof guard が unresolved type と
// 判定 → "typeof on unresolved type" Tier 2 error
//
// Implementation stage で T8 改修後に green 化 (StaticBlock body の visit 経路で
// local 変数の type info が record される)

class Initializer {
    static result: string = "uninit";

    static {
        // local 変数の typeof narrow inside static block
        const value: string | number = "default-string";
        if (typeof value === "string") {
            // narrow scope: value: string
            Initializer.result = value + "-narrowed";
        } else {
            // narrow scope: value: number
            Initializer.result = "num:" + value;
        }
    }
}

function main(): void {
    console.log(Initializer.result);  // "default-string-narrowed"
}
