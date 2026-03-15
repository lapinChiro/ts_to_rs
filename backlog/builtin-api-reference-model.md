# builtin API の参照モデル修正

## 背景・動機

`map_method_call` の各 API マッピングが、Rust の型システムを正しく反映していない。具体的には:

1. **イテレータクロージャの参照型**: `iter()` が返す要素は `&T` だが、クロージャパラメータが `T` として型注記される
2. **可変性の不足**: `sort()` / `reverse()` / `drain()` は `&mut self` を要求するが、関数パラメータは immutable で受け取る
3. **返り値型の不一致**: `position()` は `Option<usize>` を返すが、TS の `indexOf` は `number` を返す前提で `f64` の返り値型と不一致
4. **f64 の Ord 未実装**: `sort()` は `Ord` トレイトが必要だが `f64` は `PartialOrd` のみ
5. **join の引数型**: `join(&str)` に `String` を渡している
6. **splice の範囲式優先度**: `1..1.0 + 2.0 as i64` のキャスト優先度が不正

これらにより `builtin-api-batch` テストのコンパイルテストがスキップされている。

## ゴール

- `builtin-api-batch` のスナップショットが正しい Rust コードを生成する
- `builtin-api-batch` のコンパイルテストスキップが解消される（生成コードが `rustc` でコンパイル通る）
- 各 API 修正が個別のテストで検証されている

## スコープ

### 対象

以下の API マッピング修正（`builtin-api-batch.input.ts` の全関数が正しく変換される）:

1. **`reduce` → `fold`**: クロージャ第2引数の型注記を除去（Rust の型推論に任せる）
2. **`indexOf` → `position`**: 返り値を `.unwrap_or(usize::MAX)` でラップし `f64` 互換にする。または返り値型を `Option<usize>` に変更する — TS の `-1` 返却セマンティクスに対応
3. **`join`**: 引数に `&` を付与（`String` → `&str` 変換）
4. **`reverse`**: パラメータを `mut` にする
5. **`sort`（引数なし）**: `sort_by(|a, b| a.partial_cmp(b).unwrap())` に変換 + パラメータを `mut` にする
6. **`sort`（比較関数付き）**: クロージャ引数の型注記を除去 + `partial_cmp` ベースに変換 + パラメータを `mut` にする
7. **`splice` → `drain`**: 範囲式の start/end を整数キャストする IR を生成 + パラメータを `mut` にする

### 対象外

- `map_method_call` に新しい API マッピングを追加すること（`substring`, `charAt` 等）
- `findIndex` の追加（現在のフィクスチャに含まれていない）
- パラメータの `mut` 化の汎用的な仕組みの設計（本 PRD では `reverse`/`sort`/`splice` の個別対応）

## 設計

### 技術的アプローチ

#### 1. クロージャ引数の参照型対応（reduce, sort）

`fold` や `sort_by` のクロージャは `iter()` 経由で `&T` を受け取る。現在 TS のクロージャ引数に型注記があるとそのまま IR に渡されるが、`iter()` メソッドチェーン内では型注記を除去して Rust の型推論に任せるのが最も安全。

`map_method_call` 内で、クロージャ引数の `Expr::Closure` の `params` から型注記 (`ty`) を `None` に書き換える。

#### 2. sort の f64 対応

`f64` は `Ord` を実装していないので `sort()` は使えない。

- 引数なし `sort()` → `sort_by(|a, b| a.partial_cmp(b).unwrap())`
- 比較関数付き `sort((a, b) => b - a)` → `sort_by(|a, b| (b - a).partial_cmp(&0.0).unwrap())`

ただし、比較関数付きの場合、TS の比較関数 `(a, b) => b - a` は数値を返し、Rust の `sort_by` は `Ordering` を返す必要がある。TS の比較関数の返り値（差分）を `partial_cmp(&0.0)` で `Ordering` に変換する。

#### 3. indexOf → position の返り値ラップ

`position()` は `Option<usize>` を返す。TS の `indexOf` は見つからないとき `-1` を返す。

→ `.map(|i| i as f64).unwrap_or(-1.0)` でラップして `f64` を返す。

#### 4. join の引数型変換

`join(sep)` → `join(&sep)` — 引数を借用参照に変換。

`map_method_call` 内で引数を `Expr::MethodCall { method: "as_str", .. }` でラップする、または generator レベルで `&` を付与する。最もシンプルなのは引数の先頭に `&` を付けること。IR には `Expr::Ref` がないので、`Expr::Ident(format!("&{}", ...))` はハック的。

→ 代わりに `join` の引数が `Expr::Ident` なら名前に `&` を前置する。それ以外なら `.as_str()` でラップする。

#### 5. パラメータの mut 化（reverse, sort, splice）

これらのメソッドは `&mut self` を要求する。TS のパラメータは immutable だが、Rust では `mut` が必要。

`map_method_call` は現在 `object: Expr` を受け取るが、呼び出し元のパラメータの mutability を変更する手段がない。

