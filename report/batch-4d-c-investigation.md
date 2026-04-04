# Batch 4d-C: declaration 変換の責務分離 — 調査レポート

**Base commit**: `fca5f85`

## 概要

declaration 変換（union/intersection の型エイリアス変換）が SWC AST を直接操作しており、Batch 4d-B で確立した `SWC → TsTypeInfo → resolve` の 2 ステップパイプラインを迂回している。この二重パスが DRY 違反と保守コスト増大の根本原因。

## 二重実装の全体像

### 1. find_discriminant_field

| 場所 | 入力型 | 行 |
|------|--------|-----|
| `src/pipeline/type_converter/unions.rs:69` | `&[&swc_ecma_ast::TsTypeLit]` | 69-139 |
| `src/ts_type_info/resolve/intersection.rs:445` | `&[TsTypeInfo]` | 445-480 |

同一ロジック（全バリアントに共通の string literal フィールドを検出し、値のユニーク性を検証）が異なる AST 表現で実装されている。resolve 版はテストカバレッジが充実（12+ テスト、L596-1100+）。

### 2. extract_variant_info / extract_discriminated_variant

| 場所 | 入力型 | 行 |
|------|--------|-----|
| `src/pipeline/type_converter/unions.rs:150` | `&TsTypeLit` (SWC) | 150-194 |
| `src/ts_type_info/resolve/intersection.rs:483` | `&TsTypeInfo` | 483-526 |

**意味的差異**: SWC 版は生の discriminant 値を返す（`"click"` → `EnumValue::Str("click")`）。resolve 版は PascalCase 名を返し、`value: None` で登録する。declaration 変換では serde tag に生の値が必要。

### 3. extract_variant_fields

| 場所 | 入力型 | 行 |
|------|--------|-----|
| `src/pipeline/type_converter/intersections.rs:257` | `&TsType` (SWC) | 257-310 |
| `src/ts_type_info/resolve/intersection.rs:529` | `&TsTypeInfo` | 529-564 |

### 4. extract_intersection_members（SWC AST 直接操作）

`src/pipeline/type_converter/intersections.rs:153-232`

TsTypeLit を直接走査し `convert_property_signature` を呼ぶ。resolve の `resolve_type_literal_fields` と同一知識。

### 5. try_convert_general_union 内の SWC 直接操作

`src/pipeline/type_converter/unions.rs:400-433`

TsTypeLit と TsIntersectionType 分岐で `convert_property_signature` を直接呼ぶ。

## resolve 版で不足している機能

1. **discriminant raw value の保持**: `extract_discriminated_variant` が PascalCase 名のみ返し、`EnumValue::Str(raw)` に必要な生の文字列値を返さない
2. **serde_tag の伝播**: resolve 版の `resolve_intersection_with_union` は discriminant をフィールド名として返すが、`EnumVariant::value` に `EnumValue::Str` を設定しない（synthetic enum だから不要という判断）

## バッチ化の検討

### 同一バッチに含めるべき
なし。4d-C は純粋なリファクタリング（DRY 解消）であり、機能追加を混ぜるとリファクタリングの検証が困難になる。

### 関連するが別バッチ
- **I-101**: ジェネリック型を含む intersection — 機能追加（4d-C のリファクタ後に着手が容易）
- **I-314**: intersection の未使用型パラメータ（E0091）— 機能追加
- **I-338/I-318**: synthetic 型の構造的同値性 — 別の根本原因（RC-8）

### 先行すべきイシュー
なし。4d-C は Batch 4d-B 完了を前提とし、それ以外の依存はない。

## 設計方針

### 原則
declaration 変換関数は SWC AST の型判別（union/intersection/literal の分岐）のみを行い、型解決とフィールド抽出は全て TsTypeInfo → resolve パスに委譲する。

### 具体的な変更

#### Phase A: resolve の extract_discriminated_variant を拡張

`extract_discriminated_variant` の戻り値を `(raw_value: String, pascal_name: String, fields: Vec<StructField>)` に変更。これにより declaration 変換が `EnumValue::Str(raw_value)` を設定可能になる。

resolve 側の呼び出し元も更新（`resolve_intersection_with_union` で `value: Some(EnumValue::Str(raw_value))` を設定 — これは resolve 側のバグ修正でもある）。

#### Phase B: find_discriminant_field の統一

1. resolve の `find_discriminant_field` を `pub(crate)` に変更
2. `extract_discriminated_variant` を `pub(crate)` に変更
3. `extract_variant_fields` を `pub(crate)` に変更
4. `resolve_type_literal_fields` を `pub(crate)` に変更（既に pub(crate) でない場合）

#### Phase C: declaration 変換の SWC 直接操作を TsTypeInfo 経由に書き換え

1. `try_convert_discriminated_union`: SWC union → `convert_to_ts_type_info` → `find_discriminant_field` + `extract_discriminated_variant`
2. `distribute_intersection_with_union`: 同上
3. `extract_intersection_members`: 各メンバーを TsTypeInfo に変換 → `resolve_type_literal_fields` 等で抽出
4. `try_convert_general_union` の TsTypeLit/intersection 分岐: TsTypeInfo 変換 → resolve 委譲

#### Phase D: 不要コードの削除

1. `unions.rs` の `find_discriminant_field` を削除
2. `unions.rs` の `extract_variant_info` を削除
3. `unions.rs` の `is_string_literal_type` を削除
4. `intersections.rs` の `extract_intersection_members` を削除
5. `intersections.rs` の `extract_variant_fields` を削除

## リスク分析

- **低リスク**: `find_discriminant_field` の共有は入力型が変わるだけで同一ロジック。テストカバレッジ充実
- **中リスク**: `extract_discriminated_variant` の戻り値変更は呼び出し元全てに影響。ただし呼び出し元は 2 箇所のみ
- **要注意**: declaration 変換では `vis` パラメータの制御（Public/Private）があるが、resolve 版は常に `Some(Visibility::Public)`。declaration 変換側で上書きが必要
