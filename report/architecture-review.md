# アーキテクチャ全体見直しレポート

**基準コミット**: `bbe1db6`（未コミットの変更あり: split/substring, trait params, computed keys の実装済み）

---

## 1. ハイレイヤー: モジュール間の役割と協調

### パイプライン構造

```
TS Source → parser → SWC AST → registry (pre-scan) → transformer → IR → generator → Rust Source
```

| モジュール | ファイル | 行数 | 責務 |
|-----------|---------|------|------|
| CLI | `main.rs` | 397 | 引数解析、ディレクトリ走査、rustfmt 呼び出し |
| Facade | `lib.rs` | 138 | パイプライン統合、strict/resilient モード切替 |
| Parser | `parser.rs` | 69 | SWC ラッパー |
| Registry | `registry.rs` | 1,275 | 型定義の事前収集（interface, enum, class, function） |
| External Types | `external_types.rs` | 749 | ビルトイン型 JSON の読み込みと TypeRegistry への登録 |
| Transformer | `transformer/` | ~8,500 | SWC AST → IR 変換 |
| - mod.rs | | 878 | モジュールレベル変換、TypeEnv、class前処理 |
| - types/ | | 2,000+ | 型注釈の変換 |
| - expressions/ | | 3,543 | 式の変換 |
| - statements/ | | 2,553 | 文の変換 |
| - functions/ | | 965 | 関数宣言の変換 |
| - classes.rs | | 1,613 | クラスの変換 |
| Generator | `generator/` | ~1,450 | IR → Rust ソース生成 |
| - mod.rs | | 1,225 | アイテム・文・式の生成 |
| - types.rs | | 227 | 型構文の生成 |
| IR | `ir.rs` | 1,100 | 中間表現の定義 |

### 問題 1: registry と transformer/types の循環依存

`registry.rs` は `convert_ts_type()` を呼んで型を変換するが、`convert_ts_type()` は `TypeRegistry` を引数に取る。registry 構築時は空の `TypeRegistry::new()` を渡しており、他の型への参照が解決できない。

```
registry.rs:248  →  convert_ts_type(&ann.type_ann, &mut Vec::new(), &TypeRegistry::new())
registry.rs:272  →  convert_ts_type(&ann.type_ann, &mut Vec::new(), &TypeRegistry::new())
```

**影響**: クラスプロパティの型注釈で、同じモジュール内の他の型を参照できない。例えば `class Foo { bar: Bar }` の `Bar` は registry 構築時には未登録。

**改善案**: registry 構築を2パスにする — 1パス目で型名だけ収集、2パス目で構築済み registry を渡して型を解決。

### 問題 2: transformer/mod.rs の責務過多

`transformer/mod.rs` が以下を全て担当している:
- モジュールレベルの変換ディスパッチ
- TypeEnv（変数型環境）の定義と管理
- クラスの前処理（pre_scan_classes, pre_scan_interface_methods）
- クラス継承の統合処理（transform_class_with_inheritance — 6分岐）
- trait 型ラッピング（wrap_trait_for_param, wrap_trait_for_value）
- アロー関数のトップレベル関数化（convert_var_decl_arrow_fns）
- ユーティリティ関数（extract_pat_ident_name, extract_prop_name, single_declarator）

**改善案**: TypeEnv を独立モジュール、クラス前処理/継承統合を classes モジュールに移動。

### 問題 3: 型情報の流れが暗黙的

型ヒントの伝播が呼び出し側の実装に依存している。`convert_expr()` の `expected: Option<&RustType>` パラメータは、呼び出し元が正しい型を渡すかどうかに依存する。

- 関数呼び出しの引数: `convert_call_args_with_types` が TypeRegistry から型を取得して渡す → 動作する
- メソッド呼び出しの引数: `resolve_expr_type` でオブジェクト型を解決してからメソッドパラメータを取得 → オブジェクト型が解決できない場合は型ヒントなし
- return 文: `convert_stmt` が `return_type` を受け取り、return 式に渡す → 動作する
- 変数宣言: `convert_var_decl` が型注釈を渡す → 動作する
- インポート関数の引数: TypeRegistry にないため型ヒントなし → **失敗する（76件のエラーの主因）**

**根本原因**: 単一ファイル変換モードでは、インポート先の関数パラメータ型が不明。ディレクトリモードの `build_shared_registry` で部分的に解決されるが、外部パッケージの型は対象外。

---

## 2. 中間レイヤー: モジュール内の構造

### expressions/mod.rs (3,543行) — 最大の問題

**490行の `map_method_call()`**: 文字列メソッド、配列メソッド、正規表現メソッドが1つの match 式に混在。

**DRY 違反**:
1. **パラメータパターン変換の重複**: `convert_fn_expr()` (892-1015) と `convert_arrow_expr_with_return_type()` (1156-1379) が Ident/Object/Assign/Rest/Array パターンの処理をほぼ同一のコードで実装
2. **イテレータメソッドの `.iter().cloned()` パターン**: `map`, `filter`, `find`, `some`, `every` で4回繰り返し
3. **デフォルトパラメータの `unwrap_or` ロジック**: 5箇所以上で同一パターン

**100行超の関数** (7個):
| 関数 | 行数 | 責務 |
|------|------|------|
| `map_method_call` | 490 | メソッド名マッピング |
| `convert_arrow_expr_with_return_type` | 224 | アロー関数変換 |
| `convert_call_expr` | 183 | 関数呼び出し変換 |
| `convert_object_lit` | 167 | オブジェクトリテラル変換 |
| `convert_bin_expr` | 133 | 二項演算変換 |
| `convert_call_args_with_types` | 130 | 引数の型付き変換 |
| `convert_fn_expr` | 124 | 関数式変換 |

