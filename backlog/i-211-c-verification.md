# I-211-c: ECMAScript 標準型の検証 + E2E テスト + ベンチマーク

## 背景・動機

I-211-a（オーバーロード対応）と I-211-b（ECMAScript 型抽出）が完了し、TypeRegistry に `String`, `Array`, `Date` 等の ECMAScript 標準型が登録された。この PRD では、追加された型情報が変換パイプライン全体で正しく機能することを検証する。

検証が必要な理由:

1. **メソッドチェーン型追跡**: `resolve_method_return_type` が `String.split` → `Vec<String>` のように解決するか。これまで TypeRegistry に型がなかったため、この経路は Web API 型でしかテストされていない
2. **instanceof + any narrowing**: `x instanceof Date` で `Date` struct が TypeRegistry に存在するようになり、any_narrowing が生成する enum バリアント `Date(Date)` が有効な型参照になるか
3. **ベンチマーク効果**: I-112c の残存エラー 54 件のうち、TypeRegistry の型情報不足が原因のケースがどの程度解消されるか

## ゴール

1. ビルトインメソッドチェーンの E2E テストが通る（String, Array, Date）
2. instanceof ビルトイン型のスナップショットテストが正しい Rust コードを出力する
3. ベンチマーク効果が測定され、エラー数の変化が TODO に記録される

## スコープ

### 対象

- String メソッドチェーンの E2E テスト（`trim`, `split`, `join`, `toLowerCase`）
- Array メソッドチェーンのスナップショットテスト（`map`, `filter`）
- Date メソッドのスナップショットテスト（`toISOString`）
- instanceof ビルトイン型のスナップショットテスト（`Date`, `Error`, `RegExp`）
- ベンチマーク実行 + 効果測定 + TODO 更新

### 対象外

- I-211 の変更範囲外の既存バグ（I-211 以前から存在し、I-211 の変更に起因しない問題）
- Rust 標準ライブラリ側の API 差異（例: TS の `String.split` と Rust の `str::split` のセマンティクス差異）

### テスト中に発見したバグの対応方針

バグの種類に応じて対応を分ける:

1. **I-211 の変更に起因するバグ**（例: 新しい型データの不正、オーバーロード解決の誤り） → **この PRD 内で修正する**。I-211 の完了条件の一部
2. **I-211 の変更で顕在化した既存バグ**（例: TypeRegistry に型が追加されたことで、今まで到達しなかったコードパスが実行されエラーになる） → **原因を特定し、修正が局所的（1-2 箇所）なら修正する**。修正が広範囲に及ぶ場合は TODO に記録
3. **I-211 と無関係の既存バグ** → **TODO に記録する**

## 設計

### 技術的アプローチ

#### 1. String メソッドチェーンの E2E テスト

既存の `tests/e2e/scripts/method_chain.ts` を活用。現在のテストケース:

```typescript
const result1: string = "  hello world  ".trim().split(" ").join("-");
const result2: string = "  HELLO  ".toLowerCase().trim();
const result3: string = "a-b-c".split("-").join(" ");
const result4: string = "hello world".toUpperCase().split(" ").join("_");
```

これらは型注釈（`: string`）があるため TypeRegistry に依存しない可能性がある。**型注釈なし**のケースを追加して TypeRegistry からの型追跡を検証する:

```typescript
// 型注釈なし — TypeRegistry の String.split 戻り値型に依存
const parts = "hello world".split(" ");
console.log("parts:", parts.join(","));

const trimmed = "  hello  ".trim();
console.log("trimmed:", trimmed);
```

E2E テストは TypeScript と Rust の stdout を比較する方式（`tests/e2e_test.rs` の `run_e2e_test`）。Rust 側で `String::split` 等が正しく動作する必要がある。変換結果が `.split(" ")` → `.split(" ").collect::<Vec<_>>()` 等になるため、実行可能性は変換の正確さに依存する。

E2E テスト（実行比較）が通らない場合、原因を特定する:
- **I-211 の変更に起因する問題** → この PRD 内で修正する
- **I-211 で顕在化した既存バグで局所的に修正可能** → この PRD 内で修正する
- **I-211 と無関係の既存バグ、または広範囲の修正が必要** → TODO に記録し、スナップショットテストで変換結果の型追跡効果を確認する

#### 2. Array メソッドチェーンのスナップショットテスト

`tests/fixtures/` に新規 fixture を追加:

```typescript
// array-builtin-methods.input.ts
function processNumbers(nums: number[]): number[] {
    return nums.map(x => x * 2).filter(x => x > 4);
}

// 型注釈なしの変数 — TypeRegistry からの型追跡を検証
function getFirstPositive(nums: number[]): number | undefined {
    const doubled = nums.map(x => x * 2);
    return doubled.find(x => x > 0);
}
```

スナップショットで変換結果を検証。`map` の戻り値型が `Vec<f64>` と推論され、`filter`/`find` のレシーバ型が正しく解決されることを確認。

#### 3. instanceof ビルトイン型のスナップショットテスト

`tests/fixtures/` に新規 fixture を追加:

