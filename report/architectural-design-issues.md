# アーキテクチャ設計問題の徹底調査

**基準コミット**: `bcfc4a5`（未コミットの変更あり: ベンチマーク改善・調査レポートを含む）
**調査日**: 2026-03-21

## 要約

モジュール解決と同種の「根本的な設計問題」を全パイプライン（IR → Transformer → Generator）で調査した。構文対応が増えるほどコストが膨らむ問題を **4 件の CRITICAL** と **3 件の HIGH** として特定した。

**最も深刻な問題**: 変換パイプラインの情報伝搬に系統的な欠陥がある。TS AST が持つ情報のうち、IR に到達するまでに型コンテキスト、スコープ情報、親ノード情報が失われる。この損失を generator や transformer が個別のヒューリスティクスで補填しており、新しい構文を追加するたびにヒューリスティクスの複雑さが増す。

### 問題一覧（優先順位順）

| # | 問題 | 深刻度 | 影響する TODO | 所在 |
|---|------|--------|-------------|------|
| 1 | expected 型の伝搬欠落 | CRITICAL | I-112c (74件) | transformer/expressions |
| 2 | TypeRegistry と transformer の断絶 | CRITICAL | I-218, I-211 | registry.rs, transformer |
| 3 | モジュール解決の逐次変換（既知） | CRITICAL | I-222, I-18 | transformer/mod.rs |
| 4 | 合成型の分散生成・重複 | CRITICAL | I-212 | types, any_narrowing, statements |
| 5 | 型 narrowing の制約追跡不足 | HIGH | I-215, I-213, I-214 | type_env.rs |
| 6 | IR の型メタデータ不足 | HIGH | I-218, I-104 | ir.rs |
| 7 | generator のセマンティック判断 | HIGH | 全般 | generator |

## 1. CRITICAL: expected 型の伝搬欠落

### 問題

transformer は `ExprContext::expected` で「この式の期待される型」を伝搬するが、**多くのコンテキストで伝搬が途切れる**。

### 影響箇所

- `src/transformer/expressions/mod.rs:69-145` — `convert_expr()` が `ExprContext` を受け取る
- `src/transformer/expressions/calls.rs:142-150` — コールバック引数に `ExprContext::none()` を渡す
- `src/transformer/expressions/data_literals.rs:188-195` — オブジェクトリテラルが `expected` なしでエラー

### 具体例

```typescript
// パターン A: 変数の型注釈からの伝搬
const result: APIGatewayProxyResult = { body: body, statusCode: 200 };
// ↑ 型注釈 APIGatewayProxyResult があるが、convert_object_lit に expected として渡されない
//   → "object literal requires a type annotation" エラー（I-112c の 74 件の原因）

// パターン B: コールバック引数への伝搬
arr.map(item => ({ name: item.name }));
// ↑ map の型シグネチャから callback の戻り値型を推定できるはずだが、
//   convert_expr が ExprContext::none() で呼ばれるため推定不可能
```

### なぜ今後悪化するか

新しいコレクションメソッド（`flatMap`, `reduce`, `sort` 等）や高階関数を追加するたびに、コールバック内の式に型コンテキストが伝搬されないケースが増える。各メソッドに特殊な `expected` 伝搬ロジックを個別に追加する必要がある。

### 根本原因

`convert_expr()` のシグネチャが `expected` の伝搬を**構造的に保証しない**。呼び出し側が `ExprContext` を手動で構築するため、伝搬漏れが発生する。

### モジュール解決との類似性

モジュール解決では「各ファイルを独立に変換」する設計が `../..` のようなエッジケースを生んだ。同様に、「各式を独立に変換」する設計が型コンテキストの伝搬漏れを生んでいる。**情報を局所的にしか持たない設計**が問題の本質。

## 2. CRITICAL: TypeRegistry と transformer の断絶

### 問題

TypeRegistry は静的な型メタデータ（struct フィールド、メソッドシグネチャ等）を保持するが、transformer が必要とする**動的な型推論**（ジェネリクスのインスタンス化、メソッドチェーンの戻り値型解決等）を提供できない。

### 影響箇所

- `src/registry.rs:198` — `TypeRegistry::instantiate()` は登録済みの型のみ対応
- `src/transformer/expressions/type_resolution.rs:100-150` — `resolve_expr_type` がジェネリック型でフォールバック
- `src/transformer/expressions/calls.rs:117-128` — メソッド呼び出しの型解決が `Option<T>` 等のビルトイン型で失敗

