# TypeRegistry — 型定義の事前収集と参照

## 背景・動機

現在の変換パイプラインは 1 パス・ステートレスで、式の変換時にモジュール内の他の宣言（interface, enum, function）を参照できない。このため以下の変換が不可能:

- ネストしたオブジェクトリテラル: `{ origin: { x: 0, y: 0 } }` で内側の構造体名を解決できない
- 関数引数のオブジェクトリテラル: `draw({ x: 0, y: 0 })` で関数パラメータの型から構造体名を解決できない
- enum メンバーアクセス: `Color.Red` が `Color::Red`（enum アクセス）なのか `Color.Red`（フィールドアクセス）なのか判別できない

これらはいずれも「変換元ファイル内の他の宣言を参照する」という共通の課題に起因する。

## ゴール

変換パイプラインに型定義収集パス（TypeRegistry）を追加し、以下の 3 つの変換が正しく動作する:

1. `const r: Rect = { origin: { x: 0, y: 0 }, size: { w: 10, h: 20 } }` → `Rect { origin: Origin { x: 0.0, y: 0.0 }, size: Size { w: 10.0, h: 20.0 } }`（ネストしたオブジェクトリテラル）
2. `draw({ x: 0, y: 0 })` → `draw(Point { x: 0.0, y: 0.0 })`（関数引数のオブジェクトリテラル、関数 `draw(p: Point)` が同一モジュールまたは import 先に定義されている場合）
3. `Color.Red` → `Color::Red`（TypeRegistry で `Color` が enum であることを判別）

ディレクトリモードでは、import で参照された外部モジュールの型定義も解決できる。

## スコープ

### 対象

- `TypeRegistry` データ構造の定義（struct フィールド型、enum バリアント名、関数パラメータ型を保持）
- 事前収集パスの実装（SWC AST を走査して TypeRegistry を構築）
- `transform_module` が TypeRegistry を受け取るように変更
- `convert_expr` に TypeRegistry への参照を渡す
- ネストしたオブジェクトリテラルの変換（TypeRegistry でフィールド型を解決）
- 関数引数のオブジェクトリテラルの変換（TypeRegistry でパラメータ型を解決）
- enum メンバーアクセスの変換（`obj.field` が enum アクセスかどうかを TypeRegistry で判別）
- ディレクトリモードでの外部モジュール型解決（import 先のファイルを事前にスキャン）
- `transpile` API の拡張（TypeRegistry を受け取るバリアント追加）

### 対象外

- 外部パッケージ（`node_modules`）の型定義解決
- ジェネリック型の具体化（`Container<string>` のフィールド型を `String` に解決する等）
- 型の構造的一致による推論（TypeScript の structural typing のエミュレーション）
- re-export（`export { Foo } from "./bar"`）の追跡

## 設計

### 技術的アプローチ

#### 1. TypeRegistry データ構造

```rust
/// モジュール内の型定義を保持するレジストリ。
pub struct TypeRegistry {
    types: HashMap<String, TypeDef>,
}

/// 型定義の種類。
pub enum TypeDef {
    /// struct (interface/type alias から変換)
    Struct { fields: Vec<(String, RustType)> },
    /// enum
    Enum { variants: Vec<String> },
    /// 関数
    Function {
        params: Vec<(String, RustType)>,
        return_type: Option<RustType>,
    },
}
```

#### 2. パイプラインの変更

```
// 旧: 1パス
TS source → parser → SWC AST → transform_module → IR → generate

// 新: 2パス
TS source → parser → SWC AST ──┬→ collect_types → TypeRegistry
                                └→ transform_module(&TypeRegistry) → IR → generate
```

単一ファイルモード: 同一ファイル内の宣言のみ収集。
ディレクトリモード: 全 `.ts` ファイルを先にパースして TypeRegistry を構築し、各ファイルの変換に共有 TypeRegistry を渡す。

#### 3. 公開 API の変更

```rust
// 既存（互換性維持）
pub fn transpile(ts_source: &str) -> Result<String>

// 新規（ディレクトリモード用）
pub fn transpile_with_registry(ts_source: &str, registry: &TypeRegistry) -> Result<String>
pub fn build_registry(module: &Module) -> Result<TypeRegistry>
```

`transpile()` は内部で `build_registry` → `transpile_with_registry` を呼ぶ。

#### 4. ネストしたオブジェクトリテラルの変換

`convert_object_lit` で各フィールドの値を変換する際、TypeRegistry から親 struct のフィールド型を参照する:

```rust
// { origin: { x: 0, y: 0 } } で parent_type = "Rect" の場合:
// 1. TypeRegistry から Rect の fields を取得
// 2. "origin" フィールドの型が Named("Origin") であることを確認
// 3. 内側の { x: 0, y: 0 } を expected_type = Named("Origin") で変換
```

#### 5. 関数引数のオブジェクトリテラルの変換

`convert_call_expr` で引数がオブジェクトリテラルの場合、TypeRegistry から関数のパラメータ型を参照する:

```rust
// draw({ x: 0, y: 0 }) の場合:
// 1. callee が Ident("draw") → TypeRegistry から draw の TypeDef::Function を取得
// 2. 第1パラメータの型が Named("Point") であることを確認
// 3. { x: 0, y: 0 } を expected_type = Named("Point") で変換
```

