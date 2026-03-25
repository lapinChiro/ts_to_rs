# サイレント意味変更の完全解消（I-241〜I-248）

## 背景・動機

変換パイプラインに 8 箇所のサイレントな意味変更・消失がある。これらは TypeScript コードを Rust に変換する際に、エラーも警告も出さずに意味の異なるコードを生成するか、ロジックを消失させる。

変換ツールとして最も危険なカテゴリである。コンパイルエラーなら開発者が気づけるが、サイレントな意味変更は気づけない。

加えて、エラー報告に 3 つのパターン（`UnsupportedSyntaxError` / `anyhow!` / 暗黙スキップ）が混在しており、DRY でない。

## ゴール

1. 8 箇所すべてのサイレント意味変更・消失を解消する
2. 変換可能なケースは実際に Rust コードを生成する（unsupported エラーに逃げない）
3. エラー報告を統一的な機構に DRY 化する
4. `typeof` 未解決時のランタイムヘルパーにより、型が静的に解決できないケースでも正しい typeof 結果を保証する

## スコープ

### 対象

- **I-241**: BigInt リテラルの i128 拡張（`unwrap_or(0)` の除去）
- **I-242**: `typeof` 未解決型のランタイム typeof ヘルパー生成
- **I-243**: トップレベル式文の `init()` 関数変換
- **I-244**: ネスト destructuring rest パラメータの synthetic struct 変換
- **I-245**: `declare module` 内エラーの伝播修正
- **I-246**: PrivateMethod / StaticBlock / PrivateProp の変換
- **I-247**: TODO 記述の修正（調査の結果、既に変換済みと判明。bigint→i128 同期のみ）
- **I-248**: intersection 内メソッドシグネチャの impl ブロック生成
- **エラー報告統一**: 3 パターンを統一的な機構に DRY 化

### 対象外

- TypeResolver の全般的な型推論カバレッジ改善（I-224, I-266 等の OBJECT_LITERAL_NO_TYPE ロードマップで扱う）
- `num-bigint` クレートの導入（i128 範囲を超える BigInt。YAGNI — 実際に i128 溢れが発生してから対応）

## 設計

### 技術的アプローチ

#### 1. エラー報告の統一（DRY 化）

現状の 3 パターン:

1. `Err(UnsupportedSyntaxError { kind, byte_pos }.into())` — モジュールレベル
2. `Err(anyhow!("message"))` — 内部変換エラー（`transform_module_collecting` で catch されて UnsupportedSyntaxError に変換）
3. 暗黙スキップ（`continue`, 空 `vec![]`, `_ => {}`）— 今回の 8 箇所

パターン 2 と 3 を解消する。内部変換エラーも `UnsupportedSyntaxError` を直接生成し、暗黙スキップは変換実装またはエラー生成に置き換える。

具体的な統一方針:

- `UnsupportedSyntaxError` にヘルパーコンストラクタを追加: `UnsupportedSyntaxError::new(kind: impl Into<String>, span: Span)` — `byte_pos: span.lo.0` の手動抽出を排除
- 各モジュール内の `anyhow!("unsupported ...")` を `UnsupportedSyntaxError::new(...)` に置き換え。`anyhow!` は変換ロジックの失敗（バグ相当）のみに限定し、未対応構文の報告には `UnsupportedSyntaxError` を使う
- `transform_module_collecting` の `Err(other)` ブランチ（L227-234）は互換性のため残すが、新規コードでは `UnsupportedSyntaxError` を直接返す

#### 2. I-241: BigInt リテラルの i128 拡張

- `Expr::IntLit(i64)` を `Expr::IntLit(i128)` に拡張
- `src/transformer/expressions/literals.rs:70`: `parse::<i64>().unwrap_or(0)` → `parse::<i128>` に変更。パース失敗時は `UnsupportedSyntaxError` を返す（サイレント 0 丸め禁止）
- `src/pipeline/type_converter.rs:1721-1727`: union 内の `TsBigIntKeyword` のバリアント名を `"I128"` に変更、型を `i128` に
- `src/generator/expressions.rs`: `IntLit(i128)` の出力に対応（i128 リテラルに suffix `_i128` を付与するか、値の範囲で i64/i128 を判定）

#### 3. I-242: ランタイム typeof ヘルパー

TypeScript の `typeof` は本質的にランタイム操作である。TypeResolver で静的に解決できない場合にサイレントに `"object"` を返すのではなく、ランタイムで正しい結果を返すヘルパーを生成する。

