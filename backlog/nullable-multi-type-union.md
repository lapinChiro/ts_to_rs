# nullable + 複数非null型の union 対応

## 背景・動機

`string | number | null` のような「nullable かつ複数の非 null 型を含む union」が現在エラーになる。`convert_union_type` は `has_null_or_undefined && non_null_types.len() == 1` の場合のみ `Option<T>` を生成し、`has_null_or_undefined && non_null_types.len() > 1` の場合は未処理でエラーを返す。

```typescript
// 現在エラーになるパターン
type Result = string | number | null;
function find(items: Item[]): Item | null | undefined;
```

## ゴール

- `T1 | T2 | null` が `Option<T1OrT2>` に変換される（enum + Option の組み合わせ）
- `T | null | undefined` が `Option<T>` に変換される（重複する nullable の統合）

## スコープ

### 対象

- `convert_union_type` の `has_null_or_undefined && non_null_types.len() > 1` 分岐の実装
- 非 null 型が 2 つ以上の場合に enum を生成し、`Option<Enum>` でラッピング

### 対象外

- 3 型以上の非 nullable union（`string | number | boolean` — 既に enum 生成で対応済み）
- discriminated union パターン（既に別実装で対応済み）

## 設計

### 技術的アプローチ

現在の `convert_union_type` の末尾にある `else` 分岐（179-183行）を変更:

```rust
// Before: エラー
} else {
    Err(anyhow!("union with multiple non-null types is not supported"))
}

// After: enum + Option
} else {
    // has_null_or_undefined && non_null_types.len() > 1
    // e.g., string | number | null → Option<StringOrF64>
    let mut rust_types = Vec::new();
    for ty in &non_null_types {
        let rust_type = convert_ts_type(ty, extra_items)?;
        let unwrapped = unwrap_promise(rust_type);
        if !rust_types.contains(&unwrapped) {
            rust_types.push(unwrapped);
        }
    }

    // 重複除去後に 1 型になった場合（null | undefined | T → Option<T>）
    if rust_types.len() == 1 {
        return Ok(RustType::Option(Box::new(rust_types.into_iter().next().unwrap())));
    }

    // enum を生成して Option でラッピング
    let mut variants = Vec::new();
    let mut name_parts = Vec::new();
    for rust_type in &rust_types {
        let variant_name = variant_name_from_type(rust_type);
        name_parts.push(variant_name.clone());
        variants.push(EnumVariant { name: variant_name, value: None, data: Some(rust_type.clone()), fields: vec![] });
    }
    let enum_name = name_parts.join("Or");
    extra_items.push(Item::Enum { vis: Visibility::Public, name: enum_name.clone(), serde_tag: None, variants });
    Ok(RustType::Option(Box::new(RustType::Named { name: enum_name, type_args: vec![] })))
}
```

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_union_type` の `else` 分岐

## 作業ステップ

- [ ] ステップ1（RED）: `string | number | null` → `Option<StringOrF64>` のテスト追加
- [ ] ステップ2（GREEN）: `else` 分岐で enum + Option を生成
- [ ] ステップ3（RED）: `string | null | undefined` → `Option<String>` のテスト追加（null + undefined の重複）
- [ ] ステップ4（GREEN）: 重複除去後に 1 型の場合の処理
- [ ] ステップ5: 回帰テスト・Quality check

## テスト計画

- `string | number | null` → `Option<StringOrF64>` enum が extra_items に生成される
- `string | null | undefined` → `Option<String>`（重複 nullable は 1 型に）
- `boolean | string | number | null` → `Option<BoolOrStringOrF64>`
- 回帰: 既存の union テスト（`string | null` → `Option<String>` が壊れないこと）
- 回帰: 非 nullable union（`string | number` → `StringOrF64`）

## 完了条件

- nullable + 複数非null型の union がエラーにならない
- 生成される型が `Option<Enum>` パターンに従う
- 全テスト pass、0 errors / 0 warnings