### 具体例

```typescript
interface Container<T> {
  get(): T;
}
const c: Container<string> = ...;
const result = c.get();  // T → string の置換が必要だが、registry は静的スナップショット
```

`result` の型を `string` と解決するには、`Container<string>` のインスタンス化時に `T → String` の置換を registry が追跡する必要がある。現在は `get()` の戻り値型が `T`（未解決）のまま返される。

### なぜ今後悪化するか

- ジェネリクス対応（I-218）で registry への依存が増える
- ECMAScript 標準型追加（I-211）で `String`, `Array`, `Promise` 等のビルトイン型のメソッド解決が必要
- 各ビルトイン型にハードコードの特殊ケースを追加することになる

### 根本原因

TypeRegistry は「型定義の辞書」であり、「型推論エンジン」ではない。transformer が型推論を必要とする場面が増えるにつれ、registry の機能を超える処理を transformer 内に分散実装することになる。

## 3. CRITICAL: モジュール解決の逐次変換（既知）

`report/module-resolution-comparison.md` で詳細に分析済み。手法 B（依存グラフ方式）への移行を決定済み。

## 4. CRITICAL: 合成型の分散生成・重複

### 問題

union 型やインラインの型リテラルから生成される合成型（enum, struct）が **3 つの異なるモジュールで独立に生成**され、名前の重複や意味的な不一致が発生する。

### 影響箇所

1. `src/transformer/types/mod.rs:1883-1927` — インライン型リテラル → `_TypeLitN` struct
2. `src/transformer/any_narrowing.rs:84` — any パラメータの typeof → `FooXType` enum
3. `src/transformer/types/mod.rs:274-394` — union 型 → `StringOrF64` enum

### 具体例

```typescript
// 同じ union 型が異なるコンテキストで異なる名前の enum を生成
function foo(x: string | number) { ... }  // → StringOrF64 enum
function bar(y: string | number) { ... }  // → StringOrF64 enum（重複！ I-212）

// 同じ形状の型が異なるエンコーディングで生成される
const a: { x: number } = ...;           // → _TypeLit0 struct
const b: string | { x: number } = ...;  // → enum のバリアントに { x: f64 } がインライン

// static counter によるナンバリングが非決定的
type A = { a: number };  // → _TypeLit0
type B = { b: string };  // → _TypeLit1
// ファイル処理順序が変わると番号が入れ替わる
```

### なぜ今後悪化するか

- ジェネリクスと合成型の組み合わせ: `Container<{ x: number }>` がネストした合成型を生成
- ファイル間で同じ union 型を使うケースが増え、I-212（重複 enum）の発生頻度が増加
- 合成型の名前衝突を避けるために命名ロジックが複雑化

### 根本原因

合成型の生成が分散しており、**中央集権的な型レジストリがない**。「この union 型は既に enum として定義済みか？」を判定する仕組みがない。

### モジュール解決との類似性

モジュール解決の「各ファイルが独立に use を生成」と同構造。合成型も「各変換箇所が独立に型を生成」しており、全体を俯瞰する仕組みがない。

## 5. HIGH: 型 narrowing の制約追跡不足

### 問題

`TypeEnv` は変数名 → 型の平坦なマップだが、narrowing の制約（型ガード、null チェック、truthiness チェック）を追跡できない。

### 影響箇所

- `src/transformer/type_env.rs:54-116` — TypeEnv のスコープスタック
- `src/transformer/expressions/patterns.rs:15-50` — narrowing ガード抽出

### 具体例

```typescript
function handle(data: unknown) {
  if (data !== null && typeof data === "object" && Array.isArray(data)) {
    // data は array に narrowing されるべきだが、TypeEnv は1つの型しか保持できない
    // 3つの条件の AND は追跡不可能
    data.forEach(item => console.log(item));
  }
}
```

### なぜ今後悪化するか

- narrowing Phase B-2〜B-4（I-213, I-214, I-215）が全てこの制約に衝突する
- 排除 narrowing（`!==`）、複合条件（`&&`）、typeof "object"/"function" が組み合わさると指数的に複雑化

## 6. HIGH: IR の型メタデータ不足

### 問題

IR の `Expr` 列挙体に型情報がなく、generator が型に基づく判断を行えない。

### 影響箇所

- `src/ir.rs:479-486` — `Expr::MethodCall` に戻り値型なし
- `src/ir.rs:528-533` — `Expr::FnCall` にジェネリクス型引数なし
- `src/ir.rs:303-312` — `Item::Impl` に `type_params` なし

