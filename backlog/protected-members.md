# PRD: `protected` メンバーの `pub(crate)` 変換

## Background

TypeScript の `protected` メンバーは現在 `Private`（Rust のデフォルト可視性）として出力される。Rust の `pub(crate)` が意味的に最も近い。

関連コード:

- `Visibility` enum: `src/ir.rs`（52-57行目）— 現在 `Public` と `Private` のみ
- `generate_vis`: `src/generator/mod.rs` — `Visibility` から可視性修飾子文字列を生成
- クラスメンバー抽出: `extract_class_info` — SWC AST の `accessibility` フィールドからメンバーごとの可視性を取得可能

## Goal

`protected` プロパティと `protected` メソッドが `pub(crate)` で出力される。

## Scope

- **IN**: `Visibility` enum に `PubCrate` バリアント追加
- **IN**: `generate_vis` で `PubCrate` → `pub(crate) ` 出力
- **IN**: `extract_class_info` で SWC AST の `accessibility` フィールドを参照し、`Protected` → `Visibility::PubCrate` に変換
- **OUT**: `private` キーワードの明示的処理（Rust のデフォルトと一致するため不要）

## Steps

1. `Visibility` enum に `PubCrate` を追加、`generate_vis` を更新
2. **RED**: protected プロパティ・メソッドのテストを追加（期待: `pub(crate)` 出力）
3. **GREEN**: `extract_class_info` でメンバーの `accessibility` をチェックし `Protected` → `PubCrate` に変換
4. **Quality check**

## Test plan

- protected プロパティ → `pub(crate)` フィールド
- protected メソッド → `pub(crate) fn` メソッド
- リグレッション: public メンバー → `pub`、private メンバー → 修飾子なし（既存と同一）

## Completion criteria

- protected メンバーが `pub(crate)` で出力される
- 全テスト pass、0 errors / 0 warnings
