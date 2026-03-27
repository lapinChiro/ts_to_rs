# I-266: コンストラクタ引数・関数呼出し引数の expected type 伝播

## 背景

OBJECT_LITERAL_NO_TYPE エラー 54 件のうち、オブジェクトリテラルに expected type が設定されない（または誤った型が設定される）ことが根本原因。

### 調査結果: 66 件の失敗オブジェクトリテラルの内訳

デバッグ出力により、ディレクトリモード変換で失敗する全オブジェクトリテラル 66 件を以下に分類した（1 関数に複数のオブジェクトリテラルがある場合、エラーレポートでは 1 件として集計されるため、レポート上は 54 件）。

| カテゴリ | 件数 | 説明 |
|----------|------|------|
| PROPS_NO_EXPECTED | 29 | プロパティありだが expected 未設定 |
| EMPTY_OBJ_NO_EXPECTED | 19 | `{}` で expected 未設定 |
| SPREAD_NO_EXPECTED | 7 | スプレッド含むが expected 未設定 |
| WRONG_EXPECTED(String) | 6 | String が期待型として設定（誤り） |
| WRONG_EXPECTED(Tuple) | 2 | Tuple が期待型として設定（誤り） |
| WRONG_EXPECTED(Bool) | 2 | Bool が期待型として設定（誤り） |
| WRONG_EXPECTED(Any) | 1 | Any が期待型（パラメータ型が unknown） |

### 既存の `set_call_arg_expected_types` は正常に動作している

検証により、通常の関数呼び出し（CallExpr）に対する expected type 伝播は既に実装済みで正常動作することを確認した:
- TypeRegistry に登録された関数のパラメータ型 → 引数に伝播 ✓
- インライン匿名型パラメータ → synthetic struct として伝播 ✓
- メソッド呼び出し → メソッドシグネチャから伝播 ✓

### 真の根本原因: `resolve_new_expr` のコンストラクタ引数解決の欠陥

`resolve_new_expr` が `new Xxx(args)` の引数 expected type を設定する際、**コンストラクタのパラメータ型ではなく struct のフィールド型を i 番目の引数に対応付けている**。これは以下の理由で誤り:

1. **フィールド順序 ≠ コンストラクタパラメータ順序**: `new Response(body, init)` で Response の struct フィールドは `[headers, ok, redirected, status, statusText, type, url, body, bodyUsed]` だが、コンストラクタパラメータは `[body?: string, init?: ResponseInit]`
2. **フィールド型 ≠ コンストラクタパラメータ型**: 2 番目のフィールド `ok: bool` が 2 番目の引数 `{ status: 200 }` の expected type として設定される → `WRONG_EXPECTED(Bool)` のバグ
3. **コンストラクタ情報が TypeRegistry に格納されていない**: `collect_class_info` は `ClassMember::Constructor` を無視している。ビルトイン型 (`web_api.json`) にもコンストラクタ情報がない

**影響範囲の推定**:
- WRONG_EXPECTED(String/Tuple/Bool) の 10 件: フィールド型の誤った対応付けが原因 → コンストラクタ修正で全解消
- PROPS_NO_EXPECTED の多数: `new Response(body, { status, headers })` パターン → コンストラクタ解決で解消
- WRONG_EXPECTED(Any) の 1 件: パラメータ型 `unknown` は修正不可

## 完了基準

1. `collect_class_info` がコンストラクタパラメータを TypeDef::Struct に格納する
2. ビルトイン型抽出ツールがコンストラクタシグネチャを JSON に含める
3. `web_api.json` / `ecmascript.json` にコンストラクタ情報が含まれる
4. `resolve_new_expr` がコンストラクタパラメータ型を使って引数の expected type を設定する
5. `new Response("body", { status: 200 })` パターンで `{ status: 200 }` に `ResponseInit` の expected type が設定される
6. WRONG_EXPECTED_TYPE のバグ（10 件）が全解消される
7. 既存テスト全通過、clippy 0 警告、fmt pass
8. Hono ベンチマークで OBJECT_LITERAL_NO_TYPE の削減を検証

## 設計

### 1. TypeDef::Struct にコンストラクタシグネチャを追加

```rust
// registry/mod.rs
pub enum TypeDef {
    Struct {
        type_params: Vec<TypeParam>,
        fields: Vec<(String, RustType)>,
        methods: HashMap<String, Vec<MethodSignature>>,
        constructor: Option<Vec<MethodSignature>>,  // ← 追加
        extends: Vec<String>,
        is_interface: bool,
    },
    // ...
}
```

`MethodSignature` を再利用する。コンストラクタは名前を持たないため `Option<Vec<MethodSignature>>` としてオーバーロード対応。

### 2. `collect_class_info` でコンストラクタを収集

