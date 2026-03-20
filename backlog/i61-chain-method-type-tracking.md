# I-61: チェーンメソッド呼び出しの戻り値型追跡

## 背景・動機

`obj.method1().method2()` のようなメソッドチェーンで、`method1()` の戻り値型が不明なため `method2()` の変換が正しく行われない。

根本原因は 3 層に分かれる:

1. **TypeRegistry**: メソッドシグネチャにパラメータ型のみ格納し、**戻り値型を格納していない**
2. **型解決**: `resolve_call_return_type()` がメソッド呼び出し（`Member` 式）に対応していない
3. **メソッドマッピング**: `map_method_call()` が変換結果の型情報を出力しない

これにより Hono のようなメソッドチェーン多用のコードベースで `.to_string()` 漏れ、`.collect()` 漏れが大量に発生する。

## ゴール

1. TypeRegistry のメソッドシグネチャが戻り値型を含む
2. `resolve_expr_type()` がメソッド呼び出しの戻り値型を解決できる
3. チェーンメソッド `a.b().c()` で `c()` のパラメータ型ルックアップが正しく動作する
4. ビルトインメソッド（`.trim()`, `.split()`, `.map()` 等）の戻り値型が追跡される

## スコープ

### 対象

- `TypeDef::Struct::methods` の構造拡張（パラメータ + 戻り値型）
- `collect_interface_methods` での TS メソッド戻り値型の収集
- `resolve_call_return_type` のメソッド呼び出し対応
- ビルトインメソッド（String, Array 等）の戻り値型定義
- `map_method_call` の戻り値型メタデータ出力

### 対象外

- ジェネリックメソッドの型引数解決（I-100 の範囲）
- ユーザー定義クラスのメソッドチェーン（TypeRegistry にクラスメソッドを登録する基盤は既存だが、完全な対応は I-100 後）

## 設計

### 技術的アプローチ

#### 1. TypeDef::Struct::methods の拡張

```rust
// 現在
methods: HashMap<String, Vec<(String, RustType)>>,  // params only

// 変更後
methods: HashMap<String, MethodSignature>,

pub struct MethodSignature {
    pub params: Vec<(String, RustType)>,
    pub return_type: Option<RustType>,
}
```

#### 2. メソッド戻り値型の収集

`collect_interface_methods` (registry.rs) を拡張し、`TsMethodSignature::type_ann` から戻り値型を収集:

```rust
let return_type = method.type_ann.as_ref()
    .map(|ann| convert_ts_type(&ann.type_ann, &mut vec![], reg))
    .transpose()?;
```

#### 3. ビルトインメソッドの戻り値型

ビルトイン型定義ファイル（`src/builtin_types.d.ts` 埋め込み）に既にメソッドシグネチャがある。`collect_interface_methods` が戻り値型も収集するようになれば、ビルトインメソッドの戻り値型も自動的に TypeRegistry に入る。

追加で、`map_method_call` のハードコードされたメソッドマッピングにも戻り値型を付与する:

| メソッド | 入力型 | 戻り値型 |
|----------|--------|----------|
| `trim()` | String | String |
| `split()` | String | Vec<String> |
| `map()` | Vec<T> | Vec<U>（クロージャの戻り値型） |
| `filter()` | Vec<T> | Vec<T> |
| `find()` | Vec<T> | Option<T> |
| `join()` | Vec<String> | String |
| `toLowerCase()` | String | String |
| `toString()` | any | String |

#### 4. resolve_call_return_type の拡張

```rust
fn resolve_call_return_type(call: &ast::CallExpr, type_env: &TypeEnv, reg: &TypeRegistry) -> Option<RustType> {
    let callee = call.callee.as_expr()?;
    match callee.as_ref() {
        ast::Expr::Ident(ident) => {
            // 既存: 関数名から TypeRegistry ルックアップ
        }
        ast::Expr::Member(member) => {
            // 新規: オブジェクト型を解決 → メソッドの戻り値型を取得
            let obj_type = resolve_expr_type(&member.obj, type_env, reg)?;
            let method_name = member.prop.as_ident()?.sym.to_string();
            resolve_method_return_type(&obj_type, &method_name, reg)
        }
        _ => None,
    }
}
```

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/registry.rs` | `MethodSignature` 構造体追加、`TypeDef::Struct::methods` の型変更、`collect_interface_methods` の戻り値型収集 |
| `src/transformer/expressions/type_resolution.rs` | `resolve_call_return_type` のメソッド呼び出し対応 |
| `src/transformer/expressions/methods.rs` | `map_method_call` の戻り値型メタデータ（将来的な統合点） |
| `src/transformer/expressions/calls.rs` | メソッドパラメータ型取得を `MethodSignature` 対応に更新 |
| テストファイル | 新規テストケース追加 |

## 作業ステップ

- [ ] ステップ1（RED）: チェーンメソッドの型追跡テストを書く（`"hello".trim().split(" ")` で split のパラメータ型が解決される）
- [ ] ステップ2（GREEN）: `MethodSignature` 構造体を追加し、`TypeDef::Struct::methods` を移行
- [ ] ステップ3（GREEN）: `collect_interface_methods` で戻り値型を収集
- [ ] ステップ4（GREEN）: `resolve_call_return_type` にメソッド呼び出し対応を追加
- [ ] ステップ5（GREEN）: 既存の `map_method_call` 参照箇所を `MethodSignature` 対応に更新
- [ ] ステップ6（REFACTOR）: ビルトインメソッドの戻り値型が TypeRegistry 経由で自動解決されることを確認
- [ ] ステップ7: E2E スナップショットテスト（メソッドチェーンを含む TS ファイル）

## テスト計画

### 単体テスト

- `"hello".trim().split(" ")` → `trim()` の戻り値が `String`、`split()` のオブジェクト型が `String`
- `[1,2,3].map(x => x * 2).filter(x => x > 2)` → `map()` の戻り値が `Vec<f64>`、`filter()` のオブジェクト型が `Vec<f64>`
- interface メソッドの戻り値型が TypeRegistry に格納される
- `resolve_expr_type` がメソッド呼び出し式の戻り値型を返す
- メソッドの戻り値型が不明な場合に `None` が返る（エラーにならない）

### E2E テスト

- メソッドチェーンを含む TS ファイルの変換スナップショット

## 完了条件

- 全テストパターンが GREEN
- TypeRegistry のメソッドシグネチャが戻り値型を含む
- チェーンメソッドの 2 段目以降でパラメータ型ルックアップが動作する
- `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- `cargo fmt --all --check` が通る
- `cargo test` が全パス
- `cargo llvm-cov` のカバレッジ閾値を満たす
