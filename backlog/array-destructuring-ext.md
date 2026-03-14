# PRD: 配列分割代入の拡張（スキップ・rest）

## Background

基本パターン `[a, b] = arr` は実装済み（`src/transformer/statements/mod.rs` の `try_convert_array_destructuring`）。`Pat::Array` 内の `Pat::Ident` 要素のみ処理している。以下が未対応:

- スキップ: `[a, , b] = arr`
- rest: `[first, ...rest] = arr`
- ネスト: `[[a], [b]] = arr`

## Goal

- `[a, , b] = arr` → `let a = arr[0]; let b = arr[2];`（スキップ位置のインデックスを正しく算出）
- `[first, ...rest] = arr` → `let first = arr[0]; let rest = arr[1..].to_vec();`

## Scope

- **IN**: スキップ要素（配列パターン内のホール）の処理
- **IN**: rest 要素（`...rest`）の処理
- **OUT**: ネスト分割代入（複雑度が高い。TODO として記録）

## Steps

1. **RED**: `[a, , b]` スキップパターンのテストを追加（期待: `arr[0]`, `arr[2]`）
2. **GREEN**: `try_convert_array_destructuring` で空スロット（`Pat::Invalid` または `None`）を検出し、インデックスをスキップ
3. **RED**: `[first, ...rest]` rest パターンのテストを追加（期待: `arr[0]`, `arr[1..].to_vec()`）
4. **GREEN**: `Pat::Rest` を検出し、スライス構文を生成
5. **E2E**: フィクスチャファイルを追加
6. **Quality check**

## Test plan

- スキップ: `[a, , b]` → `arr[0]`, `arr[2]` でアクセス
- rest: `[first, ...rest]` → `arr[0]` + `arr[1..].to_vec()`
- 混合: `[a, , ...rest]` → `arr[0]` + `arr[2..].to_vec()`
- リグレッション: 基本パターン `[a, b]` が既存と同一出力

## Completion criteria

- スキップ・rest パターンが正しく変換される
- 全テスト pass、0 errors / 0 warnings
