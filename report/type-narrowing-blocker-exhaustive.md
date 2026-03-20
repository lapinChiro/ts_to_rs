# 型 Narrowing 完璧な実装のブロッカー全件調査

**基準コミット**: `7756458`（未コミット変更あり: I-205/I-206 実装済み）

## 要約

完璧な型 narrowing を阻むブロッカーは **9 件 + 未実装 2 件** = 計 11 件。

## ブロッカー一覧

### 楽観的 true 関連（5 件）

| # | 問題 | トリガー条件 | 現在の出力 | 正しい出力 |
|---|------|-------------|-----------|-----------|
| 1 | arrow 関数の any パラメータ | `const f = (x: any) => { typeof x === "string" }` | `if true` | enum + if let |
| 2 | ローカル変数の any | `let x: any; typeof x === "string"` | `if true` | enum + if let |
| 3 | TypeEnv 未登録の変数 | `typeof unknownVar === "string"` | `if true` | enum + if let |
| 4 | typeof !== の Any/不明型 | `typeof x !== "string"` で x が Any | `if false` | 分岐を保持 |
| 5 | instanceof の Any/不明型 | `x instanceof Foo` で x が Any | `if true` | enum + if let |

### 構文カバレッジ（4 件）

| # | 問題 | トリガー条件 | 現在の出力 | 正しい出力 |
|---|------|-------------|-----------|-----------|
| 6 | 三項演算子の typeof | `typeof x === "string" ? x.length : 0` | 条件を `true` に | match 式 |
| 7 | 複合条件 (&&/\|\|) | `typeof x === "string" && typeof y === "number"` | 各パーツが `true` に | ネストした if let |
| 8 | switch (typeof x) | `switch (typeof x) { case "string": ... }` | typeof マッチなし | match 式の各アームで narrowing |
| 9 | typeof "object"/"function" | `typeof x === "object"` | if let 未生成 | バリアント解決 + if let |

### 未実装ガード（2 件）

| # | 問題 | トリガー条件 | 現在の出力 | 正しい出力 |
|---|------|-------------|-----------|-----------|
| 10 | instanceof ガード | `if (x instanceof Foo)` | `if true` | if let Enum::Foo(x) |
| 11 | truthy ガード | `if (x)` で `x: Option<T>` | narrowing なし | if let Some(x) |

## 依存関係グラフ

```
#1,#2,#3 (any enum 登録カバレッジ拡大)
  └── #4 (楽観的 true !== の解消)
      └── 楽観的 true の完全除去

#9 (typeof "object"/"function")
  └── enum バリアント生成の拡張

#10 (instanceof ガード)
  └── NarrowingGuard に InstanceOf バリアント追加
      └── any_narrowing に instanceof 制約収集追加

#11 (truthy ガード)
  └── NarrowingGuard に Truthy バリアント追加

#6 (三項演算子)
  └── convert_cond_expr に narrowing スコープ追加

#7 (複合条件)
  └── extract_narrowing_guard の再帰的 && 処理

#8 (switch typeof)
  └── convert_switch_stmt に typeof narrowing 追加
```

## 修正の優先順位

**Phase A: 楽観的 true の完全除去**（最優先）
1. #1/#2/#3: any enum 登録のカバレッジ拡大（arrow 関数 + ローカル変数）
2. #4: typeof !== の Any 型ハンドリング修正
3. #5/#10: instanceof ガード追加

**Phase B: 構文カバレッジの拡大**
4. #9: typeof "object"/"function" バリアント解決
5. #11: truthy ガード
6. #7: 複合条件の再帰的ガード抽出
7. #6: 三項演算子の narrowing
8. #8: switch typeof の narrowing

## Hono への影響

Hono ソースコード内のランタイム typeof/instanceof チェックは 0 件（型テスト除く）。つまり Hono のコンパイルクリーン率に直接影響しない。ただし、Hono のミドルウェア型定義で `any` が 30 件使用されており、アプリケーションコードが typeof チェックを行う場合に影響する。
