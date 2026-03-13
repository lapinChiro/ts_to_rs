# Hono ソースコード構文分析

調査日: 2026-03-13
対象: honojs/hono (latest main), `src/` 配下の非テスト・非 `.d.ts` ファイル 185 件

## 目的

Hono 変換に着手するために必要な構文対応の最小セットを特定する。

## 分析結果

### 全体の構文使用頻度（非テストファイル 185 件）

| 構文 | 出現数 | 現在の対応状況 |
|------|--------|---------------|
| ジェネリクス `<T>` | 1525 | 対応済み |
| アロー関数 `=>` | 1292 | 対応済み |
| テンプレートリテラル | 643 | 対応済み |
| `export` | 763 | 対応済み |
| `import` | 486 | 対応済み |
| type assertion (`as`) | 490 | **未対応** |
| `any` 型 | 409 | **未対応** |
| intersection 型 (`&`) | 387 | **未対応** |
| type alias | 367 | 部分対応（オブジェクト型のみ） |
| `Promise` | 313 | 対応済み（async/await） |
| spread (`...`) | 250 | **未対応** |
| 三項演算子 | 222 | 対応済み |
| async/await | 172 | 対応済み |
| `typeof` | 165 | **未対応** |
| optional chaining (`?.`) | 162 | **未対応** |
| `unknown` 型 | 161 | **未対応** |
| interface | 123 | 対応済み |
| `throw` | 135 | 対応済み |
| `never` 型 | 92 | **未対応** |
| rest params (`...args`) | 97 | **未対応** |
| `infer` | 83 | **未対応** |
| re-export | 77 | **未対応** |
| nullish coalescing (`??`) | 70 | **未対応** |
| `keyof` | 65 | **未対応** |
| `for...of` | 65 | 対応済み |
| `const [a, b]` 配列分割代入 | 67 | 対応済み |
| `try/catch` | 59 | 対応済み |
| conditional types | 54 | **未対応** |
| class | 46 | 対応済み |
| mapped types | 43 | **未対応** |
| `const { x } = obj` 分割代入 | 30 | 対応済み |
| `readonly` | 30 | **未対応** |
| template literal types | 27 | **未対応** |
| param destructuring (`{x}: T`) | 25 | **未対応** |
| computed property | 23 | **未対応** |
| getter/setter | 19 | **未対応** |
| `for...in` | 18 | **未対応** |
| string literal types | 18 | 対応済み |
| `static` | 18 | **未対応** |
| `Map`/`Set` | 15 | **未対応** |
| `abstract` | 14 | **未対応** |
| `satisfies` | 12 | **未対応** |
| `Proxy` | 8 | **未対応** |
| `implements` | 7 | **未対応** |
| `export default` | 6 | **未対応** |
| `switch/case` | 5 | **未対応** |

### コアファイル（hono-base.ts, context.ts, request.ts, compose.ts, router.ts, http-exception.ts）の構文使用

コアランタイム（6 ファイル、約 2000 行）に絞った分析:

| 構文 | 出現数 |
|------|--------|
| `as` (type assertion) | 36 |
| getter/setter | 22/7 |
| spread `...` | 21 |
| `??` (nullish coalescing) | 13 |
| `?.` (optional chaining) | 5 |
| `Object.*` | 6 |
| `instanceof` | 5 |
| `private`/`protected` | 2 |
| `abstract` | 1 |

## 構文の分類

### A. 値レベルの構文（変換が必須）

実行時のロジックに直接関与するため、正しい Rust コードを生成するために対応が必須。

| 構文 | 頻度 | 変換先の見通し | 実装コスト |
|------|------|---------------|-----------|
| type assertion (`x as T`) | 490 | `x as T` / そのまま / キャスト | 小 |
| optional chaining (`x?.y`) | 162 | `x.as_ref().map(\|v\| v.y)` 等 | 中 |
| nullish coalescing (`x ?? y`) | 70 | `x.unwrap_or(y)` / `x.unwrap_or_else(\|\| y)` | 小 |
| spread (`...`) | 250 | `extend` / struct update syntax | 中 |
| rest params (`...args`) | 97 | 可変長引数 / スライス | 中 |
| switch/case | 5 | `match` | 小 |
| getter/setter | 19 | メソッド化 | 小 |
| `instanceof` | 63 | trait ベースの判定 | 中 |
| `static` | 18 | `impl` の関連関数 | 小 |
| param destructuring | 25 | 関数先頭で分割 | 小 |

### B. 型レベルの構文（スキップまたは簡略化可能）

TS の高度な型システムを Rust に完全再現する必要はない。多くは「型情報を捨てる」「近似的な型に変換する」で対処可能。

| 構文 | 頻度 | 戦略 |
|------|------|------|
| `any` 型 | 409 | `Box<dyn std::any::Any>` or ジェネリクス `T` |
| `unknown` 型 | 161 | `any` と同じ扱い |
| `never` 型 | 92 | `!` (never 型) / `unreachable!()` |
| intersection (`&`) | 387 | フィールド統合 or 複数 trait bound |
| conditional types | 54 | 型パラメータに展開 or 具体型に |
| mapped types | 43 | 具体型に展開 |
| `keyof` | 65 | 具体型に展開 |
| `infer` | 83 | 型パラメータに |
| `readonly` | 30 | 無視（Rust はデフォルト不変） |
| template literal types | 27 | `String` に |
| `satisfies` | 12 | 無視（型チェックのみ） |
| `typeof` (型位置) | 165 | 具体型に展開 |

## 推奨: Hono 対応の前提条件（最小構文セット）

**全構文を揃える必要はない。** 以下の基準で最小セットを決定する:

- コアランタイム（6ファイル）の変換に必須な構文を優先
- 型レベルの複雑な構文は「ベストエフォートで近似」の方針で十分
- 完全な変換ではなく「手動修正の起点となるコードを生成する」のが現実的なゴール

### 必須（これがないとコアファイルが変換できない）

1. **type assertion (`x as T`)** — 490 回。コアで 36 回
2. **optional chaining (`x?.y`)** — 162 回。コアで 5 回
3. **nullish coalescing (`x ?? y`)** — 70 回。コアで 13 回
4. **spread 構文** — 250 回。コアで 21 回（関数引数 + オブジェクト）
5. **`any` / `unknown` 型** — 570 回。型レベルだが出現頻度が高すぎて無視できない
6. **getter/setter** — コアで 29 回。Context クラスの API 定義に不可欠

### 推奨（あると変換品質が大幅に向上）

7. **rest params (`...args`)** — 97 回
8. **switch/case → match** — 5 回だが変換は単純
9. **param destructuring** — 25 回
10. **`static` メンバー** — 18 回
11. **`never` 型** — 92 回

### 後回し可（Hono 対応開始後に随時追加）

- intersection 型、conditional types、mapped types、`infer`、`keyof` — 型レベルの高度な構文。初回は `any` か具体型に fallback
- `Proxy`、`Symbol` — Hono のごく一部でのみ使用
- regex — ユーティリティで使用。Rust の `regex` crate に手動マッピング
