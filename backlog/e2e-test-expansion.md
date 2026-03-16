# E2E テストカバレッジの大幅拡充（I-63 + I-64）

## 背景・動機

E2E テストは「TS 実行結果 = Rust 実行結果」を検証する唯一のテスト層であり、変換の意味的正確性を保証するガードレールである。現在 E2E は 9 スクリプトしかなく、トランスパイラが対応済みの機能の大半が未検証。

他のテスト層との役割分担:
- **単体テスト**: 個々の関数の入出力を検証（内部ロジックの正しさ）
- **スナップショット**: 生成 Rust コードの文字列を固定（意図しない変更の検出）
- **コンパイルチェック**: 生成 Rust が `cargo check` を通るか（型レベルの正しさ）
- **E2E**: TS と Rust の実行結果が一致するか（**意味的正確性の唯一の保証**）

新機能を追加する前に E2E を拡充することで、既存機能の潜在バグを発見し、今後の全開発のガードレールを確立する。

併せて、`tests/e2e/scripts/` にある未使用の `.rs` ファイル（4 件）を削除する（I-64）。

## ゴール

1. E2E テストスクリプトが 9 → 最低 20 に増加する
2. 以下の高リスク機能が全て E2E でカバーされる:
   - enum, switch, optional chaining, nullish coalescing, closures, object destructuring, array destructuring, class inheritance, discriminated union, generics
3. 未使用の `.rs` オーバーライドファイルが削除される
4. E2E テスト追加時にバグが発見された場合、修正も含めて完了する

## スコープ

### 対象

新規 E2E スクリプトの作成（以下の機能カテゴリ）:

| カテゴリ | スクリプト名 | テスト内容 |
|---|---|---|
| enum | `enum_basic.ts` | enum 定義、値の参照、関数の引数/戻り値 |
| switch | `switch_match.ts` | switch/case の各分岐、default、文字列/数値マッチ |
| optional chaining | `optional_chaining.ts` | `?.` によるプロパティアクセス、null/非 null パス |
| nullish coalescing | `nullish_coalescing.ts` | `??` のフォールバック、null/非 null パス |
| closures | `closures.ts` | arrow function、変数キャプチャ、高階関数 |
| object destructuring | `object_destructuring.ts` | shorthand, rename, 関数パラメータ |
| array destructuring | `array_destructuring.ts` | 基本分割、rest、swap パターン |
| class inheritance | `class_inheritance.ts` | extends、メソッドオーバーライド、super |
| discriminated union | `discriminated_union.ts` | タグ付き union + switch による分岐 |
| generics | `generics.ts` | generic 関数呼び出し、generic クラスのインスタンス化 |
| object/array spread | `spread_ops.ts` | `...` によるオブジェクト/配列の展開 |
| do-while + break/continue | `loop_control.ts` | do-while, labeled break, continue |
| template literals | `template_literals.ts` | バッククォート文字列、式埋め込み |
| optional fields | `optional_fields.ts` | Optional プロパティの参照、undefined チェック |

未使用 `.rs` ファイルの削除:
- `tests/e2e/scripts/functions.rs`
- `tests/e2e/scripts/string_ops.rs`
- `tests/e2e/scripts/loops.rs`
- `tests/e2e/scripts/error_handling.rs`

### 対象外

- async/await の E2E（Rust runner に tokio ランタイムが必要。別途対応）
- stdin/ファイル I/O/HTTP の E2E（I-49/50/51 で別途対応）
- テストフレームワーク自体の変更（現在の stdout 比較方式で十分）
- getter/setter の E2E（実行可能だが、テストのためのスキャフォールディングが過大）

## 設計

### 技術的アプローチ

各スクリプトは以下の形式に従う:

```typescript
// 必要な型・関数の定義
function main(): void {
    // 各パターンを実行し、結果を console.log で出力
    console.log("label:", value);
}
```

E2E テストフレームワーク（`e2e_test.rs`）は変更不要。各スクリプトに対応するテスト関数を追加するのみ。

**バグ発見時の対応方針:** E2E スクリプトの追加中にトランスパイラのバグが発見された場合:
1. バグの内容と影響を記録
2. 修正が小さい場合（10 行以内）→ 同ステップ内で修正
3. 修正が大きい場合 → TODO に記録し、スクリプトは一時スキップまたは回避策を使用

### 影響範囲

- `tests/e2e/scripts/` — 新規 .ts ファイル追加、.rs ファイル削除
- `tests/e2e_test.rs` — 新規テスト関数追加
- `src/` — バグ発見時のみ修正

## 作業ステップ

- [ ] ステップ 0: 未使用 .rs ファイルの削除（I-64）
  - `tests/e2e/scripts/` から 4 つの .rs ファイルを削除

- [ ] ステップ 1: 基本制御構造（switch, do-while, break/continue）
  - `switch_match.ts`: 数値/文字列の switch、default 分岐
  - `loop_control.ts`: do-while ループ、labeled break/continue
  - テスト関数を `e2e_test.rs` に追加

- [ ] ステップ 2: 値の構造（enum, discriminated union）
  - `enum_basic.ts`: enum 定義、値参照、match
  - `discriminated_union.ts`: タグ付き union + switch 分岐
  - テスト関数を `e2e_test.rs` に追加

- [ ] ステップ 3: null 安全性（optional chaining, nullish coalescing, optional fields）
  - `optional_chaining.ts`: `?.` の null/非 null パス
  - `nullish_coalescing.ts`: `??` のフォールバック
  - `optional_fields.ts`: Optional プロパティの参照
  - テスト関数を `e2e_test.rs` に追加

- [ ] ステップ 4: 関数とクロージャ
  - `closures.ts`: arrow function、変数キャプチャ、callback パターン
  - テスト関数を `e2e_test.rs` に追加

- [ ] ステップ 5: 分割代入と展開
  - `object_destructuring.ts`: shorthand, rename
  - `array_destructuring.ts`: 基本分割、rest、swap
  - `spread_ops.ts`: object/array spread
  - テスト関数を `e2e_test.rs` に追加

- [ ] ステップ 6: クラスとジェネリクス
  - `class_inheritance.ts`: extends, super, メソッドオーバーライド
  - `generics.ts`: generic 関数、generic クラス
  - テスト関数を `e2e_test.rs` に追加

- [ ] ステップ 7: テンプレートリテラル
  - `template_literals.ts`: バッククォート文字列、式埋め込み
  - テスト関数を `e2e_test.rs` に追加

## テスト計画

- 各 E2E スクリプトは TS と Rust の stdout が完全一致すること
- `cargo test` で全テスト通過（既存 + 新規）
- バグ発見時は修正後に全テスト再実行

## 完了条件

1. `cargo test` 全テスト通過
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
3. `cargo fmt --all --check` 通過
4. E2E テストが 20 件以上
5. 高リスク機能（enum, switch, optional chaining, nullish coalescing, closures, destructuring, class inheritance, discriminated union, generics）が全て E2E でカバー
6. 未使用 .rs ファイルが削除されている
7. E2E 追加中に発見されたバグが修正済み、または TODO に記録済み