```typescript
// instanceof-builtin.input.ts
function handleValue(x: any): string {
    if (x instanceof Date) {
        return x.toISOString();
    }
    if (x instanceof Error) {
        return x.message;
    }
    if (x instanceof RegExp) {
        return x.source;
    }
    return String(x);
}
```

スナップショットで変換結果を検証。`Date`, `Error`, `RegExp` が TypeRegistry に存在するため、any_narrowing の enum バリアントが有効な型参照になることを確認。

#### 4. ベンチマーク実行 + 効果測定

```bash
./scripts/hono-bench.sh
```

実行後:
- `bench-history.jsonl` に結果が追記される
- `/tmp/hono-bench-errors.json` のエラーカテゴリ別集計を確認
- I-211 以前の結果と比較し、エラー数の変化を記録
- TODO の I-112c の記述を更新（インスタンス数の変化、解消された/残存するカテゴリ）

### 設計整合性レビュー

- **高次の整合性**: テスト・検証のみの PRD であり、既存のテストフレームワーク（E2E: stdout 比較、integration: insta スナップショット）に沿った追加。新しいテストパターンの導入なし
- **DRY / 直交性**: 確認済み、問題なし
- **結合度**: 確認済み、問題なし。テストは独立して追加・実行可能
- **割れ窓**: テスト中に発見したバグは種類に応じて対応（スコープのバグ対応方針を参照）

### 影響範囲

| モジュール | 変更内容 |
|-----------|---------|
| `tests/e2e/scripts/method_chain.ts` | 型注釈なしのメソッドチェーンケース追加 |
| `tests/fixtures/` | `array-builtin-methods.input.ts`, `instanceof-builtin.input.ts` 新規追加 |
| `tests/integration_test.rs` | 新規 fixture のテスト関数追加 |
| `tests/snapshots/` | 新規スナップショットファイル |
| `TODO` | ベンチマーク結果の反映、I-112c のインスタンス数更新 |

## タスク一覧

### T1: String メソッドチェーンの検証

- **作業内容**:
  - `tests/e2e/scripts/method_chain.ts` に型注釈なしのテストケースを追加（`const parts = "hello world".split(" ")` 等）
  - E2E テスト実行: `cargo test -- method_chain`
  - E2E テストが通らない場合: 原因を特定し、バグ対応方針に従って修正または TODO 記録。I-211 起因の問題は修正する
- **完了条件**:
  - E2E テストまたはスナップショットテストが通る
  - 型注釈なしの変数で `String.split` の戻り値型が `Vec<String>` に推論されていることがスナップショットで確認できる
- **依存**: I-211-b 完了

### T2: Array メソッドチェーン + instanceof のスナップショットテスト

- **作業内容**:
  - `tests/fixtures/array-builtin-methods.input.ts` を作成（`map`, `filter`, `find` チェーン）
  - `tests/fixtures/instanceof-builtin.input.ts` を作成（`Date`, `Error`, `RegExp` の instanceof）
  - `tests/integration_test.rs` に対応するテスト関数を追加
  - `cargo test` でスナップショット生成 → `cargo insta review` で確認
- **完了条件**:
  - スナップショットが生成され、レビュー後に accept される
  - `Array.map` の戻り値型が `Vec<_>` に推論されている
  - `instanceof Date` で生成される enum バリアントが `Date(Date)` であり、`Date` struct が存在する型として参照されている
  - `cargo test` が通る
- **依存**: T1

### T3: ベンチマーク実行 + 効果測定

- **作業内容**:
  - `./scripts/hono-bench.sh` を実行
  - `/tmp/hono-bench-errors.json` を `scripts/analyze-bench.py` で集計
  - I-211 以前のベンチマーク結果（`bench-history.jsonl` の直前エントリ）と比較
  - エラー数の変化を TODO に反映:
    - I-112c のインスタンス数を更新
    - I-211 の項目を完了済みに更新
    - 新たに発見された問題があれば TODO に追加
- **完了条件**:
  - ベンチマーク結果が `bench-history.jsonl` に記録される
  - エラーインスタンス数が I-211 以前と比較して増加していない
  - TODO が最新のベンチマーク結果を反映している
- **依存**: T2

## テスト計画

### E2E テスト

- String メソッドチェーン（型注釈なし）: `"hello world".split(" ")` → TypeScript と Rust の stdout が一致

### スナップショットテスト

- Array メソッドチェーン: `nums.map(x => x * 2).filter(x => x > 4)` の変換結果
- instanceof ビルトイン型: `x instanceof Date` → `if let` パターンの変換結果

### ベンチマーク

- Hono ベンチマークでエラー数が増加しないこと

## 完了条件

1. `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
2. `cargo fmt --all --check` が通る
3. `cargo test` が全テスト通過
4. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` が通る
5. String メソッドチェーンの E2E テストまたはスナップショットテストが通る
6. Array メソッドチェーンのスナップショットテストが通る
7. instanceof ビルトイン型のスナップショットテストが通る
8. ベンチマーク結果が `bench-history.jsonl` に記録されている
9. TODO のエラーインスタンス数が最新のベンチマーク結果を反映している
