# 低カバレッジファイル テスト観点レビュー

作成日: 2026-03-29

## 概要

CIでカバレッジ閾値89%を下回った（実測88.61%）ため、カバレッジが特に低い7ファイルを対象に、テストケース設計技法に基づく観点レビューを実施した。

**適用技法**: 同値分割、境界値分析、分岐網羅（C1）、デシジョンテーブル、AST バリアント網羅

**レビュー基準**: 件数（カバレッジ%）ではなく、テスト設計上の**観点の欠落**を特定する。

---

## 1. transformer/functions/destructuring.rs (61.95%)

**ユニットテスト: なし** — 全カバレッジがスナップショット/E2E経由のみ。

### 関数と分岐構造の分析

#### `convert_object_destructuring_param` (L13-142)

**同値分割 — ObjectPatProp の3バリアント**:

| パーティション | TS 入力例 | 期待動作 | テスト有無 |
|---|---|---|---|
| `Assign` (shorthand) | `{ x }: Point` | `let x = point.x;` | △ (snapshot経由) |
| `Assign` + default (to_string) | `{ x = "".toString() }: P` | `unwrap_or_else` | ✗ |
| `Assign` + default (StringLit) | `{ x = "default" }: P` | `unwrap_or_else` | ✗ |
| `Assign` + default (other) | `{ x = 42 }: P` | `unwrap_or` | ✗ |
| `KeyValue` (rename) | `{ x: newX }: Point` | `let new_x = point.x;` | △ |
| `KeyValue` (nested object) | `{ a: { b, c } }: T` | 再帰展開 | ✗ |
| `Rest` | `{ x, ...rest }: Point` | synthetic struct | ✗ |

**不足観点**:
1. **デフォルト値の3分岐** (L59-85): `to_string` / `StringLit` / other の3分岐それぞれを直接テストするケースがない。これはデシジョンテーブル技法の適用対象
2. **型アノテーションなしの場合** (L20-25): `serde_json::Value` へのフォールバック分岐
3. **ネストされた分割代入の再帰** (L105-113): `expand_fn_param_object_props` の再帰パスが直接テストされていない

#### `lookup_field_type` (L257-278)

**同値分割 — parent_type の分岐**:

| パーティション | テスト有無 |
|---|---|
| `RustType::Named` (直接) | ✗ |
| `RustType::Option(Named)` (アンラップ) | ✗ |
| `RustType::Option(非Named)` → None | ✗ |
| 他の型 → None | ✗ |
| type_args 空 → `reg.get()` | ��� |
| type_args あり → `reg.instantiate()` | ✗ |
| TypeDef::Struct → フィールド検�� | ✗ |
| TypeDef::Enum → None | ✗ |

**不足観点**: 4つの分岐 × 2つの条件 = 8パーティション全てが未テスト

#### `expand_rest_as_synthetic_struct` (L284-380)

**不���観点**:
1. **parent_type が None の場合のエラーパス** (L310-315)
2. **型が registry にない場合のエラーパス** (L324-333)
3. **explicit_fields の除外ロジック** (L336-349): sibling_props に Assign/KeyValue/Rest が混在するケース
4. **ジェネリック型のインスタンス化パス** (L317-321): `type_args` が空でない場合

---

## 2. transformer/classes/inheritance.rs (64.49%)

**ユニットテスト: なし**

### `rewrite_super_constructor` (L22-98)

**分岐網羅分析 — 5つの主要分岐**:

| 分岐 | 条件 | テスト有無 |
|---|---|---|
| super()引数数 ≠ 親フィールド数 | `args.len() != parent.fields.len()` | △ (1テストあり) |
| 既存 StructInit に merge | `has_struct_init == true` | ✗ |
| TailExpr(StructInit) のマージ | L60-69 | ✗ |
| Return(Some(StructInit)) のマージ | L71-82 | ✗ |
| StructInit なし → 新規作成 | L85-92 | ✗ |
| super_fields が空 | L85条件の else | ✗ |