- 新しい IR ノード `Expr::RuntimeTypeof { operand: Box<Expr> }` を追加
- Generator で以下を出力:

```rust
fn js_typeof(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "undefined",
        _ => "object",
    }
}
```

- `convert_unary_expr` の `None` ケース（L168）: operand の Rust 型が `serde_json::Value`（Any）相当なら `Expr::RuntimeTypeof` を生成。型が Not Any かつ Unknown なら `UnsupportedSyntaxError` を返す（TypeResolver のギャップを示す）
- ヘルパー関数はファイル内で `RuntimeTypeof` が 1 回以上使われた場合のみ生成する（generator が使用有無を追跡）

#### 4. I-243: トップレベル式文の init() 関数変換

TypeScript のトップレベル式文は「モジュールが初めて import されたとき、宣言順に 1 回だけ実行される」セマンティクスを持つ。Rust にはモジュールロード時の自動実行がないため、`pub fn init()` 関数として生成する。

- `transform_module_item` の `ModuleItem::Stmt(Stmt::Expr(expr_stmt))` ケース（L293-295）を変更
- 式文を `Expr` に変換し、`Item::Fn` として `init` 関数に格納する
- 同一ファイル内に複数のトップレベル式文がある場合、1 つの `init()` 関数にまとめる（本体に全式文を順番に格納）
- `init` 関数は `pub fn init()` として生成（呼び出し責務は利用者に委ねる）

実装: `Transformer` にトップレベル式文を蓄積する `Vec<Expr>` フィールドを追加。`transform_module` の最後に、蓄積があれば `Item::Fn { name: "init", ... }` を生成する。

#### 5. I-244: ネスト destructuring rest パラメータ

`{ a, ...rest }` の rest を、元の型から明示的に destructure されたフィールドを除いた synthetic struct として変換する。

- `src/transformer/functions/mod.rs:734` の `Rest(_) => {}` を変換ロジックに置き換え
- 元の構造体型を TypeRegistry から取得（destructuring 対象の型注釈 or 推論型）
- 明示的に取り出されたフィールド名を収集し、残りのフィールドで `{ParentName}Rest` synthetic struct を生成
- rest 変数を synthetic struct のインスタンスとして初期化: `let rest = ParentNameRest { b: param.b, c: param.c };`
- 型情報が取得できない場合（any 型等）: `UnsupportedSyntaxError` を返す（不完全な変換をサイレントに生成しない）

#### 6. I-245: declare module 内エラー伝播

- `src/transformer/mod.rs:491` の `if let Ok(...)` を `match` + エラー収集に変更
- `resilient` モード（`--report-unsupported`）の場合: エラーを warnings ベクタに記録して続行
- 非 resilient モードの場合: `?` でエラーを伝播
- `transform_module_item` のシグネチャは既に warnings を `Vec<String>` で返しているので、そこに追加

#### 7. I-246: PrivateMethod / StaticBlock / PrivateProp

既存の変換ロジック（`convert_class_method`, `convert_class_prop`）を拡張して対応する。

**PrivateMethod** (`ClassMember::PrivateMethod`):
- `#method()` → 非 `pub` メソッドとして変換（Rust の可視性でプライバシーを表現）
- `PrivateMethod.key` は `PrivateName { name }` 構造。`#` プレフィックスを除去してメソッド名として使用
- 既存の `convert_class_method` は `ClassMethod` を受け取るが、`PrivateMethod` も同様のフィールド（`function`, `is_static`, `is_abstract` 等）を持つ。共通の変換ロジックを抽出するか、`PrivateMethod` 用の変換関数を追加
- 生成される `Method` の `vis` を `Visibility::Private`（既存）に設定

**PrivateProp** (`ClassMember::PrivateProp`):
- `#field` → 非 `pub` フィールドとして変換
- `PrivateProp.key` は `PrivateName { name }`。`#` プレフィックスを除去
- 既存の `convert_class_prop` と同様のロジック（型注釈の変換、デフォルト値の処理）
- 生成される `StructField` の `pub` フラグを `false` に設定

**StaticBlock** (`ClassMember::StaticBlock`):
- `static { ... }` → `fn _init_static()` メソッドとして変換
- 本体の各文を `convert_stmt` で変換し、`Method` として生成
- `has_self: false`, `is_static: true` で生成
- I-243 の init() パターンと同様、呼び出し責務は利用者に委ねる

