# type_converter / ts_type_info 移行状況の包括的分析

**Base commit**: bf80284 → **Batch 4d-A 適用後** に更新

## 1. サマリ

Batch 4b で `convert_ts_type()` を `TsTypeInfo` 経由の2ステップ変換に移行したが、移行が **annotation 位置の型解決** にとどまり、**declaration 位置の変換**（型エイリアス・インターフェース宣言 → IR Item）は旧パイプラインが残存している。

Batch 4d-A で以下のクリーンアップを実施済み:
- ~~Dead code (`type_converter/indexed_access.rs`)~~ → 削除済
- ~~resolve → type_converter の逆依存~~ → `string_to_pascal_case`, `sanitize_rust_type_name` を `ir/mod.rs` に移動済
- ~~resolve 内 `try_simplify_identity_mapped` 重複~~ → 統一済

残存する構造的問題:

1. **型解決ロジックの二重実装**: 12箇所で同一アルゴリズムが別表現で並存（4d-A で 3 箇所解消）
2. **責務混合**: declaration 変換関数が型解決と Item 構築を同一関数内で実行
3. **SWC AST への直接パターンマッチ**: convert_ts_type() をバイパスして旧パイプラインで型判定する箇所が12箇所
4. **SyntheticTypeRegistry への二重登録**: ユーティリティ型で TypeResolver と Transformer の両方が登録

## 2. 二重実装の完全マッピング

| # | 機能 | type_converter | resolve | 重複度 |
|---|------|---------------|---------|--------|
| 1 | Discriminant 検出 | `unions.rs:69-139` | `resolve/intersection.rs:480-515` | 完全重複（別表現） |
| 2a | is_symbol_filter_noop | `intersections.rs:20-52` | `resolve/mod.rs:307-328` | 完全重複（別表現） |
| 2b | identity mapped 簡約 | `intersections.rs:63-130` | `resolve/mod.rs:331-361` | 完全重複（別表現） |
| ~~2c~~ | ~~identity mapped 簡約~~ | — | ~~`resolve/intersection.rs:277-311`~~ | ~~resolve内で重複~~ **4d-A で解消** |
| 3 | Partial\<T\> | `utilities.rs:152-197` | `resolve/utility.rs:15-63` | 部分重複（vis差異） |
| 4 | Required\<T\> | `utilities.rs:200-243` | `resolve/utility.rs:66-113` | 部分重複（vis差異） |
| 5 | Pick\<T,K\> | `utilities.rs:264-310` | `resolve/utility.rs:116-162` | 部分重複（vis差異） |
| 6 | Omit\<T,K\> | `utilities.rs:313-359` | `resolve/utility.rs:165-211` | 部分重複（vis差異） |
| 7 | NonNullable\<T\> | `utilities.rs:362-381` | `resolve/utility.rs:214-228` | 完全重複 |
| 8 | フィールド抽出 | `utilities.rs:28-58` | `resolve/intersection.rs:252-274` | 部分重複 |
| 9 | メソッドシグネチャ変換 | `interfaces.rs:379-428` | `resolve/intersection.rs:86-112` (inline) | 部分重複 |
| 10 | extract_string_keys | `utilities.rs:245-261` | `resolve/utility.rs:309-320` | 完全重複 |
| 11 | resolve_utility_inner_fields | `utilities.rs:67-107` | `resolve/utility.rs:234-270` | 完全重複（別表現） |
| 12 | resolve_utility_inner_with_conversion | `utilities.rs:110-149` | `resolve/utility.rs:273-308` | 完全重複（別表現） |
| 13 | capitalize_first | `utilities.rs:384-389` | — | 片方のみ |

**重要**: #8-12 は declaration 変換の内部で呼ばれるヘルパーであり、型解決と Item 構築が分離されていれば不要になる。

## 3. 新パイプラインをバイパスしている箇所

`convert_ts_type()` は `resolve_ts_type()` に完全委譲しているが、declaration 変換関数は SWC AST に直接パターンマッチして型判定を行っている:

| ファイル | 関数 | 直接マッチしている AST 型 |
|---------|------|------------------------|
| unions.rs:355-386 | try_convert_general_union | TsKeywordType |
| unions.rs:400-412 | try_convert_general_union | TsTypeLit |
| unions.rs:414-433 | try_convert_general_union | TsIntersectionType |
| unions.rs:196-237 | try_convert_string_literal_union | TsLitType |
| intersections.rs:162-180 | extract_intersection_members | TsTypeLit |
| intersections.rs:182-215 | extract_intersection_members | TsTypeRef |
| intersections.rs:264-270 | extract_variant_fields | TsTypeLit |
| type_aliases.rs:68-115 | try_convert_keyof_typeof_alias | TsTypeOperator |
| type_aliases.rs:331-367 | try_convert_fn_type_alias | TsFnType |
| type_aliases.rs:455-525 | convert_const_assertion_type | TsKeywordType, TsTypeRef, TsLitType |
| utilities.rs:67-107 | resolve_utility_inner_fields | TsTypeRef |
| utilities.rs:245-261 | extract_string_keys | TsLitType, TsUnionType |

これらは `convert_to_ts_type_info()` → `TsTypeInfo` を経由せず、SWC AST に直接依存している。

## 4. 混合責務関数の分解可能性

| 関数 | 型解決行数 | Item構築行数 | 分解可能性 |
|------|----------|-----------|-----------|
| try_convert_intersection_type | ~180 | ~190 | 条件付き可能 |
| try_convert_discriminated_union | ~27 | ~119 | 可能 |
| try_convert_general_union | ~155 | ~10 | 可能 |
| convert_utility_partial/required/pick/omit | 各30-60 | 各40-50 | 可能（resolve と重複） |
| convert_type_alias (dispatcher) | 全体が混合 | — | 条件付き可能 |
| convert_interface_as_struct_and_trait | ~32 | ~40 | 不可能（意図的設計） |

**分解の核心的障壁**: resolve_ts_type() は `RustType` のみを返し、「どの Item 型を生成すべきか」の情報（discriminated union か？struct fields は？methods は？）を返さない。この情報は現在 AST から直接抽出されている。

## 5. SyntheticTypeRegistry 二重登録の分析

ユーティリティ型（Partial/Required/Pick/Omit）で TypeResolver（resolve/utility.rs）と Transformer（type_converter/utilities.rs）の両方が `push_item()` で登録する。

- パイプライン順序: TypeResolver → merge → Transformer
- merge 後に Transformer が同名で上書き → フィールド定義が同一のため実害なし
- ただし、**概念的には二重実行**であり無駄

## 6. Dead code

~~`src/pipeline/type_converter/indexed_access.rs`（259行）~~ → **4d-A で削除済み**

## 7. 逆方向の依存

~~`resolve/mod.rs` が `type_converter` の `sanitize_rust_type_name`, `string_to_pascal_case` を import~~ → **4d-A で `ir/mod.rs` に移動し解消済み**。`resolve/` → `type_converter` への import は 0 件。

## 8. 理想的なアーキテクチャ

```
SWC AST
  │
  ├─ Phase 1: AST → TsTypeInfo（既存の convert_to_ts_type_info）
  │   [AST 依存性を局所化]
  │
  ├─ Phase 2: TsTypeInfo → RustType（既存の resolve_ts_type）
  │   [型解決 — 唯一の実装。synthetic 登録もここで完結]
  │
  └─ Phase 3: Declaration → Item（type_converter の Item 構築責務のみ残す）
      [TsTypeInfo + RustType を入力として、Item::Struct/Enum/Trait を構築]
      [型解決は一切行わない。resolve の結果を使うのみ]
```

現在の type_converter は Phase 2 と Phase 3 が混合している。Phase 2 を resolve に統一し、Phase 3 のみを type_converter に残す（またはモジュール名を変更して責務を明確化する）のが理想。

## 9. 既知の関連 TODO

- I-347: TypeDef::substitute_types が Function/ConstValue 未処理（plan.md Batch 4c）
- I-348: TypeDef::type_params() が Function を返さない（plan.md Batch 4c）

これらは TypeDef のジェネリック化の残課題であり、本移行とは独立して対応可能。
