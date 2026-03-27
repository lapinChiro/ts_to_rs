# 完了済み機能一覧

変換ツールが対応済みの機能・イシューの一覧。git history で追跡可能だが、再実装防止と依存関係の文脈のために集約。

## ビルトイン型・外部型

- **ビルトイン型**: Web API 型（105 型）+ ECMAScript 標準型（57 型: String, Array, Date, Error, RegExp, Map, Set, Promise 等）がバイナリ埋め込み済み。`src/builtin_types/web_api.json` + `ecmascript.json` に分割管理
- **Node.js API**: `fs.readFileSync/writeFileSync/existsSync`、`process.env.VAR`、stdin パターンの変換が動作
- **I-211 ECMAScript 標準型**: オーバーロード解決（I-211-a）、ECMAScript 型抽出・JSON 分割（I-211-b）、検証・E2E テスト・ベンチマーク（I-211-c）完了。String/Array/Map/Set 等のメソッドチェーン型追跡が動作
- **I-270 ビルトイン型 struct 定義自動生成**: 外部型 struct の JSON フィールド情報ベース自動生成（推移的依存の固定点計算）。type_params 抽出パイプライン（FORMAT_VERSION 2）。フィールド名サニタイズの IR レベル移動。未定義型のスタブ struct 自動生成。types.rs インポート自動生成。types.rs エラー 36→5（残 5 件は I-273 として追跡）。HashMap 変換時の自動インポート（旧 I-190）も同時解決

## 型変換・型推論

- **Interface → Trait**: interface は内容に応じて struct/trait/struct+trait+impl に 3 分類変換。extends, 交差型 trait 合成対応
- **Trait パラメータ型**: `&dyn Trait` / `Box<dyn Trait>` 変換 + 呼び出し側型強制（`&*` / `Box::new()`）
- **チェーンメソッド型追跡**: `MethodSignature`（パラメータ + 戻り値型）を TypeRegistry に格納。`TypeResolver::resolve_method_return_type` でメソッド呼び出しの戻り値型を解決
- **ジェネリクス基盤**: `TypeParam`（名前 + 制約）を IR に定義。`TypeDef::Struct`/`Enum` に `type_params` 格納。`RustType::substitute` で型パラメータの具体型置換。`TypeRegistry::instantiate` でジェネリック型のインスタンス化
- **I-218 ジェネリクス基盤の残課題**: class/type alias の type_params 収集。substitute_types の Enum 対応。Item::Impl に type_params 追加。ClassInfo への type_params 伝播。`TraitRef { name, type_args }` 導入
- **I-112c オブジェクトリテラル型推定 Phase 1-3**: TypeResolver の expected type 設定強化、匿名構造体自動生成、スプレッドマージ型推定。70→53 件に削減。残 53 件は I-266〜I-269 の独立イシューとして追跡
- **I-212 union 型 enum 重複定義の解消**: per-file `SyntheticTypeRegistry` の `register_union` dedup により同一ファイル内の同じ union 型は 1 つの enum に統一
- **I-226 TypeEnv 完全除去**: TypeEnv 構造体を完全削除。全型情報を FileTypeResolution 経由に一本化

## 型 narrowing

- typeof/instanceof/null-check/truthy → `if let` パターン生成。any 型 → enum 自動生成。typeof "object"/"function" バリアント解決。複合条件 (&&) のネスト if let。三項演算子の `Expr::IfLet`。switch (typeof x) の match 変換。楽観的 `true` フォールバックは `todo!()` に置換済み

## コンパイルエラー修正

- **I-223/I-227 コンパイルエラーのクイックウィン**: 文字列リテラルの Rust エスケープ（`escape_rust_string` ヘルパー）。union enum 名の識別子サニタイズ（`variant_name_for_type` を `pub(crate)` に統一）
- **I-241〜I-248 サイレント意味変更の完全解消**: エラー報告統一（UnsupportedSyntaxError::new）、BigInt i128 拡張、RuntimeTypeof ヘルパー、トップレベル init() 関数、ネスト destructuring rest の合成構造体変換、declare module エラー伝播、private class members 変換、intersection メソッドシグネチャの impl 生成、union キーワード型の明示的パターンマッチ化

## モジュール・ディレクトリ

- **ディレクトリモード改善**: 中間ディレクトリの mod.rs 生成。`../` import パスの `crate::` パス解決。ハイフン入りファイル名のアンダースコア変換。ディレクトリ単位コンパイルチェック
- **モジュール参照解決の一本化**: `ModuleGraph` が全ファイルの import/export を事前解析し re-export チェーンを解決。`TrivialResolver`（単一ファイルモード用）導入。パス→モジュールパス変換を `file_path_to_module_path()` に集約

## ツール・サブコマンド

- **resolve-types**: `ts_to_rs resolve-types --tsconfig ...` サブコマンド実装済み

## リファクタリング

- **I-192 大規模ファイル分割 + I-271 ファイル行数制限**: 元 18 個の 1000 行超ファイルを全て分割。`scripts/check-file-lines.sh`（閾値 1000 行）で再発防止
- **I-225 compile_test 失敗時のフィクスチャ名表示**: `tests/compile_test.rs:94` でフィクスチャ名が表示されるよう改善済み