**不足観点**:
1. **StructInit マージの2パターン** (TailExpr vs Return): 2つの match arm があるが個別テストなし
2. **super() 呼び出しなしのケース**: body に super() がない場合（super_fields が空のまま完了）
3. **super() 以外のステートメントの保持**: super() を除去した後、他のステートメントが `new_body` に残ることの検証

### `transform_class_with_inheritance` (L145-180)

**デシジョンテーブル — 7つの分岐**:

| is_abstract | is_parent | has_parent | parent_abstract | has_implements | 呼出関数 | テスト有無 |
|---|---|---|---|---|---|---|
| true | - | - | - | - | `generate_abstract_class_items` | ✗ |
| false | true | - | - | - | `generate_parent_class_items` | ✗ |
| false | false | true | true | - | `generate_child_of_abstract` | ✗ |
| false | false | true | false | true | `generate_child_class_with_implements` | ✗ |
| false | false | true | false | false | `generate_items_for_class(+parent)` | ✗ |
| false | false | false | - | true | `generate_class_with_implements` | ✗ |
| false | false | false | - | false | `generate_items_for_class(None)` | △ |

**不足観点**: 7分岐中6分岐が直接テストされていない。`pre_scan_classes` の結果を使った統合シナリオのテストが必要

---

## 3. transformer/statements/helpers.rs (65.74%)

**ユニットテスト: なし**

### `extract_conditional_assignment` (L38-88)

**同値分割 — 3パターン**:

| パターン | TS 入力 | テスト有無 |
|---|---|---|
| bare assignment | `if (x = expr)` | ✗ |
| assignment on left of comparison | `if ((x = expr) > 0)` | ✗ |
| assignment on right of comparison | `if (0 < (x = expr))` | ✗ |
| non-assignment → None | `if (x > 0)` | ✗ |
| nested parens | `if (((x = expr)))` | ✗ |

**不足観点**: 全パターン未テスト。特に「代入が左辺 vs 右辺」の対称性テストが重要

### `generate_truthiness_condition` / `generate_falsy_condition` (L139-182)

**同値分割 — 型による4分岐**:

| RustType | truthiness 生成 | falsy 生成 | テスト有無 |
|---|---|---|---|
| `F64` | `var != 0.0` | `var == 0.0` | ✗ |
| `String` | `!var.is_empty()` | `var.is_empty()` | ✗ |
| `Bool` | `var` | `!var` | ✗ |
| その他 (fallback) | `var` | `!var` | ✗ |

**不足観点**: truthiness / falsy は**対称性テスト**（同じ入力型に対して逆の条件が生成される）で検証すべき。`Option` 型は doc comment に「if let を使う」と書かれているが、この関数に渡された場合の挙動が未検証

---

## 4. transformer/expressions/patterns.rs (70.95%)

**���ニットテスト: なし** — ただし `expressions/tests/type_guards.rs` に関連テストが一部存在

### `try_convert_undefined_comparison` (L16-40)

**同値分割**:

| パーティション | テスト有無 |
|---|---|
| `x === undefined` → `is_none()` | △ |
| `x !== undefined` → `is_some()` | △ |
| `undefined === x` (逆順) | ✗ |
| `undefined !== x` (逆順) | ✗ |
| 非等値演算子 → None | ✗ |
| 両辺とも undefined でない → None | ✗ |

### `convert_in_operator` (L177-251)

**デシジョンテーブル — キー種別 × オブジェクト型**:

| キー | obj 型 | 期待 | テスト有無 |
|---|---|---|---|
| string lit | HashMap/BTreeMap | `contains_key()` | ✗ |
| string lit | HashMap, complex RHS expr | `todo!()` | ✗ |
| string lit | Named struct (field exists) | `true` | ✗ |
| string lit | Named struct (field missing) | `false` | ✗ |
| string lit | Named enum (tag field) | `true` | ✗ |
| string lit | Named enum (variant field) | `true`/`false` | ✗ |
| string lit | Named (unknown shape) | `todo!()` | ✗ |
| string lit | unknown type | `todo!()` | ✗ |
| non-string key | any | `todo!()` | ✗ |

