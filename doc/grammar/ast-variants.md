# SWC AST Variant Catalog (Beta)

**Version snapshot**: SWC `swc_ecma_ast` v21, `swc_ecma_parser` v35 (2026-04-17)
**Pilot validated**: I-050-a (2026-04-17) — Expr::Lit の 3 variant を matrix 列挙に使用、漏れなし

本ドキュメントは `spec-first-prd.md` の grammar-derived matrix 作成時に参照する。
PRD の入力次元 (AST shape) を列挙する際、本カタログの全 variant について
「この機能に関与するか否か」を判定する。

**更新トリガー**: SWC crate upgrade 時 / IR の AST-facing enum 変更時に同時更新。

---

## 凡例

- **Tier 1**: 現行 pipeline で Rust コードを emit する (handled)
- **Tier 2**: 現行 pipeline で unsupported / error 扱い (名前のみ列挙)
- **Tier 3**: SWC が accept するが ts_to_rs が見ない (NA justify で除外)

---

## 1. Expr (式)

### Tier 1 — Handled

| Variant | 変換先 / 処理 | 主要 handler |
|---------|-------------|-------------|
| `Ident` | IR `Expr::Ident` / 特殊 (`undefined`→None, `NaN`→f64::NAN, `Infinity`→f64::INFINITY) | `convert_expr` |
| `Lit` | 各 Lit variant に delegate | `convert_lit` |
| `Bin` | typeof/undefined/enum 比較, instanceof, in, `??` の特殊処理 + 通常 BinOp | `convert_bin_expr` |
| `Tpl` | `format!()` macro | `convert_template_literal` |
| `Paren` | inner expr を unwrap | `convert_expr` |
| `Member` | field access / method / computed index | `convert_member_expr` |
| `This` | `Expr::Ident("self")` | `convert_expr` |
| `Assign` | 代入式 (NullishAssign 含む全 AssignOp) | `convert_assign_expr` |
| `Update` | `++`/`--` → desugar to `i = i + 1.0` | `convert_update_expr` |
| `Arrow` | closure / Box\<dyn Fn\> | `convert_arrow_expr` |
| `Fn` | 関数式 | `convert_fn_expr` |
| `Call` | 関数呼び出し (builtin remap 含む) | `convert_call_expr` |
| `New` | constructor 呼び出し | `convert_new_expr` |
| `Array` | `vec![...]` | `convert_array_lit` |
| `Object` | struct init / HashMap | `convert_object_lit` |
| `Cond` | 三項演算子 / if-else expr | `convert_cond_expr` |
| `Unary` | `!`, `-`, `typeof`, `+` (numeric coercion) | `convert_unary_expr` |
| `TsAs` | type assertion (f64/bool cast のみ実処理、他 passthrough) | `convert_expr` |
| `OptChain` | `?.` → Option chain (`map`/`and_then`) | `convert_opt_chain_expr` |
| `Await` | `expr.await` | `convert_expr` |
| `TsNonNull` | `!` assertion → inner passthrough | `convert_expr` |

### Tier 2 — Unsupported (名前のみ)

| Variant | 備考 |
|---------|------|
| `Seq` | カンマ式 (I-114) |
| `Yield` | generator (ts_to_rs 未対応) |
| `MetaProp` | `import.meta`, `new.target` |
| `Class` | クラス式 (I-093) |
| `TaggedTpl` | タグ付きテンプレートリテラル (I-110) |
| `SuperProp` | `super.x` |
| `TsTypeAssertion` | `<T>x` (旧 syntax、TsAs と類似) |
| `TsSatisfies` | `x satisfies T` (TS 4.9+, I-115) |
| `TsConstAssertion` | `x as const` |
| `TsInstantiation` | `f<T>` (TS 4.7+ instantiation expression) |
| `PrivateName` | `#field` (class 外の standalone) |
| `Invalid` | parser error marker |

### Tier 3 — NA

| Variant | NA 理由 |
|---------|--------|
| `JSXMember` | JSX — ts_to_rs scope 外 (TS syntax 仕様ではなく JSX 拡張) |
| `JSXNamespacedName` | 同上 |
| `JSXEmpty` | 同上 |
| `JSXElement` | 同上 |
| `JSXFragment` | 同上 |

---

## 2. Stmt (文)

### Tier 1 — Handled

| Variant | 変換先 / 処理 | 主要 handler |
|---------|-------------|-------------|
| `Return` | `Stmt::Return` (spread 展開チェック付き) | `convert_stmt` |
| `Decl` | Var/Fn/Class/TsInterface/TsTypeAlias/TsEnum に分岐 | `convert_stmt` |
| `If` | `Stmt::If` + narrowing | `convert_if_stmt` |
| `Expr` | 式文 (spread/nullish-assign intercept) | `convert_stmt` |
| `Throw` | `Err(...)` return | `convert_throw_stmt` |
| `While` | `while` loop | `convert_while_stmt` |
| `ForOf` | `for item in iter` | `convert_for_of_stmt` |
| `For` | C-style for → while/range 変換 | `convert_for_stmt` |
| `Break` | `break` (label 対応) | `convert_stmt` |
| `Continue` | `continue` (label 対応) | `convert_stmt` |
| `Labeled` | `'label: loop` | `convert_labeled_stmt` |
| `DoWhile` | `loop { ... if !cond { break; } }` | `convert_do_while_stmt` |
| `Try` | `match` / Result | `convert_try_stmt` |
| `Switch` | `match` | `convert_switch_stmt` |
| `ForIn` | `for key in obj.keys()` | `convert_for_in_stmt` |
| `Empty` | no-op | `convert_stmt` |
| `Block` | block scope | `convert_block_or_stmt` |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `With` | strict mode で禁止 (TS は strict) |
| `Debugger` | runtime 専用 |

