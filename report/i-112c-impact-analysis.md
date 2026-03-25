# I-112c 影響範囲調査と開発計画

**基準コミット**: `20a8a54`（未コミット変更: `.serena/project.yml` のみ、調査に影響なし）

---

## 1. エグゼクティブサマリー

I-112c（型注釈なしオブジェクトリテラル）は **70 インスタンス（全 132 エラーの 53.0%）** を占める最大のエラーカテゴリである。本調査の結論:

- **I-112c は他のイシューへの前提依存なし**で着手可能（I-226, I-218 は完了済み）
- **I-211（ECMAScript 標準型追加）は I-112c の効果を最大化するが、ハードブロッカーではない**
- I-112c を先に実装し、その後 I-211 で残りのケースをカバーする順序が最も効率的
- 70 インスタンスのうち **推定 50–55 件は I-112c 単体で解消可能**、残りは I-211 等が必要

---

## 2. I-112c の技術的分析

### 2.1 エラー発生箇所

**エラー生成**: `src/transformer/expressions/data_literals.rs:156-158`

```rust
_ => {
    return Err(anyhow!(
        "object literal requires a type annotation to determine struct name"
    ))
}
```

**呼び出しチェーン**:
1. `convert_expr_with_expected()` (`src/transformer/expressions/mod.rs:42-51`) が `FileTypeResolution.expected_types` から expected type を取得
2. `convert_object_lit(obj_lit, expected)` (`mod.rs:108`) に `Option<&RustType>` を渡す
3. `expected` が `None` または `RustType::Named` 以外の場合にエラー

### 2.2 TypeResolver の expected type 設定箇所（現状）

| コンテキスト | 箇所 | 設定条件 |
|---|---|---|
| 型注釈付き変数宣言 | `type_resolver.rs:258-261` | `const x: Point = {...}` |
| クラスプロパティの型注釈 | `type_resolver.rs:401` | `name: string = {...}` |
| return 文 | `type_resolver.rs:436-438` | `current_fn_return_type` が存在 |
| 関数引数 | `type_resolver.rs:1444-1449` | TypeRegistry に関数の型定義が存在 |
| コンストラクタ引数 | `type_resolver.rs:1523-1528` | TypeRegistry に struct 定義が存在 |
| ネストされたオブジェクトフィールド | `type_resolver.rs:759-774` | 親オブジェクトの expected type から TypeRegistry でフィールド型を解決 |
| 配列要素 | `type_resolver.rs:788-794` | 親配列の expected type が `Vec<T>` |
| 三項演算子の分岐 | `type_resolver.rs:814-823` | 三項演算子の expected type が存在 |

### 2.3 expected type が設定されない原因分類

以下の場合に `expected_type()` が `None` になり、エラーとなる:

1. **関数の戻り値型が未登録**: return 文のオブジェクトリテラルで `current_fn_return_type` が `None`
   - 原因: 関数の戻り値型注釈がない、またはジェネリクス・ユーティリティ型で解決できない
2. **関数パラメータ型が未登録**: 引数のオブジェクトリテラルで `set_call_arg_expected_types` が型を取得できない
   - 原因: 呼び出し先の関数が TypeRegistry に未登録
3. **変数に型注釈がなく初期化子がオブジェクトリテラル**: `const obj = { key: value }`
   - 原因: TypeResolver が型注釈なし変数の初期化子に expected type を設定しない
4. **スプレッド構文**: `const opts = { ...defaults, ...options }`
   - 原因: スプレッドのマージ結果の型が推定されない

---

## 3. Hono ベンチマークでの 70 インスタンスの内訳

`/tmp/hono-bench-errors.json` の実測データに基づく。50 ファイルに分散。

### 3.1 代表的なパターン分析

**パターン A: return 文（関数戻り値型から推定可能）** — 推定 20–25 件

```typescript
// middleware/cors/index.ts:64 — CORSOptions 型が変数注釈にある
const defaults: CORSOptions = {
  origin: '*',
  allowMethods: ['GET', 'HEAD', 'PUT', 'POST', 'DELETE', 'PATCH'],
}
// ↑ これは型注釈があるので本来エラーにならないはず
// ↓ こちらがエラーの本体
const opts = { ...defaults, ...options }  // 型注釈なしスプレッド
```