### 具体例

```rust
// Expr::MethodCall には戻り値型がないため、generator は:
// - turbofish 構文 (::method::<T>()) を生成できない
// - メソッドチェーンの中間型を推定できない
// - 戻り値型に基づく .to_string() 等の自動挿入ができない

// Item::Impl に type_params がないため:
// impl Container → impl<T> Container<T> が生成できない（I-218）
```

### なぜ今後悪化するか

- ジェネリクス対応で turbofish 構文が必須になるケースが増加
- 所有権推論（I-104）で `Expr` の型情報が borrow/clone の判断に必要
- メソッドチェーンの型追跡強化で、各 `MethodCall` の戻り値型が下流の変換に影響

## 7. HIGH: generator のセマンティック判断

### 問題

generator が IR を**解釈して**セマンティックな判断を行っている。これは transformer の責務。

### 影響箇所

- `src/generator/statements.rs:176-190` — match 式の discriminant に `.as_str()` を付加するか判断
- `src/generator/mod.rs:324-429` — enum のカテゴリ分類（data/numeric/string）と impl 生成を判断
- `src/generator/mod.rs:29-34` — 生成済み文字列をスキャンして `use regex::Regex;` を挿入

### 具体例

```rust
// generator/mod.rs:29-34
if output.contains("Regex::new(") {
    format!("use regex::Regex;\n\n{output}")
}
// ↑ 生成済みの文字列を正規表現的にスキャンして import を判断
//   コメントや文字列リテラル内の "Regex::new(" で誤検知する可能性
```

### なぜ今後悪化するか

- serde, tokio, chrono 等の新しいクレート依存が増えるたびにスキャンルールを追加
- 各クレートの import 判定ロジックが generator に蓄積

## ブロッカー間の因果関係

```
問題 1 (expected 型伝搬)
  ← I-112c の根本原因（74件のオブジェクトリテラルエラー）
  ← 問題 2 (TypeRegistry 断絶) が基盤に影響
     ← ジェネリクスの型解決が不完全なため、expected 型の情報源自体が不足

問題 4 (合成型分散)
  ← I-212（重複 enum）の根本原因
  ← 問題 1 の修正（I-112c）で合成 struct の生成が増えるため悪化
  ← 問題 3 (モジュール解決) とは独立だが、同じ「全体俯瞰の欠如」パターン

問題 5 (narrowing 制約)
  ← I-213, I-214, I-215 の根本原因
  ← 問題 6 (IR 型メタデータ) がないため、narrowing 結果を IR に保存できない

問題 7 (generator セマンティクス)
  ← 問題 6 (IR 型メタデータ不足) が根本原因
  ← IR に情報がないため generator が推測で補填している
```

## 推奨アクション（解消順序）

以下の順序は因果関係と影響範囲に基づく。上位の問題を解消すると下位の問題が自然に軽減される。

### 1. 合成型の中央管理化（問題 4）

**理由**: I-212（重複 enum）を直接解消し、今後の union 型・ジェネリクス対応の基盤になる。他の問題の修正に先行して整備すべき。

**方向性**: 合成型の生成を `TypeRegistry` または新しい `SyntheticTypeRegistry` に集約。同一のセマンティックシグネチャ（`string | number`）は1つの enum にまとめる。

### 2. モジュール解決の依存グラフ方式への移行（問題 3）

**理由**: 既に決定済み。I-222 を直接解消。

### 3. expected 型の構造的伝搬（問題 1）

**理由**: I-112c（74件）を直接解消する前提。TypeRegistry の改善（問題 2）と同時に進める必要がある。

**方向性**: `convert_expr` の呼び出しチェーンで expected 型が**常に**伝搬される仕組み。変数宣言の型注釈 → 右辺の expected、関数の戻り値型 → return 文の expected、メソッドのパラメータ型 → 引数の expected。

### 4. IR の型メタデータ拡充（問題 6 + 7）

**理由**: generator のセマンティック判断を transformer に移すための前提。

**方向性**: `Expr` に `Option<RustType>` の型メタデータを追加。`Item::Impl` に `type_params` を追加。import 追跡を IR に含める。

### 5. 型 narrowing の制約ストア（問題 5）

**理由**: narrowing Phase B の前提。問題 6（IR 型メタデータ）が先行すると実装が容易。

**方向性**: `TypeEnv` を制約ストアに拡張。変数ごとに複数の制約（型ガード、null チェック等）を AND/OR で追跡。