### generator/mod.rs (1,225行) — DRY 違反

**`generate_enum()` と `generate_serde_tagged_enum()` の重複**: 両方が enum バリアントの生成、Display impl の生成、derive マクロの付与を行う。ロジックの大半が重複しており、タグ付き enum の差分は serde アトリビュートの追加のみ。

**Display impl の重複**: 数値 enum と文字列 enum で Display impl 生成がほぼ同一（lines 408-414 vs 417-423）。

### registry.rs (1,275行) — 設計の制約

**`collect_class_info()` が空の TypeRegistry で型変換**: クラスフィールドの型を変換する際、空の registry を使うため、同一モジュール内の他の型への参照が解決できない。

```rust
// registry.rs:248
convert_ts_type(&ann.type_ann, &mut Vec::new(), &TypeRegistry::new())
```

**`interface_names` セットの二重管理**: `register()` と `register_interface()` が分離しており、呼び出し側が `register_interface()` を忘れると `is_trait_type()` が正しく動作しない。登録箇所は `collect_decl()` と `load_types_json()` の2箇所のみだが、新しい登録パスが追加された場合にバグの原因になる。

### IR (ir.rs, 1,100行) — 表現力の限界

**`Expr` enum が30+バリアント**: 追加のたびに generator の全 match 式に影響。

**`Named` 型への文字列エンコード**: `&dyn Greeter` や `Box<dyn Greeter>` が `RustType::Named { name: "&dyn Greeter" }` として表現されており、型の構造情報が失われる。`is_trait_type` のような判定を generator 側で行えない。

---

## 3. ローレイヤー: 実装の詳細

### コメントの自己完結性

55+ の `I-XX` 参照がコードベース全体に散在。これらは TODO の項目 ID であり、TODO から削除されると意味不明になる。

**例**:
```rust
// I-86: Optional None completion for omitted fields
// I-68: self.field string concat gets .clone()
// I-90: String literal union enum and discriminated union
```

後続の開発者にとって、`I-86` が何を意味するかは TODO を見ないと分からない。さらに、TODO 項目は PRD 化・完了時に削除されるため、参照先が消失する。

**改善案**: コメントに I-XX を使わず、変換ルールの内容を自己完結的に記述する。
- Before: `// I-86: Optional None completion for omitted fields`
- After: `// Optional<T> フィールドが省略された場合、None で埋める（TS の省略可能プロパティに対応）`

### TODO コメント

`expressions/mod.rs:1642`:
```rust
// TODO: clone 削減 — Copy 型には .copied()、不要な clone は所有権解析で除去
```

このコメントは自己完結的で、I-XX 番号を使っていないため良い例。

### ハードコードされた値

- **`generator/mod.rs` のキーワードリスト**: Rust のキーワードがリテラル配列でハードコードされている
- **`expressions/mod.rs` のビルトイン関数チェック**: `parseInt`, `parseFloat`, `isNaN` が if-else チェーンで処理されている

---

## 4. 構造的な問題の優先度評価

### 高優先度（伝播リスク大）

1. **registry ↔ transformer の循環依存**: 型解決の正確性に直接影響。新しい型変換を追加するたびにこの制約に当たる
2. **expressions/mod.rs の肥大化**: 3,543行は保守性の限界を超えている。変更のたびに全体を理解する必要がある
3. **I-XX コメントの参照先消失**: TODO 項目の削除に伴い、コードの意図が失われていく

### 中優先度（保守性）

4. **パラメータ変換ロジックの重複**: fn_expr と arrow_expr で同一処理が重複。バグ修正が片方のみに適用されるリスク
5. **generator の enum 生成重複**: 新しい enum パターン追加時に2箇所を修正する必要
6. **型ヒント伝播の暗黙的な依存**: 新しいコンテキスト（例: for-of の右辺）で型ヒントを渡し忘れると silent に失敗する

### 低優先度（改善余地）

7. **map_method_call の490行**: 分割しても本質的な複雑さは変わらない（メソッドマッピングのテーブル的性質）
8. **Named 型への文字列エンコード**: 現時点では動作しているが、型情報の構造化が必要になった時点で負債化する

---

## 5. 推奨アクション

### 即座に対応すべき（コスト低、効果高）

1. **I-XX コメントを自己完結的な説明に置換**: grep で全件抽出し、一括置換。工数: 1-2時間
2. **パラメータ変換の共通化**: `convert_fn_expr` と `convert_arrow_expr_with_return_type` の重複部分を `convert_param_pat()` ヘルパーに抽出。工数: 2-3時間

### PRD 化して計画的に対応すべき

3. **expressions/mod.rs の分割**: `map_method_call` → `method_mapping.rs`、型解決 → `type_resolution.rs`、パターン検出 → `pattern_detection.rs` に分割。既存 I-31 と統合
4. **registry 構築の2パス化**: 型名収集パス + 型変換パスに分離。registry の型解決精度向上
5. **generator の enum 生成統一**: `generate_enum` と `generate_serde_tagged_enum` を統合

### 設計方針の見直しが必要

6. **型ヒント伝播の明示化**: 現在の `Option<&RustType>` による暗黙的な伝播を、変換コンテキスト構造体で明示化する。変換の各段階で何の型情報が利用可能かを型システムで保証する
7. **IR の `Named` 型エンコード見直し**: `&dyn Trait` / `Box<dyn Trait>` を構造的に表現する `RustType::Ref` / `RustType::DynTrait` バリアントの導入
