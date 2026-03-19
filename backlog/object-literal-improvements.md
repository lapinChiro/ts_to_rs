# オブジェクトリテラル変換の改善

対象 TODO: I-112b, I-134

## 背景・動機

OBJECT_LITERAL_NO_TYPE（76 インスタンス）は Hono ベンチマーク最大のエラーカテゴリ。関数引数のオブジェクトリテラルに型注釈がない場合、struct 名を決定できず変換が失敗する。

I-24 完了により TypeRegistry にビルトイン型（Response, Request 等 106 型）が登録済み。関数パラメータの型情報を活用してオブジェクトリテラルのフィールドを解決可能。

I-134（OBJECT_LITERAL_KEY、4 インスタンス）は計算プロパティキー `{ [key]: value }` が未対応。同じオブジェクトリテラル変換コードパスのため同時に対応する。

## ゴール

1. 関数呼び出しの引数位置にあるオブジェクトリテラルが、関数パラメータの型情報から struct 名とフィールド型を解決して変換される
2. `{ [key]: value }` の計算プロパティキーが HashMap 等に変換される
3. Hono ベンチマークの OBJECT_LITERAL_NO_TYPE が大幅に減少する（0 にはならない — 戻り値型伝播等の未対応パターンが残るため）
4. OBJECT_LITERAL_KEY が 0 になる
5. 既存テストに退行がない

## スコープ

### 対象

- 関数引数位置のオブジェクトリテラル型解決（TypeRegistry からパラメータ型を取得）
- 計算プロパティキー `{ [key]: value }` → `HashMap::from([(key, value)])` 変換
- ユニットテスト + スナップショットテスト + E2E テスト

### 対象外

- 戻り値型からのオブジェクトリテラル型推論（`return { ... }` の struct 名決定）
- 型注釈なし変数へのオブジェクトリテラル代入（I-112c、設計方針決定待ち）
- ネストしたオブジェクトリテラルの再帰的な型解決

## 設計

### I-112b: 関数引数の型解決

現在の `convert_call_args` は型ヒントなしでオブジェクトリテラルを変換しようとし、型名が不明で失敗する。

修正: `convert_call_expr` で関数の TypeDef を TypeRegistry から検索し、各引数位置のパラメータ型を取得。オブジェクトリテラルの引数に型ヒント（struct 名）を渡す。

```
// 疑似コード
let fn_def = reg.get(&fn_name);  // TypeDef::Function or TypeDef::Struct.methods
for (arg, param_type) in zip(call.args, fn_def.params) {
    if arg is ObjectLiteral && param_type is Named { name } {
        convert_object_lit(arg, struct_name = name, reg)
    }
}
```

### I-134: 計算プロパティキー

`{ [key]: value }` は `ast::PropOrSpread::Prop(Prop::KeyValue)` で key が `Computed` の場合。

変換: `HashMap::from([(key_expr, value_expr)])` を生成。

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/expressions/mod.rs` | `convert_call_expr` で型ヒント伝播、`convert_object_lit` の計算キー対応 |
| `src/transformer/expressions/tests.rs` | ユニットテスト |
| `tests/fixtures/` | スナップショットフィクスチャ |
| `tests/e2e/scripts/` | E2E スクリプト |

## 作業ステップ

- [ ] 1: 関数引数の型ヒント伝播テスト（RED）
- [ ] 2: TypeRegistry から関数パラメータ型を取得し、引数変換に渡す（GREEN）
- [ ] 3: ビルトイン型（ResponseInit 等）でのオブジェクトリテラル解決テスト
- [ ] 4: 計算プロパティキーのユニットテスト（RED）
- [ ] 5: 計算プロパティキーの HashMap 変換実装（GREEN）
- [ ] 6: E2E テスト
- [ ] 7: Hono ベンチマーク実行、OBJECT_LITERAL_NO_TYPE の変化確認
- [ ] 8: 退行チェック

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| 関数引数 OBJ_LIT | `foo({ status: 200 })` where `foo(init: ResponseInit)` | `foo(ResponseInit { status: 200.0 })` |
| 計算キー | `{ [key]: value }` | `HashMap::from([(key, value)])` |
| 型ヒントなし（フォールバック） | 型情報がない場合 | 既存のエラー報告（退行なし） |

## 完了条件

- [ ] 関数引数のオブジェクトリテラルが TypeRegistry の型情報で解決される
- [ ] 計算プロパティキーが HashMap に変換される
- [ ] Hono ベンチの OBJECT_LITERAL_NO_TYPE が 76 から減少
- [ ] OBJECT_LITERAL_KEY が 4 から 0 になる
- [ ] E2E テスト PASS
- [ ] 既存テストに退行がない
- [ ] clippy 0 警告、fmt PASS、全テスト PASS
