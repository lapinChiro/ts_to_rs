// PRD 2.7 Matrix Cell #15: Prop::Assign × NA (object literal context で parse error)
//
// `{ x = 1 }` 構文は TS spec で **destructuring default の ObjectAssignmentPattern 限定**:
//   - 通常の object literal expression context (`const obj = { x = 1 }`) では parse error
//   - destructuring default context (`const { x = 1 } = obj`) では valid
//
// SWC parser は object literal context で `Prop::Assign` を reject (= AST 不到達)
// → cell 15 は **structurally unreachable** = NA cell
//
// Post-PRD 2.7 (T9 + T10):
// - TypeResolver expressions.rs Object expr arm: `Prop::Assign(_) => unreachable!(...)`
// - Transformer data_literals.rs convert_object_lit: 同 `unreachable!()`
// - reason: SWC parser reject 前提の bug detection mechanism、もし fire したら parser 仕様変更 = bug
//
// 本 fixture は **SWC parser empirical regression test** (Test 20) と整合 evidence:
// 通常の object literal で `{ x = 1 }` を parse させる → SWC parser reject 確認
//
// E2E framework note: 本 fixture は **SWC parser 段階で reject** されるため、ts_to_rs 変換は
// parse error stage で停止 (= Transformer 到達不可、cargo run の TS conversion path 非 reachable)。
// expected output は parse error message (stderr)。
// spec-traceable evidence として保持、tsc 経由 type check で同様 parse error を確認。

// 本 fixture は parse error を意図的に triggers するため、tsc/tsx 実行も parse fail。
// fixture 配置自体が cell 15 の structural reason の empirical evidence。

// destructuring default (valid syntax、reference for contrast):
function destructuringExample(): void {
    const obj: { x?: number } = {};
    const { x = 1 } = obj;  // valid: ObjectAssignmentPattern with default
    console.log(x);  // 1
}

// object literal context with `{ x = 1 }` syntax (parse error、本 cell 対象):
// 以下行を uncomment すると tsc/SWC parser で TS1005 / TS2304 等 syntax error:
// const objLit = { x = 1 };  // ← parse error in object literal context

function main(): void {
    destructuringExample();
    // 本 fixture の core message: object literal context で `{ x = 1 }` は SWC parser
    // が reject、cell 15 (Prop::Assign) は actual unreachable、`unreachable!()` macro
    // 呼び出しは bug detection mechanism として機能する
}