---

## 3. Decl (宣言)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `Var` | 変数宣言 (destructuring 含む) |
| `Fn` | 関数宣言 |
| `Class` | クラス宣言 |
| `TsInterface` | trait / struct 変換 |
| `TsTypeAlias` | type alias / fn type alias |
| `TsEnum` | enum 変換 |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `TsModule` | namespace 宣言 |
| `Using` | `using` resource 宣言 (TC39 Stage 3) |

---

## 4. Lit (リテラル)

### Tier 1 — Handled

| Variant | 変換先 |
|---------|--------|
| `Num` | `f64` (NumberLit) |
| `Str` | `String` (enum variant lookup 含む) |
| `Bool` | `bool` |
| `Null` | `None` |
| `Regex` | `Regex::new(...)` (flags 対応) |
| `BigInt` | `i128` (range check) |

### Tier 3 — NA

| Variant | NA 理由 |
|---------|--------|
| `JSXText` | JSX scope 外 |

---

## 5. BinaryOp (二項演算子)

### Tier 1 — Handled

| Variant | IR / 処理 |
|---------|----------|
| `Add` | `BinOp::Add` (string concat 特殊処理) |
| `Sub` | `BinOp::Sub` |
| `Mul` | `BinOp::Mul` |
| `Div` | `BinOp::Div` |
| `Mod` | `BinOp::Mod` |
| `EqEq` | `BinOp::Eq` |
| `EqEqEq` | `BinOp::Eq` |
| `NotEq` | `BinOp::NotEq` |
| `NotEqEq` | `BinOp::NotEq` |
| `Lt` | `BinOp::Lt` |
| `LtEq` | `BinOp::LtEq` |
| `Gt` | `BinOp::Gt` |
| `GtEq` | `BinOp::GtEq` |
| `LogicalAnd` | `BinOp::LogicalAnd` |
| `LogicalOr` | `BinOp::LogicalOr` |
| `BitAnd` | `BinOp::BitAnd` |
| `BitOr` | `BinOp::BitOr` |
| `BitXor` | `BinOp::BitXor` |
| `LShift` | `BinOp::Shl` |
| `RShift` | `BinOp::Shr` |
| `ZeroFillRShift` | `BinOp::UShr` |
| `NullishCoalescing` | `unwrap_or` / `or` / `or_else` (I-022) |
| `InstanceOf` | `convert_instanceof()` 特殊処理 |
| `In` | `contains_key()` / field 存在チェック |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `Exp` | `**` 累乗 (I-082, `f64::powf()` 予定) |

---

## 6. UnaryOp (単項演算子)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `Bang` | `UnOp::Not` |
| `Minus` | `UnOp::Neg` |
| `TypeOf` | 型別 static string / Any runtime typeof |
| `Plus` | numeric coercion (`parse::<f64>()`) |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `Void` | `void expr` (I-086) |
| `Delete` | `delete obj.x` (I-086) |
| `Tilde` | bitwise NOT (I-086) |

---

## 7. AssignOp (代入演算子)

**全 variant handled** (Tier 1 complete):

| Variant | 処理 |
|---------|------|
| `Assign` (`=`) | 直接代入 |
| `AddAssign` (`+=`) | desugar: `target = target + right` |
| `SubAssign` (`-=`) | 同上 |
| `MulAssign` (`*=`) | 同上 |
| `DivAssign` (`/=`) | 同上 |
| `ModAssign` (`%=`) | 同上 |
| `BitAndAssign` (`&=`) | 同上 |
| `BitOrAssign` (`\|=`) | 同上 |
| `BitXorAssign` (`^=`) | 同上 |
| `LShiftAssign` (`<<=`) | 同上 |
| `RShiftAssign` (`>>=`) | 同上 |
| `ZeroFillRShiftAssign` (`>>>=`) | 同上 |
| `AndAssign` (`&&=`) | 同上 |
| `OrAssign` (`\|\|=`) | 同上 |
| `NullishAssign` (`??=`) | `pick_strategy()` 分岐 (I-142) |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `ExpAssign` (`**=`) | `**` (Exp) が unsupported のため desugar 不可 (I-082 依存) |

---

## 8. UpdateOp (更新演算子)

**全 variant handled** (Tier 1 complete):

| Variant | 処理 |
|---------|------|
| `PlusPlus` | prefix: `{ i += 1; i }`, postfix: `{ let _old = i; i += 1; _old }` |
| `MinusMinus` | 同上 (decrement) |

