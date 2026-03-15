# 型注記位置の intersection / TsTypeLit での struct 生成

## 背景・動機

`convert_ts_type` が型注記位置の以下 2 パターンを処理できない:

1. **TsTypeLit**: `x: { a: string, b: number }` — インライン型リテラルが型注記位置で未対応（`unsupported type: TsTypeLit` エラー）
2. **intersection**: `x: Foo & Bar` — intersection 型が型注記位置では最初の型のみ使用し、2 番目以降の情報を消失（T-3 Critical）

type alias 位置（`type X = { a: string }` / `type X = A & B`）では既に struct 生成が実装済み。型注記位置でも `extra_items` 伝搬を活用して同等の処理を行えばよい。

Hono で計 3 件（TsTypeLit 2件 + intersection 1件）の変換エラーを解消する。

## ゴール

- `convert_ts_type` が `TsTypeLit` を処理し、合成 struct を `extra_items` に追加して `RustType::Named` を返す
- `convert_ts_type` が intersection を処理し、フィールド統合した合成 struct を `extra_items` に追加して `RustType::Named` を返す
- 既存のスナップショットテストに回帰がない

## スコープ

### 対象

- `convert_ts_type` に `TsTypeLit` の match arm を追加
- `convert_ts_type` の intersection 分岐を first-type フォールバックから合成 struct 生成に変更
- 合成 struct の命名規則を設計（コンテキストベース or カウンタベース）
- `extra_items` 伝搬で合成 struct を収集

### 対象外

- ジェネリック型を含む intersection（`Required<Omit<...>> & { ... }`）— ジェネリック型展開が前提
- `convert_param` の inline type literal 処理（既に実装済み、重複ではない — `convert_param` は関数パラメータ専用、ここではより汎用的な型注記位置を扱う）

## 設計

### 技術的アプローチ

#### 1. TsTypeLit の match arm 追加

`convert_ts_type` に `TsType::TsTypeLit` のアームを追加。

```rust
TsType::TsTypeLit(type_lit) => {
    let mut fields = Vec::new();
    for member in &type_lit.members {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                fields.push(convert_property_signature(prop, extra_items)?);
            }
            _ => return Err(anyhow!("unsupported type literal member")),
        }
    }
    // Generate synthetic struct name using a counter or hash
    let struct_name = generate_synthetic_name("TypeLit");
    extra_items.push(Item::Struct {
        vis: Visibility::Public,
        name: struct_name.clone(),
        type_params: vec![],
        fields,
    });
    Ok(RustType::Named { name: struct_name, type_args: vec![] })
}
```

#### 2. intersection の合成 struct 生成

既存の `try_convert_intersection_type`（type alias 位置）のロジックを流用:

```rust
TsIntersectionType(intersection) => {
    let mut all_fields = Vec::new();
    for ty in &intersection.types {
        match ty.as_ref() {
            TsType::TsTypeLit(lit) => {
                // Extract fields directly
                for member in &lit.members { ... }
            }
            TsType::TsTypeRef(ref_) => {
                // Look up in TypeRegistry for named types
                // If not found, convert recursively and skip
            }
            _ => {
                // Unsupported intersection member — skip or error
            }
        }
    }
    let struct_name = generate_synthetic_name("Intersection");
    extra_items.push(Item::Struct { ... });
    Ok(RustType::Named { name: struct_name, type_args: vec![] })
}
```

#### 3. 合成 struct の命名

`convert_param` では `to_pascal_case(&format!("{fn_name}_{param_name}"))` を使っている。型注記位置では関数名やパラメータ名が利用できない場合がある。

→ `thread_local` カウンタで `_TypeLit0`, `_TypeLit1` のようなユニーク名を生成する。

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_ts_type` の match にアーム追加、intersection 分岐修正
- テストファイル全般

## 作業ステップ

### Part A: TsTypeLit

- [ ] ステップ1（RED）: `{ a: string, b: number }` が `RustType::Named` を返し `extra_items` に struct が追加されるテスト
- [ ] ステップ2（GREEN）: `convert_ts_type` に `TsTypeLit` アーム追加
- [ ] ステップ3: 合成 struct 命名の実装

### Part B: intersection

- [ ] ステップ4（RED）: `Foo & { c: number }` が統合 struct を生成するテスト
- [ ] ステップ5（GREEN）: intersection 分岐の修正
- [ ] ステップ6: TypeRegistry 経由の名前付き型解決

### Part C: 統合

- [ ] ステップ7: スナップショット更新
- [ ] ステップ8: Quality check

## テスト計画

### TsTypeLit

- `{ a: string }` → `extra_items` に `struct _TypeLit0 { pub a: String }` が追加され、`RustType::Named { name: "_TypeLit0" }` が返される
- `{ a: string, b?: number }` → optional フィールドが `Option<f64>` になる
- 既存の `convert_param` の inline type literal テストに回帰がない

### intersection

- `Foo & { c: number }` → `extra_items` に `struct _Intersection0 { pub x: f64, pub c: f64 }` が追加（Foo のフィールド + c）
- `{ a: string } & { b: number }` → 両方の type literal のフィールドが統合
- 重複フィールド → エラー

## 完了条件

- `convert_ts_type` が `TsTypeLit` に対して struct を生成し `Named` を返す
- `convert_ts_type` が intersection に対して統合 struct を生成し `Named` を返す（first-type フォールバックが消えている）
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
