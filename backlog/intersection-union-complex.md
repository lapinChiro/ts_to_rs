# PRD: intersection + union の複合型（`(A & B) | C`）

## Background

intersection 基本パターン（`TsTypeLit` 同士のフィールド統合）と union 型参照バリアントは実装済み。`(A & B) | C` のような、union メンバーに intersection が含まれる複合型が未対応。

`src/transformer/types/mod.rs` の `try_convert_general_union` は union メンバーとして `TsLitType`、`TsKeywordType`、`TsTypeRef` を処理するが、`TsIntersectionType` を union メンバーとして扱う分岐がない。

## Goal

```typescript
type X = { a: string } & { b: number } | { c: boolean }
```

が enum に変換される。intersection メンバーはフィールドを統合した匿名型としてバリアントデータに展開する。

期待出力:

```rust
enum X {
    Variant0 { a: String, b: f64 },
    Variant1 { c: bool },
}
```

## Scope

- **IN**: `try_convert_general_union` で union メンバーが `TsIntersectionType` の場合、フィールドを統合してバリアントデータに変換
- **OUT**: 名前付き型参照を含む intersection メンバー（`A & B` で A, B が型参照の場合。TypeRegistry による型解決が必要）

## Steps

1. **RED**: `type X = { a: string } & { b: number } | { c: boolean }` のテストを追加（期待: 2バリアントの enum）
2. **GREEN**: `try_convert_general_union` に `TsIntersectionType` アームを追加。intersection 内の `TsTypeLit` メンバーのフィールドを統合
3. **Quality check**

## Test plan

- intersection + オブジェクトリテラル型の union: フィールド統合バリアント + 通常バリアント
- intersection + キーワード型の union: フィールド統合バリアント + キーワードバリアント
- リグレッション: 既存の union テスト（リテラル型、型参照）が同一出力

## Completion criteria

- union メンバーとしての intersection が正しく処理される
- 全テスト pass、0 errors / 0 warnings