#### 6. enum メンバーアクセスの変換

`convert_member_expr` で、オブジェクトが識別子かつ TypeRegistry で enum として登録されている場合、`Expr::FieldAccess` の代わりに enum バリアントアクセスの構文を生成する:

```rust
// Color.Red の場合:
// 1. object が Ident("Color") → TypeRegistry で Color が Enum であることを確認
// 2. Expr::Ident("Color::Red") を生成（FieldAccess ではなく）
```

#### 7. 外部モジュールの型解決

ディレクトリモードでの処理順序:

1. 全 `.ts` ファイルをパース（SWC AST を取得）
2. 各ファイルの型定義を収集し、ファイルパスをキーにした `HashMap<PathBuf, TypeRegistry>` を構築
3. 各ファイルの import 宣言を解析し、import 先の TypeRegistry から該当する型を現在のファイルの TypeRegistry にマージ
4. マージ済み TypeRegistry を使って各ファイルを変換

### 影響範囲

- `src/ir.rs` — `TypeRegistry`, `TypeDef` の追加（新規モジュール `src/registry.rs` でも可）
- `src/lib.rs` — `transpile_with_registry`, `build_registry` の追加
- `src/transformer/mod.rs` — `transform_module` が `&TypeRegistry` を受け取る
- `src/transformer/expressions.rs` — `convert_expr` が `&TypeRegistry` を受け取る、`convert_member_expr` の enum 判定、`convert_object_lit` のフィールド型解決、`convert_call_expr` のパラメータ型解決
- `src/transformer/statements.rs` — `convert_stmt` が `&TypeRegistry` を受け取る
- `src/transformer/functions.rs` — `convert_fn_decl` が `&TypeRegistry` を受け取る
- `src/transformer/classes.rs` — `convert_class_decl` が `&TypeRegistry` を受け取る
- `src/main.rs` — ディレクトリモードで全ファイル事前スキャン

## 作業ステップ

- [ ] ステップ1: `TypeRegistry` と `TypeDef` のデータ構造を定義する。ユニットテストで構築・参照の基本動作を検証
- [ ] ステップ2: `build_registry` を実装し、SWC AST から interface/type alias/enum/function の型情報を収集する。テストで各種宣言の収集を検証
- [ ] ステップ3: `transform_module` と `convert_expr` のシグネチャに `&TypeRegistry` を追加する。全呼び出し元を更新し、空の TypeRegistry を渡して既存テストが通ることを確認
- [ ] ステップ4: ネストしたオブジェクトリテラル — `convert_object_lit` で TypeRegistry からフィールド型を解決し、内側のオブジェクトリテラルに expected_type を伝播する
- [ ] ステップ5: 関数引数のオブジェクトリテラル — `convert_call_expr` で TypeRegistry から関数パラメータ型を解決し、オブジェクトリテラル引数に expected_type を伝播する
- [ ] ステップ6: enum メンバーアクセス — `convert_member_expr` で TypeRegistry を参照し、enum の場合は `Color::Red` 形式の `Expr::Ident` を生成する
- [ ] ステップ7: 外部モジュールの型解決 — `main.rs` のディレクトリモードで全ファイル事前スキャン、import に基づく TypeRegistry マージを実装する
- [ ] ステップ8: `transpile_with_registry` と `transpile` の統合 — 公開 API を整理し、`transpile()` が内部で TypeRegistry を使うように変更
- [ ] ステップ9: スナップショットテスト — ネストしたオブジェクト、関数引数のオブジェクト、enum アクセスの fixture を追加

## テスト計画

- 正常系: `build_registry` が interface/type alias からフィールド名・型を正しく収集する
- 正常系: `build_registry` が enum からバリアント名を収集する
- 正常系: `build_registry` が function からパラメータ名・型、戻り値型を収集する
- 正常系: ネストしたオブジェクトリテラル（`{ origin: { x: 0, y: 0 } }` → `Rect { origin: Origin { ... } }`）
- 正常系: 関数引数のオブジェクトリテラル（`draw({ x: 0, y: 0 })` → `draw(Point { ... })`）
- 正常系: enum メンバーアクセス（`Color.Red` → `Color::Red`）
- 正常系: 外部モジュールの型解決（import 先の interface がオブジェクトリテラル変換に使える）
- 異常系: TypeRegistry に存在しない型名のオブジェクトリテラル（フォールバック: 既存の動作を維持）
- 異常系: TypeRegistry に存在しない関数名の呼び出し（フォールバック: 通常の関数呼び出しとして変換）
- 境界値: 空の TypeRegistry（既存の全テストが回帰しない）
- 境界値: import のない単一ファイルモード
- スナップショット: ネストしたオブジェクト、関数引数、enum アクセスの fixture 追加

## 完了条件

- `TypeRegistry` がモジュール内の interface/type alias/enum/function の型情報を収集できる
- ネストしたオブジェクトリテラルが正しく変換される
- 関数引数のオブジェクトリテラルが正しく変換される
- `Color.Red` → `Color::Red` が正しく変換される
- ディレクトリモードで import 先の型定義が解決される
- 既存の全テストが回帰しない
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