---

## 9. AssignTarget (代入ターゲット)

### SimpleAssignTarget — Tier 1

| Variant | 処理 |
|---------|------|
| `Ident` | 識別子代入 |
| `Member` | field / index 代入 |

### SimpleAssignTarget — Tier 2

| Variant | 備考 |
|---------|------|
| `SuperProp` | `super.x = v` |
| `Paren` | `(x) = v` |
| `OptChain` | `x?.y = v` |
| `TsAs` | `(x as T) = v` |
| `TsSatisfies` | `(x satisfies T) = v` |
| `Invalid` | parser error |

### PatternAssignTarget — Tier 2

未対応 (destructuring assignment)。

---

## 10. Pat (パターン)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `Ident` | 単純識別子バインディング |
| `Object` | オブジェクトデストラクチャリング |
| `Array` | 配列デストラクチャリング |
| `Rest` | `...rest` パターン |
| `Assign` | デフォルト値パターン |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `Expr` | 式パターン (TS では稀) |
| `Invalid` | parser error |

---

## 11. ObjectPatProp (オブジェクトパターンプロパティ)

**全 variant handled** (Tier 1 complete):

| Variant | 処理 |
|---------|------|
| `Assign` | `{ x }` / `{ x = default }` |
| `KeyValue` | `{ oldName: newName }` |
| `Rest` | `{ ...rest }` |

---

## 12. PropName (プロパティ名)

| Variant | Status | 備考 |
|---------|--------|------|
| `Ident` | Tier 1 | 識別子プロパティ |
| `Str` | Tier 1 | 文字列リテラルプロパティ |
| `Computed` | Tier 2 | 計算プロパティ (部分対応、I-121) |
| `Num` | Tier 2 | 数値プロパティ |
| `BigInt` | Tier 2 | BigInt プロパティ |

---

## 13. MemberProp (メンバーアクセスプロパティ)

**全 variant handled** (Tier 1 complete):

| Variant | 処理 |
|---------|------|
| `Ident` | `obj.field` |
| `PrivateName` | `obj._private` |
| `Computed` | `obj[expr]` |

---

## 14. ClassMember (クラスメンバー)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `ClassProp` | instance / static プロパティ |
| `Constructor` | constructor (param property 抽出含む) |
| `Method` | instance メソッド |
| `PrivateMethod` | private メソッド |
| `PrivateProp` | private プロパティ |
| `StaticBlock` | static 初期化ブロック |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `TsIndexSignature` | index signature (filter out) |
| `Empty` | 空メンバー (no-op) |
| `AutoAccessor` | JS auto accessor |

---

## 15. ModuleDecl (モジュール宣言)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `ExportDecl` | export + 宣言 → pub visibility |
| `Import` | import → use / mod |
| `ExportNamed` | named re-export |
| `ExportAll` | wildcard re-export |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `ExportDefaultDecl` | default export |
| `ExportDefaultExpr` | default export 式 |
| `TsImportEquals` | `import X = require(...)` |
| `TsExportAssignment` | `export = X` |
| `TsNamespaceExport` | `export as namespace X` |

---

## 16. TsType (TypeScript 型)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `TsKeywordType` | 全 12 keyword (string/number/boolean/void/any/unknown/never/object/null/undefined/bigint/symbol) |
| `TsArrayType` | `T[]` → `Vec<T>` |
| `TsTypeRef` | 型参照 (generics 対応) |
| `TsUnionOrIntersectionType` | union / intersection |
| `TsParenthesizedType` | 括弧型 unwrap |
| `TsFnOrConstructorType` | 関数型 / constructor 型 |
| `TsTupleType` | タプル型 |
| `TsIndexedAccessType` | `T[K]` indexed access |
| `TsTypeLit` | 型リテラル (object shape) |
| `TsLitType` | リテラル型 (string/number/bool/bigint) |
| `TsConditionalType` | 条件型 |
| `TsMappedType` | mapped type |
| `TsTypePredicate` | 型述語 |
| `TsInferType` | `infer T` |
| `TsTypeQuery` | `typeof x` 型 |

### Tier 2 — Partial / Unsupported

| Variant | Status | 備考 |
|---------|--------|------|
| `TsTypeOperator` | Partial | `keyof` / `readonly` のみ、`unique` 未対応 |
| `TsImportType` | Unsupported | `import("...").Type` |
| `TsRestType` | Unsupported | 型位置の rest (I-094) |
| `TsOptionalType` | Unsupported | tuple optional element |
| `TsThisType` | Unsupported | `this` 型 |

---

## 17. TsTypeElement (型要素)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `TsPropertySignature` | プロパティ (optional/readonly) |
| `TsMethodSignature` | メソッドシグネチャ |
| `TsCallSignatureDecl` | callable signature |
| `TsConstructSignatureDecl` | constructor signature |
| `TsIndexSignature` | index signature (記録のみ) |

### Tier 2 — Unsupported

| Variant | 備考 |
|---------|------|
| `TsGetterSignature` | getter signature |
| `TsSetterSignature` | setter signature |