#### 8. I-247: TODO 記述修正 + bigint 同期

- 調査の結果、`bigint`/`symbol`/`undefined` は既に変換されていることが判明
- TODO から I-247 を削除し、I-241 の bigint→i128 対応で union バリアントも自動的に同期される
- `_ => continue`（L1735）は `TsIntrinsicKeyword` 等の特殊キーワード型のみが到達する。これに対して `UnsupportedSyntaxError` を返すか、`continue`のままにするかはベンチマークでの出現頻度で判断（`TsIntrinsicKeyword` は Hono では未出現）

#### 9. I-248: intersection 内メソッドシグネチャの impl 生成

- `src/pipeline/type_converter.rs` の `try_convert_intersection_type`（L1835-1852）と `convert_intersection_in_annotation`（L2019-2033）の 2 箇所
- `TsTypeElement::TsMethodSignature` を `convert_method_signature`（既存関数、L786）で `Method` に変換
- `try_convert_intersection_type` の返り値を拡張: フィールド(`Vec<StructField>`)に加えてメソッド(`Vec<Method>`)も返す
- 呼び出し元で、メソッドが存在する場合に `Item::Impl` を追加生成する
- `convert_intersection_in_annotation` でも同様: synthetic struct に対する `Item::Impl` を生成
- `convert_method_signature` は既に `TsMethodSignature` → `Method` の変換を実装済みなので、新規コードは最小限

### 設計整合性レビュー

