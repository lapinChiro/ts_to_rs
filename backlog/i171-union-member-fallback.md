# I-171: union 未対応メンバーの型付きフォールバック

## 背景・動機

union 型の変換で、関数型・タプル型等の未対応メンバーが `serde_json::Value` 相当の `RustType::Any` バリアントに畳み込まれる。複数の異なる未対応型が同じ `Other{N}(Any)` バリアントになるため、パターンマッチで区別不能。

さらに、型エイリアス経由の union（`try_convert_general_union`）にはフォールバックが存在するが、インライン union（`convert_union_type`）ではエラー伝播（`?`）により変換自体が失敗する一貫性の問題がある。

## ゴール

1. 関数型メンバーは `Box<dyn Fn(params) -> ret>` バリアントとして変換される
2. タプル型メンバーは `(T1, T2, ...)` バリアントとして変換される
3. 残りの未対応型は `Other{N}(serde_json::Value)` に型名ヒント付きのバリアント名で変換される
4. インライン union と型エイリアス union で同一のフォールバック戦略が適用される

## スコープ

### 対象

- 関数型 union メンバーの `Box<dyn Fn>` バリアント変換
- タプル型 union メンバーのタプルバリアント変換
- インライン union（`convert_union_type`）へのフォールバック導入
- 型エイリアス union（`try_convert_general_union`）の既存フォールバックの改善（バリアント名に型ヒント付与）
- フォールバック発生時の診断メッセージ（`--report-unsupported` 出力への統合）

### 対象外

- conditional type（`T extends U ? A : B`）の完全変換 — 独立した設計課題
- mapped type の union メンバー変換 — I-200 の範囲
- ジェネリック型引数を含む union メンバー — I-100 の範囲

## 設計

### 技術的アプローチ

`try_convert_general_union` (types/mod.rs:1619-1730) のフォールバック分岐を拡張し、`convert_union_type` (types/mod.rs:272-383) にも同じロジックを適用する。

#### メンバー型別の変換ルール

| TS メンバー型 | 生成される enum バリアント | IR 表現 |
|--------------|-------------------------|---------|
| 関数型 `(x: T) => U` | `Fn(Box<dyn Fn(T) -> U>)` | `EnumVariant { data: Some(RustType::Fn { .. }) }` |
| タプル型 `[T, U]` | `Tuple(T, U)` | `EnumVariant { data: Some(RustType::Tuple(..)) }` |
| その他未対応 | `Other{TypeHint}{N}(serde_json::Value)` | `EnumVariant { data: Some(RustType::Any) }` |

#### 共通フォールバック関数の抽出

```rust
fn convert_union_member_with_fallback(
    member: &TsType,
    variants: &mut Vec<EnumVariant>,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<()>
```

この関数を `convert_union_type` と `try_convert_general_union` の両方から呼び出し、一貫性を確保する。

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/transformer/types/mod.rs` | `convert_union_type` にフォールバック導入、`try_convert_general_union` のフォールバック改善、共通関数抽出 |
| `src/ir.rs` | EnumVariant の変更は不要（既存構造で表現可能） |
| `src/generator/items.rs` | 変更不要（既存の enum 生成ロジックで対応可能） |
| テストファイル | 新規テストケース追加 |

## 作業ステップ

- [ ] ステップ1（RED）: 関数型を含む union の変換テストを書く（期待: `Fn(Box<dyn Fn(f64) -> String>)` バリアント）
- [ ] ステップ2（RED）: タプル型を含む union の変換テストを書く
- [ ] ステップ3（RED）: インライン union で未対応型がフォールバックするテストを書く（現在はエラーになることを確認）
- [ ] ステップ4（GREEN）: 共通フォールバック関数を実装し、関数型・タプル型の変換を追加
- [ ] ステップ5（GREEN）: `convert_union_type` に共通フォールバック関数を適用
- [ ] ステップ6（GREEN）: `try_convert_general_union` の既存フォールバックを共通関数に置換
- [ ] ステップ7（REFACTOR）: バリアント名の型ヒント付与（`OtherFn0`, `OtherTuple1` 等）
- [ ] ステップ8: E2E スナップショットテスト

## テスト計画

### 単体テスト

- 関数型メンバーを含む union → `Fn(Box<dyn Fn>)` バリアント生成
- タプル型メンバーを含む union → `Tuple(T1, T2)` バリアント生成
- 複数の未対応型を含む union → 各型が区別可能なバリアントに変換
- インライン union での未対応型 → フォールバック（エラーにならない）
- 型エイリアス union での未対応型 → 同一のフォールバック結果
- サポート済み型（文字列リテラル、数値リテラル、型参照）は既存動作を維持

### E2E テスト

- 関数型を含む union の変換スナップショット

## 完了条件

- 全テストパターンが GREEN
- `convert_union_type` と `try_convert_general_union` が同一のフォールバック関数を使用
- `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- `cargo fmt --all --check` が通る
- `cargo test` が全パス
- `cargo llvm-cov` のカバレッジ閾値を満たす