**不足観点**: 9パーティション全てが直接テストされていない。`in` 演算子は TypeDef::Struct / Enum / HashMap の3つの分岐を全て通す必要がある

### `convert_instanceof` (L259-324)

**同値分割 — lhs_type による分岐**:

| lhs_type | 条件 | 期待 | テスト有無 |
|---|---|---|---|
| `Any` | - | `todo!()` | ✗ |
| `None` | - | `todo!()` | ✗ |
| `Named` (enum, variant match) | variant exists | `matches!()` | △ (snapshot) |
| `Named` (enum, no match) | variant not found | `BoolLit(false)` or `BoolLit(name==class)` | �� |
| `Named` (non-enum, same name) | name == class | `BoolLit(true)` | ✗ |
| `Named` (non-enum, diff name) | name != class | `BoolLit(false)` | ✗ |
| `Option(Named)` (match) | inner matches | `is_some()` | ✗ |
| `Option(Named)` (no match) | inner differs | `BoolLit(false)` | ✗ |
| `Option(非Named)` | - | `BoolLit(false)` | ✗ |
| other type | - | `BoolLit(false)` | ✗ |
| non-ident RHS | - | `todo!()` | ✗ |

### `extract_narrowing_guard` (L650-717)

**AST バリアント網羅**:

| 条件式パターン | guard 種別 | テスト有無 |
|---|---|---|
| `x instanceof Foo` (both ident) | `InstanceOf` | ✗ |
| `x instanceof expr` (non-ident RHS) | `None` | ✗ |
| `typeof x === "string"` | `Typeof` | ✗ |
| `x !== null` | `NonNullish(is_neq=true)` | ✗ |
| `x === undefined` | `NonNullish(is_neq=false)` | ✗ |
| `null !== x` (reversed) | `NonNullish` | ✗ |
| `x` (ident, truthy) | `Truthy` | ��� |
| `undefined` (keyword ident) | `None` | �� |
| `true` / `false` (keyword ident) | `None` | ✗ |
| non-bin, non-ident | `None` | ✗ |

**不足観点**: guard 抽出はナローイング変換の根幹。10パーティション全てが直接テストされていない。特に「キーワード除外」(L707) と「逆順 null/undefined」のテストは**サイレントな意味変更**を検出する重要な観点

---

## 5. pipeline/type_converter/type_aliases.rs (73.01%)

**ユニ���トテスト: なし**

### `convert_type_alias_items` (L7-62)

**分岐網羅**:

| 条件 | テスト有無 |
|---|---|
| conditional type → Ok(ty) | ✗ |
| conditional type → Err (fallback) | ✗ |
| keyof typeof → Some(items) | ✗ |
| keyof typeof → None (fallthrough) | ✗ |
| 通常 → `convert_type_alias()` | △ (snapshot) |

### `convert_type_alias` (L140-327)

**デシジョンテーブル — type_ann の種別 × サブ条件**:

| type_ann | サブ条件 | IR 出力 | テスト有無 |
|---|---|---|---|
| string literal union | - | Enum | △ |
| single string literal | - | Enum (1 variant) | ✗ |
| discriminated union | - | serde-tagged Enum | △ |
| general union | - | Enum | △ |
| intersection type | - | Struct (merged) | △ |
| function type | - | TypeAlias(Fn) | ✗ |
| tuple type | - | TypeAlias(Tuple) | ✗ |
| TsTypeLit + call sigs only | - | TypeAlias(Fn) | ✗ |
| TsTypeLit + methods only | - | Trait | ✗ |
| TsTypeLit + properties | - | Struct | △ |
| TsTypeLit + index signature | with type_ann | TypeAlias(HashMap) | ✗ |
| TsTypeLit + index signature | without type_ann | Error | ✗ |
| TsTypeLit + unsupported member | - | Error | ✗ |
| TsTypeLit + mixed methods+props | - | Struct (methods skipped) | ✗ |
| fallback → `convert_ts_type` | - | TypeAlias | △ |