```typescript
// helper/cookie/index.ts:43 — return {} で型注釈なし
if (!cookie) {
  return {}  // ← GetCookie 型の戻り値だが、current_fn_return_type が設定されていない
}
```

**パターン B: 関数引数（パラメータ型から推定可能）** — 推定 10–15 件

```typescript
// utils/jwt/jwt.ts:65
encodedHeader = encodeJwtPart({ alg, typ: 'JWT', kid: privateKey.kid })
// encodeJwtPart の引数型が TypeRegistry にあれば推定可能
```

**パターン C: 変数初期化（型推定が必要）** — 推定 15–20 件

```typescript
// adapter/cloudflare-pages/handler.ts:32
export const handle = <E extends Env>(app: Hono<E>): PagesFunction<E> => {
  // ← 関数内でオブジェクトリテラルが使われる
}
```

**パターン D: スプレッド構文（マージ型推定が必要）** — 推定 5–10 件

```typescript
const opts = { ...defaults, ...options }
```

### 3.2 ファイル分布（上位 10）

| ファイル | 件数 |
|---|---|
| `helper/cookie/index.ts` | 5 |
| `adapter/aws-lambda/handler.ts` | 4 |
| `adapter/lambda-edge/handler.ts` | 4 |
| `utils/jwt/jwt.ts` | 3 |
| `adapter/cloudflare-pages/handler.ts` | 2 |
| `helper/dev/index.ts` | 2 |
| `middleware/etag/index.ts` | 2 |
| `middleware/jsx-renderer/index.ts` | 2 |
| `middleware/jwk/jwk.ts` | 2 |
| `middleware/jwt/jwt.ts` | 2 |
| その他 40 ファイル | 各 1 |

---

## 4. 他イシューとの依存関係分析

### 4.1 依存関係マップ

```
                    ┌─────────────────────────┐
                    │  I-226 TypeEnv 除去 ✅   │
                    │  I-218 ジェネリクス ✅    │
                    └────────┬────────────────┘
                             │ 完了済み前提
                    ┌────────▼────────────────┐
                    │      I-112c             │
                    │  オブジェクトリテラル     │
                    │  型推定（70 件）          │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼──┐   ┌──────▼──────┐  ┌───▼──────────┐
     │  I-211    │   │  I-224      │  │  他イシュー    │
     │ ES標準型  │   │ this 型解決 │  │  （独立）      │
     │ 効果増幅  │   │ 効果増幅    │  │               │
     └───────────┘   └─────────────┘  └───────────────┘
```

### 4.2 各イシューとの関係詳細

#### ハードブロッカー: なし

I-226（TypeEnv 除去）と I-218（ジェネリクス基盤）は**完了済み**。I-112c に着手するための前提条件はすべて満たされている。

#### 効果増幅（I-112c の後に実施すると追加効果がある）

| イシュー | 関係 | 影響度 | 詳細 |
|---|---|---|---|
| **I-211** | 順方向増幅 | 高（+10-15件） | ECMAScript 標準型（String, Array, Date 等）を TypeRegistry に追加すると、ビルトインメソッドの戻り値型・引数型が解決可能に。I-112c で構築した型推定インフラを I-211 のデータで活用できる |
| **I-224** | 順方向増幅 | 中（+5-8件） | `this` 型解決により、クラスメソッド内の `this.field` / `this.method()` 経由の型が解決可能に。メソッド戻り値型の推定精度が向上 |

#### 独立（並行して実施可能）