- **高次の整合性**: 変換パイプライン（parser → transformer → generator）の設計方針に沿っている。エラー報告の統一は transformer 全体の一貫性を向上させる。`Expr::RuntimeTypeof` と `Expr::IntLit(i128)` の IR 追加は既存の IR 設計パターン（式の種類ごとにバリアントを追加）に合致
- **DRY / 直交性 / 結合度**: エラー報告の 3 パターン混在を解消することで DRY を改善。PrivateMethod/PrivateProp の変換は既存の Method/ClassProp 変換と共通ロジックを共有する設計とし、重複を避ける
- **割れ窓**: `_ => continue`（union キーワード型 L1735, intersection メソッド L1850）は「暗黙スキップ」パターンの典型。本 PRD で解消する

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/ir.rs` | `Expr::IntLit(i64)` → `Expr::IntLit(i128)`, `Expr::RuntimeTypeof` 追加 |
| `src/transformer/mod.rs` | エラー報告統一ヘルパー追加、トップレベル式文変換(I-243)、declare module エラー伝播(I-245) |
| `src/transformer/expressions/literals.rs` | BigInt パース i128 化(I-241) |
| `src/transformer/expressions/binary.rs` | typeof ランタイムヘルパー生成(I-242) |
| `src/transformer/functions/mod.rs` | rest パラメータ変換(I-244) |
| `src/transformer/classes.rs` | PrivateMethod/StaticBlock/PrivateProp 変換(I-246) |
| `src/pipeline/type_converter.rs` | union bigint→i128 同期(I-247), intersection メソッド変換(I-248) |
| `src/generator/expressions.rs` | `IntLit(i128)` 出力, `RuntimeTypeof` 出力 |
| `src/generator/mod.rs` | `js_typeof` ヘルパー関数の条件付き生成 |
| `TODO` | I-241〜I-248 の削除、I-247 記述修正 |

## タスク一覧

※完了したタスク: T1（エラー統一基盤）、T2（BigInti128）、T3（RuntimeTypeof）、T4（トップレベル init()）、T6（declare moduleエラー伝播）、T7（private class members）

### T1: エラー報告の統一基盤

- **作業内容**: `UnsupportedSyntaxError` に `fn new(kind: impl Into<String>, span: Span) -> Self` ヘルパーを追加（`src/transformer/mod.rs`）。既存の `anyhow!("unsupported ...")` パターンのうち、未対応構文の報告に使われているものを `UnsupportedSyntaxError::new()` に置き換える（変換ロジックのバグを示す `anyhow!` は変更しない）
- **完了条件**: (1) `UnsupportedSyntaxError::new` ヘルパーが存在する (2) 新規の未対応構文報告が全て `UnsupportedSyntaxError` を使用 (3) 既存テストが全て通る
- **依存**: なし

### T2: I-241 BigInt i128 拡張

- **作業内容**: `src/ir.rs` の `Expr::IntLit(i64)` を `Expr::IntLit(i128)` に変更。`src/transformer/expressions/literals.rs:70` を `parse::<i128>()` に変更し、パース失敗時は `UnsupportedSyntaxError` を返す。`src/generator/expressions.rs` の `IntLit` 出力を i128 対応に更新。`src/pipeline/type_converter.rs:1721-1727` の union バリアントを `"I128"` / `i128` に更新
- **完了条件**: (1) i64 範囲内の BigInt は従来通り変換される (2) i64 範囲外・i128 範囲内の BigInt が正しく i128 リテラルとして出力される (3) i128 範囲外の BigInt が `UnsupportedSyntaxError` を返す (4) union 内の `bigint` バリアントが `I128(i128)` として生成される (5) テスト追加・パス
- **依存**: T1

### T3: I-242 ランタイム typeof ヘルパー

- **作業内容**: `src/ir.rs` に `Expr::RuntimeTypeof { operand: Box<Expr> }` を追加。`src/transformer/expressions/binary.rs:168` の `None => Expr::StringLit("object")` を、operand の型に応じて `Expr::RuntimeTypeof` または `UnsupportedSyntaxError` に分岐するよう変更。`src/generator/expressions.rs` で `RuntimeTypeof` → `js_typeof(&operand)` 呼び出しを出力。`src/generator/mod.rs` でファイル内に `RuntimeTypeof` が使われた場合のみ `js_typeof` ヘルパー関数を生成
- **完了条件**: (1) 型未解決時に `"object"` がハードコードされない (2) Any 型の operand に対して `js_typeof()` 呼び出しが生成される (3) `js_typeof` ヘルパー関数が正しく出力される (4) 既存の typeof テスト（型が解決できるケース）が引き続きパス (5) テスト追加・パス
- **依存**: T1

### T4: I-243 トップレベル式文の init() 関数変換

- **作業内容**: `Transformer` にトップレベル式文を蓄積する `top_level_exprs: Vec<Expr>` フィールドを追加。`src/transformer/mod.rs:293-295` でトップレベル式文を変換して蓄積。`transform_module` / `transform_module_collecting` の最後に、蓄積があれば `Item::Fn { name: "init", body: [...], vis: Public, ... }` を生成
- **完了条件**: (1) `console.log("init")` が `pub fn init() { println!("init"); }` に変換される (2) 複数のトップレベル式文が 1 つの `init()` にまとまる (3) トップレベル式文がないファイルでは `init()` が生成されない (4) テスト追加・パス
- **依存**: T1

### T5: I-244 ネスト destructuring rest パラメータ

- **作業内容**: `src/transformer/functions/mod.rs:734` の `Rest(_) => {}` を変換ロジックに置き換え。TypeRegistry から元の構造体の全フィールドを取得し、明示的に destructure されたフィールドを除いた残余フィールドで `{ParentName}Rest` synthetic struct を生成。rest 変数をその struct のインスタンスとして初期化する `Let` 文を生成。型情報が取得できない場合は `UnsupportedSyntaxError` を返す
- **完了条件**: (1) `function f({ a, ...rest }: Config)` で `rest` が `ConfigRest { b, c }` に変換される (2) synthetic struct が `SyntheticTypeRegistry` に登録される (3) 型情報不明時にエラーが報告される（サイレントスキップしない） (4) テスト追加・パス
- **依存**: T1

### T6: I-245 declare module 内エラー伝播

- **作業内容**: `src/transformer/mod.rs:491` の `if let Ok(...)` を `match` + エラー分岐に変更。`resilient` が `true` の場合は warnings ベクタにエラーメッセージを追加して続行。`false` の場合は `?` でエラーを伝播。declare module 内の非 `Stmt::Decl` アイテム（例: `ModuleItem::ModuleDecl`）もエラーとして報告する
- **完了条件**: (1) declare module 内の変換エラーが `--report-unsupported` で報告される (2) 非 resilient モードでエラーが伝播する (3) 変換成功するケースは従来通り動作 (4) テスト追加・パス
- **依存**: T1

### T7: I-246 PrivateMethod / PrivateProp / StaticBlock 変換

- **作業内容**: `src/transformer/classes.rs:124` の `_ => {}` を 3 つの ClassMember バリアントに対する変換ロジックに置き換え。PrivateMethod: `convert_class_method` と共通のロジックで変換（`#` プレフィックス除去、vis=Private）。PrivateProp: `convert_class_prop` と共通のロジックで変換（`#` プレフィックス除去、pub=false）。StaticBlock: 本体を `fn _init_static()` メソッドとして変換。未知の ClassMember バリアントには `UnsupportedSyntaxError` を返す
- **完了条件**: (1) `#method()` が非 pub メソッドとして出力される (2) `#field` が非 pub フィールドとして出力される (3) `static { ... }` が `fn _init_static()` として出力される (4) メソッド名・フィールド名から `#` が除去される (5) テスト追加・パス
- **依存**: T1