```rust
// registry/collection.rs — collect_class_info 内
ast::ClassMember::Constructor(ctor) => {
    let params: Vec<(String, RustType)> = ctor.params.iter().filter_map(|p| {
        match p {
            ast::ParamOrTsParamProp::Param(param) => {
                // 通常のパラメータ（Method と同じ処理）
            }
            ast::ParamOrTsParamProp::TsParamProp(param_prop) => {
                // Parameter property (e.g., constructor(public name: string))
            }
        }
    }).collect();
    constructor_sigs.push(MethodSignature { params, return_type: None });
}
```

### 3. ビルトイン型抽出ツールにコンストラクタ情報を追加

`tools/extract-types/src/index.ts` を修正して、インターフェースのコンストラクタシグネチャ (`ConstructSignatureDeclaration`) を JSON に含める。

JSON スキーマ:
```json
{
  "name": "Response",
  "kind": "interface",
  "fields": [...],
  "methods": {...},
  "constructors": [
    {
      "params": [
        { "name": "body", "type": { "kind": "union", "types": [...] }, "optional": true },
        { "name": "init", "type": { "kind": "named", "name": "ResponseInit" }, "optional": true }
      ]
    }
  ]
}
```

### 4. JSON ローダーでコンストラクタを TypeDef に読み込み

`src/external_types/mod.rs` の JSON パーサーがコンストラクタ情報を `TypeDef::Struct.constructor` に格納する。

### 5. `resolve_new_expr` の修正

```rust
// type_resolver/expressions.rs — resolve_new_expr
fn resolve_new_expr(&mut self, new_expr: &ast::NewExpr) -> ResolvedType {
    // ...
    if let Some(type_def) = self.registry.get(&class_name) {
        if let Some(args) = &new_expr.args {
            // コンストラクタシグネチャから引数の expected type を設定
            let param_types = match type_def {
                TypeDef::Struct { constructor: Some(sigs), .. } => {
                    // オーバーロード対応: 引数数でマッチするシグネチャを選択
                    sigs.iter()
                        .find(|s| s.params.len() >= args.len())
                        .map(|s| s.params.iter().map(|(_, ty)| ty.clone()).collect())
                }
                TypeDef::Struct { fields, .. } => {
                    // フォールバック: コンストラクタ未定義の場合はフィールドを使用
                    // （TS の省略コンストラクタはフィールド = パラメータ）
                    Some(fields.iter().map(|(_, ty)| ty.clone()).collect())
                }
                _ => None,
            };
            // expected type の設定（既存ロジックと同様）
        }
    }
}
```

### スコープ外

- `return {}` パターンの empty object 解決（独立した改善）
- スプレッドオペレータの expected type 伝播（I-269）
- ジェネリクスパラメータの展開（I-268）
- パラメータ型が `unknown`/`any` のケース（構造体名を特定不可）

## タスク

### Phase 1: TypeDef にコンストラクタフィールド追加 + collect_class_info 修正

1. `TypeDef::Struct` に `constructor: Option<Vec<MethodSignature>>` を追加
2. 全ての `TypeDef::Struct` 生成箇所に `constructor: None` を追加
3. `collect_class_info` に `ClassMember::Constructor` ハンドラを追加
4. テスト: ユーザー定義クラスのコンストラクタパラメータが TypeRegistry に登録されることを検証

### Phase 2: resolve_new_expr の修正

5. `resolve_new_expr` をコンストラクタシグネチャ優先に変更
6. RED: `new MyClass(arg1, { field: value })` パターンのテスト追加
7. GREEN: コンストラクタパラメータから expected type を設定
8. 既存テスト全通過を確認

### Phase 3: ビルトイン型のコンストラクタ情報追加

9. `tools/extract-types/src/index.ts` にコンストラクタ抽出ロジック追加
10. JSON スキーマにコンストラクタフィールド追加
11. `src/external_types/mod.rs` のローダーでコンストラクタを読み込み
12. `web_api.json` / `ecmascript.json` を再生成
13. テスト: `new Response("body", { status: 200 })` で `{ status: 200 }` に `ResponseInit` expected type が設定されることを検証

### Phase 4: 検証

14. 既存テスト全通過（1372）
15. clippy 0 警告、fmt pass
16. Hono ベンチマークで OBJECT_LITERAL_NO_TYPE 削減を定量検証
17. WRONG_EXPECTED_TYPE バグ（10 件）の解消を確認

## 依存関係

- なし（独立して実施可能）
- Phase 3 はビルトイン型抽出ツール（`tools/extract-types/`）に依存するが、手動で JSON を更新することで回避可能

## リスク

- `TypeDef::Struct` にフィールド追加するため、全ての `TypeDef::Struct` 生成・パターンマッチ箇所の更新が必要（影響範囲は広いが機械的な変更）
- ビルトイン型の `ConstructSignatureDeclaration` は TypeScript の型宣言ファイルから抽出する必要があり、抽出ツールの修正が必要