**不足観点**:
1. **TsTypeLit の3-way分類** (call sigs / methods / properties): 最も複雑な分岐だが直接テストなし
2. **conditional type の infer パターン** (L418): `T extends Foo<infer U> ? U : never` → associated type 生成
3. **conditional type の true/false literal パターン** (L436): `T extends X ? true : false` → `bool`
4. **unused type param フィルタリング** (L21-24): 使用されない型パラメータが除去されるか

### `try_convert_keyof_typeof_alias` (L68-129)

**同値分割**:

| パーティション | テスト有無 |
|---|---|
| keyof typeof → struct の fields から enum 生成 | ✗ |
| keyof typeof → enum の string_values から enum 生成 | ✗ |
| 非 KeyOf operator → None | ✗ |
| 非 TsTypeQuery → None | ✗ |
| 非 Ident entity → None | ✗ |
| registry に型がない → None | ✗ |

---

## 6. pipeline/type_resolver/du_analysis.rs (73.08%)

**ユニッ���テスト: なし**

### `detect_du_switch_bindings` (L18-106)

**デシジョンテーブル — 複合条件**:

| discriminant | obj type | tag match | case test | body | テスト有無 |
|---|---|---|---|---|---|
| `obj.field` (member) | DU enum, tag matches | yes | string lit | non-empty | ✗ |
| `obj.field` | DU enum, tag differs | no → early return | - | - | ✗ |
| `obj.field` | non-enum | - → early return | - | - | ✗ |
| non-member expr | - | - → early return | - | - | ✗ |
| `obj.field` | DU enum | yes | non-string lit | → continue | ✗ |
| `obj.field` | DU enum | yes | string lit | empty (fall-through) | ✗ |

**不足観点**:
1. **fall-through ケース** (L69-71): empty body で pending_variant_names が蓄積され、次の non-empty body で複数バリアントが処理される
2. **field_exists_in_variant の判定** (L89-93): variant_fields にフィールドが存在しない場合のスキップ
3. **early return の網羅**: discriminant が member でない / obj が ident でない / type が Named でない / tag_field 不一致 — 4つの early return パス

### `collect_du_field_accesses_from_stmt_inner` (L133-169)

**AST Stmt バリアント網羅**:

| Stmt 種別 | テスト有無 |
|---|---|
| `Expr` | ✗ |
| `Return` (with arg) | ✗ |
| `Return` (without arg) | ✗ |
| `Decl::Var` | ✗ |
| `If` (with alt) | ✗ |
| `If` (without alt) | ✗ |
| `Block` | ✗ |
| `_ => {}` (unhandled) | ✗ |

### `collect_du_field_accesses_from_expr_inner` (L171-220)

**AST Expr バリアント網羅**:

| Expr 種別 | テスト有無 |
|---|---|
| `Member` (obj matches, prop is ident, not tag) | ✗ |
| `Member` (obj matches, prop is tag field) → skip | ✗ |
| `Member` (obj differs) → skip | ✗ |
| `Call` (callee + args) | ✗ |
| `Bin` (left + right) | ✗ |
| `Tpl` (exprs) | ✗ |
| `Paren` (inner) | ✗ |
| `Assign` (right) | ✗ |
| `Cond` (test + cons + alt) | ✗ |
| `_ => {}` (unhandled) | ✗ |

**不足観点**: tag_field の除外ロジック (L183) が正しく動作することの検証が特に重要。tag field を誤ってバインディングに含めるとサイレントな意味変更になる

---

## 7. registry/interfaces.rs (78.49%)

**ユニッ���テスト: なし**

### `collect_interface_fields` (L14-28)

**同値分割**:

| メンバー種別 | テスト有無 |
|---|---|
| `TsPropertySignature` → field 収集 | △ (snapshot経由) |
| 非 PropertySignature → skip | ✗ |
| 空の body | ✗ |

### `collect_interface_methods` (L31-90)

**デシジョンテーブル — パラメータ種別 × 型情報**:

| パラメータ種別 | 型アノテーション | テスト有無 |
|---|---|---|
| `Ident` + type_ann あり | 正常 | △ (snapshot) |
| `Ident` + type_ann なし | → None (filter_map skip) | ✗ |
| `Rest` + rest.type_ann あり | 正常 | ✗ |
| `Rest` + ident fallback type_ann | ident から型取�� | ✗ |
| `Rest` + type_ann なし | → None | ✗ |
| `Rest` + non-ident arg | name = "rest" | ✗ |
| 他のパラメータ種別 | → None | ✗ |
| return_type あり | 正常 | ✗ |
| return_type なし | None | ✗ |
| has_rest の判定 | rest param 存在 | ✗ |
| 同名メソッドの overload 蓄積 | entry().or_default().push() | ✗ |

**不足観点**:
1. **Rest パラメータの型取得フォールバック** (L59-65): `rest.type_ann` がない場合に `ident.type_ann` から取得するロジック — 条件分岐が2段階
2. **overload（同名メソッド）の蓄積**: HashMap の entry API で同名メソッドが Vec に蓄積される動作
3. **非 Ident キーの skip** (L40-41): `Expr::Ident` 以外のキーで `continue` するパス

### `collect_property_signature` (L93-115)

**同値分割**:

| 条件 | テスト有無 |
|---|---|
| Ident key + type_ann + non-optional | ✗ |
| Ident key + type_ann + optional → Option<T> | ✗ |
| Ident key + type_ann なし → None | ✗ |
| non-Ident key → None | ✗ |

---

## 総合分析

### 共通パターン

7ファイル全てに共通する問題:

1. **ユニットテストが完全に欠如**: 全ファイルに `#[cfg(test)] mod tests` がない。カバレッジはスナップショット/E2E テスト経由の間接的なものだけ
2. **エラーパスのテスト不足**: `Result::Err` を返す分岐や `None` を返す early return が直接テストされていない
3. **デフォルト値/フォールバック分岐の未検証**: 型アノテーションなし → fallback、registry に型がない → エラー、等の防御的分岐

### 優先度付き改善提案

#### 最優先（サイレントな意味変更リスク）

1. **patterns.rs の `extract_narrowing_guard`**: キーワード除外ロジック (L707) のテスト。`undefined` を変数として narrowing するとサイレントな意味変更
2. **du_analysis.rs の tag_field 除外**: tag field がバインディングに含まれるとパターンが壊れる
3. **helpers.rs の truthiness/falsy 対称性**: 型ごとの条件生成が正しく逆になることの検証

#### 高優先（分岐網羅の大幅改善）

4. **inheritance.rs の `transform_class_with_inheritance`**: 7分岐中6分岐未テスト
5. **type_aliases.rs の `convert_type_alias`**: TsTypeLit の3-way分類と conditional type パターン
6. **patterns.rs の `convert_in_operator`**: 9パーティション全未テスト

#### 中優先（基盤テスト）

7. **destructuring.rs のデフォルト値3分岐**: to_string / StringLit / other
8. **interfaces.rs の Rest パラメータフォールバック**: 型取得の2段階ロジック
9. **du_analysis.rs の AST バリアント網羅**: Stmt/Expr 再帰収集の全パス

### テスト追加による推定カバレッジ改善

| ファイル | 現在 | 主要未カバー分岐数 | 追加テスト観点数 |
|---|---|---|---|
| destructuring.rs | 61.95% | ~12 | 10 |
| inheritance.rs | 64.49% | ~10 | 8 |
| helpers.rs | 65.74% | ~10 | 9 |
| patterns.rs | 70.95% | ~25 | 20 |
| type_aliases.rs | 73.01% | ~18 | 15 |
| du_analysis.rs | 73.08% | ~15 | 12 |
| interfaces.rs | 78.49% | ~8 | 7 |

合計約81の新規テスト観点。全体の TOTAL Lines カバレッジを 88.61% → 89%+ に引き上げるには、特に行数の多い patterns.rs (451行) と type_aliases.rs (389行) の改善が最も効果的。