### T8: I-248 intersection メソッドシグネチャの impl 生成

- **作業内容**: `src/pipeline/type_converter.rs` の `try_convert_intersection_type`（L1847-1850）で `TsMethodSignature` を `convert_method_signature` で変換し、メソッドリストに収集。返り値型をフィールド + メソッドの組に拡張。呼び出し元で、メソッドが存在する場合に `Item::Impl` を追加。`convert_intersection_in_annotation`（L2019-2021）でも同様に対応
- **完了条件**: (1) `type X = { a: string } & { foo(): void }` で `struct X` と `impl X { fn foo(&self) }` が生成される (2) プロパティのみの intersection は従来通り動作 (3) 型エイリアスと型注釈の両方で動作 (4) テスト追加・パス
- **依存**: T1

### T9: I-247 TODO 記述修正 + 残存 wildcard の処理

- **作業内容**: TODO から I-247 を削除（既に変換済み）。`src/pipeline/type_converter.rs:1735` の `_ => continue` に対して、到達する可能性のあるキーワード型（`TsIntrinsicKeyword` 等）を明示的にリストし、本当に未知のキーワード型にのみ `UnsupportedSyntaxError` を返すよう変更
- **完了条件**: (1) TODO から I-247 が削除されている (2) `_ => continue` が明示的なパターンマッチに置き換えられている (3) 既存テストがパス
- **依存**: T1, T2（bigint の i128 同期）

### T10: ベンチマーク検証・ドキュメント同期

- **作業内容**: `./scripts/hono-bench.sh` を実行し、変更前後のエラーインスタンス数を比較。TODO から I-241〜I-248 を削除。plan.md を最新化
- **完了条件**: (1) ベンチマーク結果が記録されている (2) 新たなサイレント意味変更が導入されていない (3) TODO, plan.md が最新
- **依存**: T1〜T9

## テスト計画

### 単体テスト

各タスクで以下のテストを追加:

- **T2 (BigInt)**: i64 範囲内、i64 超・i128 範囲内、i128 超の 3 パターン。union 内 bigint バリアント
- **T3 (typeof)**: 型解決済み（従来通り）、Any 型（RuntimeTypeof 生成）、Option 型（is_some 分岐）
- **T4 (init)**: 単一式文、複数式文、式文なし
- **T5 (rest)**: 型情報あり、型情報なし（エラー）
- **T6 (declare module)**: 成功ケース、エラーケース（resilient=true/false）
- **T7 (private members)**: PrivateMethod, PrivateProp, StaticBlock 各 1 ケース以上
- **T8 (intersection methods)**: プロパティ+メソッド、プロパティのみ、メソッドのみ
- **T9 (union wildcard)**: 既知キーワード型、未知キーワード型

### E2E テスト（スナップショットテスト）

- BigInt リテラル（i128 範囲）のフィクスチャ追加
- `typeof` ランタイムヘルパーのフィクスチャ追加
- トップレベル式文のフィクスチャ追加
- private class members のフィクスチャ追加
- intersection with methods のフィクスチャ追加

## 完了条件

1. 8 箇所すべてのサイレント意味変更・消失が解消されている（暗黙スキップ 0 件）
2. 変換可能なケース（I-241, I-243, I-244, I-246, I-248）は実際に Rust コードが生成される
3. I-242 は Any 型に対してランタイム typeof ヘルパーが生成され、非 Any 未解決型には `UnsupportedSyntaxError` が返る
4. エラー報告が `UnsupportedSyntaxError` に統一されている（新規コードで `anyhow!("unsupported ...")` パターンを使用していない）
5. `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
6. `cargo fmt --all --check` がパス
7. `cargo test` が全テストパス
8. Hono ベンチマークでリグレッションがない（エラーインスタンス数が増加していない）
