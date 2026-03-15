# ブラックボックステスト: TS → Rust の実行結果一致検証

## 背景・動機

現在のテストは 3 層で構成されている:

1. **ユニットテスト**: 個々の変換関数の入出力を検証
2. **スナップショットテスト**: 生成コードの文字列表現を検証
3. **コンパイルテスト**: 生成 Rust がコンパイルを通ることを検証

しかし、**「変換結果が元の TypeScript と同じ振る舞いをするか」は一切検証されていない**。これは変換ツールとして最も重要な品質保証が欠落していることを意味する。

「コンパイルは通るが結果が異なる」サイレント不具合は、検出が遅れるほど修正コストが跳ね上がる。変換ロジックの追加・変更のたびにリスクが増大しており、早期にこの検証基盤を確立する必要がある。

### なぜこのタスクが困難か

このタスクは他の開発タスクとは根本的に異なる。

**通常の開発タスク**: 「この構文を変換する」→ 入力と出力が明確 → テストが書きやすい

**このタスク**: 「変換結果が正しいことを証明する」→ **何をもって正しいとするかの定義自体が設計対象**

具体的な困難:

1. **テストケースの設計が本質**: テストインフラ（TS 実行 → Rust 実行 → 比較）は機械的に作れる。しかし「何をテストするか」の設計は、TS と Rust の全てのセマンティクスの差異を理解した上で、意味のある検証シナリオを構築する必要がある

2. **カバレッジの定義が曖昧**: ユニットテストなら「この関数の全分岐」と定義できる。ブラックボックステストでは「現実的な TS プログラムの振る舞い空間」をカバーする必要があり、その空間は事実上無限

3. **TS と Rust のセマンティクスの差異**: 浮動小数点の精度、文字列のエンコーディング、エラーハンドリングの構造（throw vs Result）、null/undefined vs Option — これらの差異を「許容する差異」と「バグ」に分類する判断が必要

4. **テストスクリプトの保守**: テストスクリプトは ts_to_rs が対応している構文の範囲内で書く必要がある。新機能追加のたびにテストスクリプトも拡充する必要があり、テスト自体が生きたドキュメントとなる

## ゴール

- `tests/e2e/` に「引数 → stdout」形式のテストスクリプト群を配置
- テストランナーが各スクリプトに対して: TS を変換 → Rust をビルド・実行 → TS を実行 → stdout を比較
- 以下のカテゴリのスクリプトを各 1 つ以上含む:
  - 数値計算（四則演算、Math 関数、型変換）
  - 文字列操作（結合、スライス、メソッドチェーン）
  - 配列操作（map/filter/reduce、スプレッド、ソート）
  - 条件分岐（if/else, switch, 三項演算子）
  - ループ（for, while, for-of, break/continue）
  - 関数呼び出し（再帰、クロージャ、デフォルト引数）
  - エラーハンドリング（try/catch/finally）
  - クラス/struct 操作（コンストラクタ、メソッド、継承）
- 全スクリプトで TS と Rust の stdout が完全一致
- テストが `cargo test` で実行可能
- 全テスト pass、clippy 0 警告、fmt 通過

## スコープ

### 対象

- テストインフラの構築:
  - `tests/e2e/` ディレクトリ構造
  - Rust テストランナー（TS 変換 → Rust ビルド・実行 → TS 実行 → stdout 比較）
  - e2e 用の固定 Cargo プロジェクト（`tests/e2e/rust-runner/`）
- テストスクリプトの設計・作成（上記カテゴリ × 各 1 つ以上）
- 入力: コマンドライン引数 or ハードコード定数
- 出力: stdout（`println!` / `console.log`）

### 対象外（TODO に記録、段階的に対応）

- 標準入力からの読み取り
- ファイル I/O（読み書き）
- HTTP リクエスト/レスポンス
- 非決定的な出力（乱数、タイムスタンプ等）
- 非同期処理の実行順序検証

## 設計

### テストケース設計方針

各テストスクリプトは以下の原則に従う:

1. **自己完結**: 外部依存なし。標準ライブラリのみ使用
2. **決定的**: 同じ入力に対して常に同じ出力
3. **Observable**: 計算結果を `console.log()` で出力。出力がなければ検証できない
4. **1 スクリプト 1 関心事**: 各スクリプトは 1 つのカテゴリに焦点。複数カテゴリを混ぜない
5. **失敗時の診断容易性**: 出力の各行にラベルを付ける（例: `"sum: 15"` ではなく `15` だけにしない）

### TS/Rust 間の既知のセマンティクス差異への対処

| 差異 | TS の挙動 | Rust の変換結果 | テストでの扱い |
|------|----------|----------------|---------------|
| 浮動小数点表示 | `console.log(1)` → `1` | `println!("{}", 1.0)` → `1` or `1.0` | 数値出力は `{:.1}` 等でフォーマット統一 |
| null/undefined | `null`, `undefined` | `None` | テストスクリプトで null を使わない、または出力時に統一的な表現にする |
| エラーメッセージ | `Error("msg")` | `Err("msg".to_string())` | メッセージ文字列の一致を検証 |
| 配列の toString | `[1,2,3].toString()` → `"1,2,3"` | 該当なし | 明示的にフォーマットする |

### テストインフラ構成

```
tests/
├── e2e/
│   ├── scripts/          # TS テストスクリプト
│   │   ├── arithmetic.ts
│   │   ├── string_ops.ts
│   │   ├── array_ops.ts
│   │   ├── control_flow.ts
│   │   ├── loops.ts
│   │   ├── functions.ts
│   │   ├── error_handling.ts
│   │   └── classes.ts
│   └── rust-runner/      # Rust 実行用 Cargo プロジェクト
│       ├── Cargo.toml
│       └── src/
│           └── main.rs   # テスト時に上書き
├── e2e_test.rs           # テストランナー
```

