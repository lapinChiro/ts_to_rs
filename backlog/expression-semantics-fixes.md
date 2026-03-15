# 式変換のセマンティクス修正（複数件）

## 背景・動機

式変換で複数の小規模なセマンティクス問題が確認されている:

1. **Math.max/min の 3 引数以上**: `Math.max(a, b, c)` → `a.max(b, c)` だが `f64::max` は 2 引数のみ
2. **オブジェクトスプレッドが複数不可**: `{...a, ...b}` で 2 つ目がエラー
3. **三項演算子の型不一致**: `cond ? "text" : 123` で if 式の分岐型が不一致
4. **代入式が条件内で無効**: `while (x = getValue())` がコンパイル不可
5. **super() の位置ベースマッピング**: 引数順序がフィールド宣言順と一致しない場合に誤り

## ゴール

上記 5 件がそれぞれ正しい Rust コードを生成する。

## スコープ

### 対象

- Math.max/min の可変引数チェーン化（`a.max(b).max(c)`）
- オブジェクトスプレッドの複数対応
- 三項演算子で型が異なる場合の対処（enum 生成 or コメント）
- 代入式の条件内使用（`loop` + `let` パターンに変換）
- super() マッピングの改善（named field matching の検討）

### 対象外

- 全ての TS 式パターンの網羅的対応

## 設計

### 技術的アプローチ

**1. Math.max/min**: 3 引数以上の場合、チェーン呼び出しに変換: `a.max(b).max(c)`

**2. 複数スプレッド**: 最初の spread をベースにし、2 つ目以降のフィールドで上書き:
```rust
let mut _obj = base1.clone();
// base2 のフィールドで上書き
_obj.field = base2.field;
```

**3. 三項演算子**: 分岐の型が異なる場合、非 nullable union と同じ enum 生成パターンを使用。ただしコンパイルエラーになるケースは限定的（Rust の型推論で解決されることが多い）なので、TODO コメントの付与で対応。

**4. 代入式**: `while (x = f())` → `loop { let x = f(); if !x { break; } ... }` に変換。検出は `Expr::Assign` が条件位置にある場合。

**5. super()**: 現状の位置ベースマッピングは一般的な TS パターンと一致するため、ドキュメントコメントで制約を明記する。named field matching は型情報が不足しており実装困難。

### 影響範囲

- `src/transformer/expressions/mod.rs` — Math 変換、スプレッド、三項演算子
- `src/transformer/statements/mod.rs` — while 条件の代入式検出
- `src/transformer/classes.rs` — super() のドキュメント
- テストファイル

## 作業ステップ

- [ ] ステップ1（RED）: `Math.max(a, b, c)` → `a.max(b).max(c)` テスト
- [ ] ステップ2（GREEN）: Math 可変引数チェーン化
- [ ] ステップ3（RED）: `{...a, ...b, x: 1}` のテスト
- [ ] ステップ4（GREEN）: 複数スプレッド対応
- [ ] ステップ5: 三項演算子・代入式は TODO コメント付与に留める（影響が限定的）
- [ ] ステップ6: Quality check

## テスト計画

- `Math.max(a, b, c)` → `a.max(b).max(c)`
- `Math.min(a, b, c)` → `a.min(b).min(c)`
- `{...a, ...b}` → コンパイル可能な Rust
- 回帰: 既存の Math、スプレッド、三項演算子テスト

## 完了条件

- Math.max/min が 3 引数以上でコンパイル可能
- 複数スプレッドがエラーにならない
- 全テスト pass、0 errors / 0 warnings