→ transformer レベルで対応: `convert_stmt` で式文（`Stmt::Expr`）の中のメソッド呼び出しが `sort`/`reverse`/`drain` の場合、オブジェクトの変数宣言を `mutable: true` に更新する。ただしこれは `convert_stmt` のスコープを超える。

**代替案**: `convert_var_decl` で `Vec<T>` 型の変数は常に `mutable: true` にする。現在 `is_object_type` が `Vec(_)` を含んでいるが、`const` の場合にのみ適用されている。`let` でも `Vec` なら `mut` にする。

→ 実際には、関数パラメータが問題。`convert_fn_decl` でパラメータを受け取る段階ではまだ body を見ていないので、mutability を判定できない。

**最もシンプルな解**: 生成される関数シグネチャで `arr: Vec<f64>` を `mut arr: Vec<f64>` にする。これは `convert_fn_decl` の後処理で body を走査し、`sort`/`reverse`/`drain` メソッド呼び出しのレシーバー変数を `mut` パラメータに変換する。

### 影響範囲

- `src/transformer/expressions/mod.rs` — `map_method_call` の各分岐修正
- `src/transformer/functions/mod.rs` — パラメータ mut 化の後処理
- `src/transformer/classes.rs` — メソッドパラメータの mut 化（同上）
- `src/generator/expressions.rs` — `generate_range_bound` の優先度修正（必要な場合）
- `tests/compile_test.rs` — スキップリスト更新
- テストファイル全般

## 作業ステップ

### Part A: クロージャ・返り値の修正（型の正確性）

- [ ] ステップ1（RED）: `reduce` → `fold` でクロージャ引数の型注記が除去されるテスト
- [ ] ステップ2（GREEN）: `map_method_call` の `reduce` 分岐でクロージャ引数の型を除去
- [ ] ステップ3（RED）: `indexOf` → `position` が `.map(|i| i as f64).unwrap_or(-1.0)` を返すテスト
- [ ] ステップ4（GREEN）: `map_method_call` の `indexOf` 分岐を修正
- [ ] ステップ5（RED）: `join` が `&sep` を引数に取るテスト
- [ ] ステップ6（GREEN）: `map_method_call` の `join` 分岐で借用参照化

### Part B: sort の f64 対応

- [ ] ステップ7（RED）: `sort()` が `sort_by(|a, b| a.partial_cmp(b).unwrap())` を生成するテスト
- [ ] ステップ8（GREEN）: `map_method_call` の `sort` 分岐を修正
- [ ] ステップ9（RED）: `sort((a, b) => b - a)` が `sort_by` + `partial_cmp` を生成するテスト
- [ ] ステップ10（GREEN）: 比較関数付き sort の変換を実装

### Part C: パラメータ mut 化

- [ ] ステップ11（RED）: `reverse(arr)` で `arr` パラメータが `mut` になるテスト
- [ ] ステップ12（GREEN）: 関数 body 走査による mut パラメータ判定を実装
- [ ] ステップ13: `sort`, `splice`/`drain` にも同じ判定を適用

### Part D: splice 範囲式の修正

- [ ] ステップ14（RED）: `splice(1, 2)` が正しい範囲式（`1..3` or `1usize..3usize`）を生成するテスト
- [ ] ステップ15（GREEN）: splice 変換で整数リテラル化（start, start+count をコンパイル時計算可能な場合は直接整数に）

### Part E: 統合・スキップ解消

- [ ] ステップ16: スナップショット更新
- [ ] ステップ17: `compile_test.rs` から `builtin-api-batch` のスキップを削除
- [ ] ステップ18: Quality check

## テスト計画

### 個別 API テスト（transformer レベル）

- `reduce((acc, x) => acc + x, 0)` → `iter().fold(0.0, |acc, x| acc + x)` （型注記なし）
- `indexOf(target)` → `iter().position(...).map(|i| i as f64).unwrap_or(-1.0)`
- `join(sep)` → `join(&sep)`
- `reverse()` → `reverse()` + パラメータ `mut`
- `sort()` → `sort_by(|a, b| a.partial_cmp(b).unwrap())` + パラメータ `mut`
- `sort((a, b) => b - a)` → `sort_by(|a, b| (b - a).partial_cmp(&0.0).unwrap())` + パラメータ `mut`
- `splice(1, 2)` → `drain(1..3).collect::<Vec<_>>()` + パラメータ `mut`

### 統合テスト（E2E）

- `builtin-api-batch` のスナップショットが更新される
- `builtin-api-batch` がコンパイルテストを通過する（スキップ解消）
- 他のスナップショットテストに回帰がない

## 完了条件

- `builtin-api-batch` のコンパイルテストスキップが解消されている
- 各 API 修正に対応する個別のユニットテストが存在する
- `fold` / `sort_by` のクロージャ引数が型注記なし（Rust 型推論に委ねる）
- `indexOf` → `position` の返り値が `f64` 互換にラップされている
- `sort()` が `f64` の `PartialOrd` に対応している（`partial_cmp` 使用）
- `reverse` / `sort` / `drain` のレシーバーパラメータが `mut` になっている
- `splice` の範囲式が正しく整数で構成されている
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