### テストランナーの処理フロー

```
for each script in tests/e2e/scripts/*.ts:
  1. ts_to_rs で TS → Rust に変換
  2. 変換失敗 → テスト失敗（変換エラーを報告）
  3. 生成 Rust を rust-runner/src/main.rs に書き込み
  4. `cargo run` で Rust を実行 → stdout をキャプチャ
  5. `npx tsx script.ts` で TS を実行 → stdout をキャプチャ
  6. 両者の stdout を行単位で比較
  7. 不一致 → テスト失敗（差分を報告）
```

### テストスクリプトの制約

テストスクリプトは ts_to_rs が**現在対応している構文**の範囲内で書く必要がある。未対応構文を含むスクリプトは変換段階で失敗するため、テストとして機能しない。

これは制約であると同時に利点でもある: **テストスクリプトの集合が、ts_to_rs の実用的な対応範囲を示す生きたドキュメントになる**。

### 影響範囲

- `tests/e2e/` — 新規ディレクトリ
- `tests/e2e_test.rs` — テストランナー
- `.gitignore` — `tests/e2e/rust-runner/target/` を除外
- `Cargo.toml` — `[workspace] exclude` に `tests/e2e/rust-runner` を追加

## 作業ステップ

### Part A: テストインフラ

- [ ] ステップ1: `tests/e2e/rust-runner/` Cargo プロジェクト作成
- [ ] ステップ2: テストランナー `tests/e2e_test.rs` の実装（1 スクリプトで動作確認）
- [ ] ステップ3: 最小のテストスクリプト（`console.log("hello")` のみ）で E2E パイプライン全体を検証

### Part B: テストスクリプト作成

各スクリプトは TDD で進める: まず TS スクリプトを書き、TS で期待出力を確認してから、変換・比較テストを実行する。

- [ ] ステップ4: 数値計算（四則演算、Math.floor/ceil/abs、型変換）
- [ ] ステップ5: 文字列操作（結合、toUpperCase/toLowerCase、includes、indexOf、split/join）
- [ ] ステップ6: 配列操作（map/filter/reduce、push、indexOf、スプレッド、ソート）
- [ ] ステップ7: 条件分岐（if/else, switch, 三項演算子）
- [ ] ステップ8: ループ（for, while, for-of, break/continue）
- [ ] ステップ9: 関数（再帰、クロージャ、デフォルト引数、rest パラメータ）
- [ ] ステップ10: エラーハンドリング（try/catch/finally、throw）
- [ ] ステップ11: クラス/struct（コンストラクタ、メソッド、継承、getter/setter）

### Part C: 統合

- [ ] ステップ12: 全スクリプトの E2E テスト pass 確認
- [ ] ステップ13: Quality check

## テスト計画

### テストスクリプト詳細設計

#### arithmetic.ts
```typescript
// 四則演算
console.log("add:", 1 + 2);
console.log("sub:", 10 - 3);
console.log("mul:", 4 * 5);
console.log("div:", 10 / 3);
console.log("mod:", 10 % 3);

// Math 関数
console.log("floor:", Math.floor(3.7));
console.log("ceil:", Math.ceil(3.2));
console.log("abs:", Math.abs(-5));
console.log("max:", Math.max(1, 5, 3));
console.log("min:", Math.min(1, 5, 3));
```

#### string_ops.ts
```typescript
const s: string = "Hello, World!";
console.log("upper:", s.toUpperCase());
console.log("lower:", s.toLowerCase());
console.log("includes:", s.includes("World"));
console.log("starts:", s.startsWith("Hello"));
console.log("trim:", "  spaces  ".trim());
console.log("split:", "a,b,c".split(",").join(" "));
```

#### array_ops.ts
```typescript
const arr: number[] = [1, 2, 3, 4, 5];
console.log("map:", arr.map((x: number): number => x * 2));
console.log("filter:", arr.filter((x: number): boolean => x > 2));
console.log("find:", arr.find((x: number): boolean => x === 3));
console.log("length:", arr.length);
console.log("includes:", arr.includes(3));
```

#### control_flow.ts
```typescript
function classify(x: number): string {
    if (x > 0) {
        return "positive";
    } else if (x < 0) {
        return "negative";
    } else {
        return "zero";
    }
}
console.log("classify 5:", classify(5));
console.log("classify -3:", classify(-3));
console.log("classify 0:", classify(0));
```

#### loops.ts
```typescript
// for loop
let sum: number = 0;
for (let i: number = 0; i < 10; i++) {
    sum = sum + i;
}
console.log("sum 0-9:", sum);

// while loop
let count: number = 0;
let n: number = 1;
while (n < 100) {
    n = n * 2;
    count = count + 1;
}
console.log("doublings to 100:", count);

// for-of with break
const items: number[] = [1, 2, 3, 4, 5];
let found: number = -1;
for (const item of items) {
    if (item === 3) {
        found = item;
        break;
    }
}
console.log("found:", found);
```

（他のスクリプトも同様の粒度で設計。実装時に確定する）

## 完了条件

- テストインフラ（TS 実行 → Rust 変換・ビルド・実行 → stdout 比較）が `cargo test` で動作する
- 8 カテゴリ以上のテストスクリプトが存在し、全て TS と Rust の stdout が完全一致する
- テスト失敗時に差分が明確に報告される（どのスクリプトの何行目が不一致か）
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