| イシュー | 関係 | 理由 |
|---|---|---|
| **I-241〜I-248** (サイレント意味変更) | 独立 | 型推定とは無関係。変換ロジックの正確性の問題 |
| **I-215** (typeof narrowing) | 独立 | narrowing イベントの型設定の問題。object literal の expected type とは別の code path |
| **I-213** (!== narrowing) | 独立 | 型 narrowing の分岐ロジック。object literal とは直交 |
| **I-214** (三項 && narrowing) | 独立 | 式変換の narrowing。object literal 推定とは別の concern |
| **I-102** (any/unknown 残課題) | 独立 | any-narrowing enum 生成は独立した分析パス |
| **I-101** (ジェネリック intersection) | 独立 | intersection 型変換の問題。object literal の struct 名推定とは別 |
| **I-219** (conditional type) | 独立 | 型エイリアス変換の問題 |
| **I-221** (intersection member) | 独立 | intersection 変換の内部エラー |
| **I-104** (所有権推論) | 逆方向依存 | I-112c で正しく型付けされた後に所有権判断が可能 |
| **I-195, I-196** (未対応構文) | 独立 | 構文変換の問題 |

#### 逆方向依存（I-112c が先に完了することで恩恵を受ける）

| イシュー | 恩恵 |
|---|---|
| **I-104** (所有権推論) | object literal が正しく struct に変換されると、フィールドの所有権判断が可能に |
| **I-182** (Hono コンパイルテスト) | 変換成功率が上がることで、コンパイルテスト対象ファイルが増加 |

### 4.3 I-211 との関係の深掘り

I-211 は I-112c の**ブロッカーではないが、効果を最大化するパートナー**である。

**I-112c 単体で解決できるケース**:
- ユーザー定義型の関数パラメータ、戻り値型、変数型注釈からの推定
- TypeRegistry に登録済みの型（Hono 独自の型、Web API 型 106 個）からの推定
- 推定 50–55 件

**I-211 が必要なケース**:
- `String.split()` 等のビルトインメソッド戻り値型からの推定
- `Array.map()` のコールバック引数型からの推定
- `Date`, `RegExp` 等のコンストラクタ引数型からの推定
- 推定 10–15 件

**最適な順序**: I-112c → I-211（I-112c で型推定インフラを構築し、I-211 でデータを追加）

---

## 5. I-112c の実装に必要な変更範囲

### 5.1 主要変更ファイル

| ファイル | 変更内容 | 影響度 |
|---|---|---|
| `src/pipeline/type_resolver.rs` | expected type の設定箇所を拡張（TypeResolver の中核変更） | 高 |
| `src/transformer/expressions/data_literals.rs` | フォールバック推定ロジック追加（エラー箇所の改善） | 中 |
| `src/pipeline/type_resolution.rs` | 必要に応じてデータ構造の拡張 | 低 |

### 5.2 必要な推定戦略（優先度順）

**Strategy 1: 関数戻り値型からの推定（return 文）**
- 現状: `current_fn_return_type` が設定されていない関数がある
- 原因: 型エイリアス（`GetCookie` 等）やジェネリクス付き戻り値型の解決が不完全
- 対処: TypeResolver の関数走査時に、型注釈からの戻り値型解決を強化
- 影響: 推定 20–25 件を解消

**Strategy 2: 関数引数型からの推定（call site）**
- 現状: `set_call_arg_expected_types` は TypeRegistry 登録済み関数のみ対応
- 原因: ローカル関数・クロージャ・メソッドのパラメータ型が TypeResolver のスコープで解決できていない
- 対処: スコープ内の `Fn` 型変数からパラメータ型を伝搬
- 影響: 推定 10–15 件を解消

**Strategy 3: 匿名構造体のフォールバック生成**
- 型推定が完全に不可能な場合に、オブジェクトリテラルのフィールドから匿名 struct を自動生成
- `SyntheticTypeRegistry` に登録し、struct 定義を生成
- 影響: 残りの 15–20 件のうち一部を解消

**Strategy 4: スプレッド構文のマージ型推定**
- `{ ...a, ...b }` で `a` と `b` の型が既知の場合、フィールドをマージした型を生成
- TypeRegistry のフィールド情報を使ってマージ
- 影響: 5–10 件を解消

---

## 6. 推奨開発計画

### 6.1 計画概要

I-112c を**段階的に**実装し、各段階でベンチマークで効果を計測する。

### Phase 1: TypeResolver の expected type 設定強化（最大レバレッジ）

