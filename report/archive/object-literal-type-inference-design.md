# オブジェクトリテラル型推論の設計調査レポート

**基準コミット**: 12c69c0（未コミットの変更あり: I-108/109/110/114 修正済み）
**調査日**: 2026-03-18

## 1. 問題の概要

Hono 全151ファイルの変換で、28ファイルが `"object literal requires a type annotation to determine struct name"` エラーで失敗。最大のボトルネック。

## 2. 現在の実装

`convert_object_lit` (expressions/mod.rs) はオブジェクトリテラルの変換時に `expected: Option<&RustType>` パラメータから struct 名を決定する:

```rust
let struct_name = match expected {
    Some(RustType::Named { name, .. }) => name.as_str(),
    _ => return Err(anyhow!("object literal requires a type annotation to determine struct name"))
};
```

`expected` が `None` または `RustType::Named` 以外の場合、即座にエラーになる。

## 3. 28ファイルの失敗パターン分類

### パターン A: 変数の型注釈付きArrow関数（16件）— **最大グループ**

```typescript
export const getConnInfo: GetConnInfo = (c) => ({
    remote: { address: c.req.header('cf-connecting-ip') }
})
```

該当ファイル: adapter/*/conninfo.ts (8件), utils/basic-auth.ts, middleware/cors/index.ts, middleware/timing/timing.ts, validator/validator.ts, helper/dev/index.ts 等

**根本原因**: `convert_var_decl_arrow_fns` (mod.rs:553) が変数の型注釈（`: GetConnInfo`）を**全く読み取っていない**。変数名だけを抽出し、型注釈は無視される。そのため Arrow 関数の戻り値型が不明となり、`return { ... }` でオブジェクトリテラルに expected type が伝播しない。

**重要**: 型情報は AST に存在する。`Pat::Ident` の `type_ann` フィールドにアクセスすれば取得可能。既に `convert_var_decl` (statements/mod.rs:132) では同じパターンで型注釈を抽出している。

### パターン B: 未登録のコンストラクタ/関数の引数（6件）

```typescript
new Response(msg, { status: 413 })
new SmartRouter({ routers: [...] })
```

該当: http-exception.ts, hono.ts, preset/quick.ts, middleware/body-limit/index.ts, utils/stream.ts, helper/testing/index.ts

**根本原因**: `Response`, `SmartRouter` 等が TypeRegistry に未登録のため、コンストラクタ引数の型情報がない。

### パターン C: メソッド/関数呼び出しの引数（8件）

```typescript
crypto.subtle.digest({ name: algorithm.name }, sourceBuffer)
serialize('name', value, { path: '/', ...opt })
```

該当: utils/crypto.ts, helper/cookie/index.ts, helper/ssg/ssg.ts, adapter/bun/websocket.ts, middleware/jwk/jwk.ts 等

**根本原因**: メソッドの引数型が TypeRegistry に登録されていない。

### パターン D: 型注釈なし変数の初期化（2件）

```typescript
let value = {}
```

該当: validator/validator.ts

**根本原因**: 変数に型注釈がなく、空オブジェクトの型が不明。

## 4. 設計案の比較

### 設計案 1: 変数型注釈からの戻り値型伝播（パターン A 対応）

**概要**: `convert_var_decl_arrow_fns` で変数の型注釈を読み取り、関数型の戻り値型を Arrow 関数の body に伝播する。

**対象**: パターン A（16件）

**変更箇所**:
- `src/transformer/mod.rs` の `convert_var_decl_arrow_fns`

**具体的な実装**:
1. `declarator.name` の `type_ann` から型注釈を取得
2. 型注釈が `RustType::Fn { return_type, .. }` なら、その `return_type` を Arrow の body に伝播
3. 型注釈が `RustType::Named { name }` なら、TypeRegistry で `TypeDef::Function { return_type }` を検索して戻り値型を取得

**メリット**:
- 16件を一度に解消。最大のインパクト
- 既存の型情報を正しく利用するだけ。新しい推論ロジックは不要
- `convert_var_decl` で同じパターンが実績あり
- DRY: 既に存在すべきだった処理の追加

**デメリット**:
- パターン B/C/D は解消しない
- 型エイリアスの解決が必要（`GetConnInfo` → `TypeDef::Function`）

**難易度**: 低〜中

### 設計案 2: コンテキストベースの匿名構造体生成（パターン B/C/D 対応）

**概要**: expected type がない場合、オブジェクトリテラルのフィールドから匿名構造体を自動生成する。

**対象**: パターン B/C/D（14件）

**具体的な実装**:
1. `convert_object_lit` で `expected` が `None` の場合、フィールド名と値の型からコンテキスト名を生成
2. コンテキスト名の生成ルール:
   - 関数引数: `{関数名}_{パラメータ位置}` (例: `ResponseInit`)
   - return 文: `{関数名}Result`
   - 変数宣言: `{変数名}` を PascalCase 化
3. 生成した構造体定義を `extra_items` として返す

**メリット**:
- expected type がない全ケースに対応
- 型安全性を維持（`HashMap` 等への退避より良い）

**デメリット**:
- 大量の使い捨て構造体が生成される可能性
- コンテキスト名の生成ルールが複雑
- 既に TypeRegistry に同等の型がある場合、重複定義になる
- フィールドの型推論が必要（nested object はさらに再帰的に匿名構造体）
- **サイレントに意味が変わるリスク**: 同じフィールド名で異なる型の構造体が生成される可能性

**難易度**: 高

### 設計案 3: 設計案1 + 限定的な匿名構造体（段階的アプローチ）

**概要**: まず設計案1で16件を解消し、残りの14件は匿名構造体で対応するが、安全なケースに限定する。

**対象**: 全28件

**第1段階（設計案1）**:
- `convert_var_decl_arrow_fns` の型注釈伝播を修正
- 16件解消

**第2段階（限定的な匿名構造体）**:
- `return { ... }` で戻り値型がない場合のみ、関数名ベースで構造体名を生成
- 関数引数の場合は引き続きエラー（型情報なしで構造体を作るリスクが高い）

**メリット**:
- 第1段階だけでも16件（57%）解消
- 段階的に進められ、各段階で検証可能
- リスクの高い匿名構造体は限定的に使用

**デメリット**:
- 第2段階の設計が不確定
- パターン B/C は未解決

**難易度**: 第1段階=低、第2段階=中〜高

### 設計案 4: TypeEnv の拡張（変数型 → Arrow 戻り値型の解決）

**概要**: TypeEnv に「変数の型が関数型の場合、戻り値型を記録する」機能を追加。

**対象**: パターン A（16件）

**変更箇所**:
- TypeEnv に関数型情報を格納する機能追加
- Arrow 関数の変換時に TypeEnv から戻り値型を参照

**メリット**:
- ネストした Arrow 関数にも対応可能
- 将来的にスコープベースの型推論基盤になる

**デメリット**:
- 設計案1と本質的に同じ問題を解決するが、実装が間接的
- TypeEnv の責務が肥大化する
- `convert_var_decl_arrow_fns` は TypeEnv を直接使っていない

**難易度**: 中

## 5. 設計案の比較表

| 評価軸 | 案1: 型注釈伝播 | 案2: 匿名構造体 | 案3: 段階的 | 案4: TypeEnv拡張 |
|--------|----------------|----------------|-----------|----------------|
| 解消ファイル数 | 16/28 (57%) | 14/28 (50%) | 28/28 (100%) | 16/28 (57%) |
| 実装コスト | 低 | 高 | 中（段階的） | 中 |
| サイレントリスク | なし | あり | 低（限定的） | なし |
| KISS | ◎ | × | ○ | ○ |
| YAGNI | ◎ | × | ○ | △ |
| 既存コードとの整合性 | ◎ | △ | ○ | ○ |
| 将来の拡張性 | ○ | ○ | ◎ | ◎ |

## 6. 推奨設計

**設計案1（変数型注釈からの戻り値型伝播）を推奨する。**

理由:
1. **最大の投資対効果**: 16件（全体の57%）を低コストで解消。最小の変更で最大のインパクト
2. **バグ修正の性質**: 型注釈が AST に存在するのに無視されている。これは「未対応」ではなく「バグ」
3. **KISS**: 新しい概念（匿名構造体）を導入せず、既存パターンの横展開
4. **サイレントリスク 0**: 既存の型情報を正しく伝播するだけで、新しい推論を行わない
5. **残り14件は別PRDで対応**: パターン B/C/D は別の根本原因（TypeRegistry の不足）であり、混ぜるべきではない

パターン B/C/D（未登録型の引数）は、I-24（外部パッケージの型定義解決）が前提となる可能性が高く、今回のスコープに含めるべきではない。