**目標**: return 文と関数引数の expected type 設定率を向上
**推定効果**: 30–40 件解消（70→30–40 件に削減）

タスク:
1. `current_fn_return_type` の設定漏れを調査・修正（型エイリアスの解決強化）
2. `set_call_arg_expected_types` のローカル関数・クロージャ対応
3. テスト追加 + ベンチマーク計測

### Phase 2: 匿名構造体のフォールバック生成

**目標**: 型推定不可能なオブジェクトリテラルに対する安全なフォールバック
**推定効果**: 10–15 件解消

タスク:
1. `SyntheticTypeRegistry` にオブジェクトリテラルのフィールドから struct を自動生成するロジックを追加
2. `data_literals.rs` のエラーパスを匿名 struct 生成に置換
3. テスト追加 + ベンチマーク計測

### Phase 3: スプレッド構文のマージ型推定

**目標**: `{ ...a, ...b }` パターンの型推定
**推定効果**: 5–10 件解消

タスク:
1. TypeResolver でスプレッドソースの型をマージする推定ロジック
2. テスト追加 + ベンチマーク計測

### Phase 4（後続）: I-211 + I-224 で残りを解消

I-112c の基盤上に I-211（ECMAScript 標準型追加）と I-224（`this` 型解決）を実装し、残りのケースをカバー。

### 6.2 見積り効果

| フェーズ | 解消見込み | 累積残エラー | 全体エラー |
|---|---|---|---|
| 着手前 | — | 70 件 | 132 件 |
| Phase 1 完了 | 30–40 件 | 30–40 件 | 92–102 件 |
| Phase 2 完了 | 10–15 件 | 15–30 件 | 77–92 件 |
| Phase 3 完了 | 5–10 件 | 10–20 件 | 72–82 件 |
| Phase 4 (I-211 + I-224) | 5–15 件 | 0–10 件 | 62–72 件 |

**フェーズ移行基準「エラーインスタンス < 50 件」の達成**: Phase 2 完了時点で可能性あり。Phase 3 + Phase 4 で確実に達成。

---

## 7. リスクと注意事項

### 7.1 I-112c を先に実装することによる他イシューへの悪影響: なし

- I-112c は TypeResolver の `expected_types` 設定を拡張するもので、既存のコードパスを壊さない（追加のみ）
- 匿名構造体の生成は `SyntheticTypeRegistry` に追加するため、既存の struct 定義と競合しない
- 他のイシュー（I-241〜I-248 等）はすべて独立したコードパスであり、影響を受けない

### 7.2 匿名構造体生成の設計上の注意

- 同一フィールド構成の匿名構造体が複数回生成されると重複定義になる → `SyntheticTypeRegistry` での dedup が必要（I-212 の union enum dedup と同じパターン）
- 匿名構造体名の命名規則を事前に決定する必要がある（例: `AnonStruct_field1_field2`、またはコンテキストベースの名前）
- **正確性の観点**: 匿名構造体は TypeScript の構造的型付けを模倣するものであり、Rust の名前的型付けとのギャップが発生する。フィールドの型が正確でない場合にコンパイルエラーになるが、これはサイレント意味変更よりは安全

### 7.3 Phase 1 の戻り値型推定で注意すべき点

- 型エイリアスの解決は `convert_ts_type` に依存するが、ジェネリクス付き型エイリアス（`Promise<T>` 等）の解決が不完全な可能性がある
- 関数式（arrow function）に型注釈がない場合、代入先の型変数から戻り値型を逆推定する必要がある（例: `const getCookie: GetCookie = (c, key?) => { return {} }`）

---

## 8. 結論

**推奨**: I-112c に直ちに着手する。ハードブロッカーはなく、TODO 内の他イシューとの依存関係もない。段階的に Phase 1 → 2 → 3 と進め、各段階でベンチマーク計測を行う。I-211 と I-224 は I-112c 完了後に「効果増幅」として実施する。

I-112c は全エラーの 53% を占める最大のレバレッジポイントであり、解消すればフェーズ移行基準（エラー < 50 件）の達成に大きく近づく。
