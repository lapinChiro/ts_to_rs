# I-268 + I-269: ジェネリクス型パラメータ・Optional スプレッドのフィールド展開

## 背景

OBJECT_LITERAL_NO_TYPE エラー 48 件のうち、18 件（I-268: 14件、I-269: 4件）がスプレッドソースのフィールド展開失敗に起因する。

### 現状の `merge_object_fields` の制約

`src/pipeline/type_resolver/expected_types.rs:49-86` の `merge_object_fields` はスプレッドソースの型が `RustType::Named` かつ TypeRegistry に `TypeDef::Struct` として登録されている場合のみフィールドを展開する。以下のケースで `None` を返し、expected type 未設定となる:

1. **I-268**: スプレッドソースが型パラメータ — `{ ...env }` で `env: E`（`E extends Env`）の場合、`registry.get("E")` が `None` → フィールド展開不可
2. **I-269**: スプレッドソースが Optional — `{ ...options }` で `options?: CORSOptions` の場合、型が `RustType::Option(Named("CORSOptions"))` → `Named` ではないため展開不可

### 影響パターン

**I-268 のパターン**:
```typescript
function serveStatic<E extends Env>(options: ServeStaticOptions<E>) {
  return baseServeStatic({ ...options, getContent })(c, next)
  // ↑ options の型は ServeStaticOptions<E> だが、E が型パラメータのため
  //   フィールド展開時に E のフィールド情報を取得できない
}
```

**I-269 のパターン**:
```typescript
function cors(options?: CORSOptions) {
  const defaults = { origin: "*", ...options }
  // ↑ options の型は Option<CORSOptions> だが、Option は Named struct ではないため
  //   フィールド展開できない
}
```

## 完了基準

1. TypeResolver が関数/クラスの型パラメータ制約を追跡し、スプレッド展開時に制約型からフィールドを取得する
2. `merge_object_fields` が `RustType::Option(inner)` を unwrap して inner のフィールドを展開する
3. `merge_object_fields` が `RustType::Named` の type_args に型パラメータが含まれる場合、制約型でインスタンス化してフィールドを展開する
4. 既存テスト全通過、clippy 0 警告、fmt pass
5. Hono ベンチマークで OBJECT_LITERAL_NO_TYPE の削減を検証

## スコープ

### スコープ内

- `merge_object_fields` の拡張（Option unwrap + 型パラメータ制約解決）
- TypeResolver への型パラメータ制約追跡の追加
- `convert_object_lit`（Transformer 側）のスプレッド展開における同等の対応

### スコープ外

- 型パラメータの高度な推論（条件型、マップ型等）
- `new obj.Class()` パターン（I-278）
- `MethodSignature` の `has_rest` 追加（I-276）

## 設計

### 技術アプローチ

#### 1. TypeResolver に型パラメータ制約マップを追加

`TypeResolver` に `type_param_constraints: HashMap<String, RustType>` を追加する。ジェネリック関数/クラスの解決開始時に型パラメータの制約を登録し、終了時にクリアする。

```rust
// mod.rs
pub struct TypeResolver<'a> {
    // ... existing fields ...
    /// 現在のスコープで有効な型パラメータ制約。
    /// `E extends Env` → {"E": Named("Env")}
    type_param_constraints: HashMap<String, RustType>,
}
```

登録タイミング:
- `resolve_arrow_expr` / `resolve_fn_expr`: 関数の `type_params` から制約を抽出
- `visit_class_body`: クラスの `type_params` から制約を抽出

#### 2. `merge_object_fields` の拡張

スプレッドソースの型解決を以下の順序で試行する:

```rust
fn resolve_spread_source_fields(&self, spread_ty: &RustType) -> Option<&[(String, RustType)]> {
    match spread_ty {
        // 1. Option<T> → unwrap して T のフィールドを取得
        RustType::Option(inner) => self.resolve_spread_source_fields(inner),

        // 2. Named type → registry lookup（既存ロジック）
        RustType::Named { name, type_args } => {
            // 2a. 直接 registry に存在する場合
            if let Some(TypeDef::Struct { fields, .. }) = self.registry.get(name) {
                return Some(fields);
            }
            // 2b. 型パラメータの場合 → 制約型のフィールドを使用
            if let Some(constraint) = self.type_param_constraints.get(name) {
                return self.resolve_spread_source_fields(constraint);
            }
            None
        }
        _ => None,
    }
}
```

#### 3. Transformer 側（`convert_object_lit`）の対応

`data_literals.rs` の `convert_object_lit` でも同様に:
- `struct_fields` の取得時に型パラメータ制約を考慮
- `Option` 型のスプレッドソースを unwrap

ただし Transformer 側は TypeResolver の結果（`expected_types`）を使うため、TypeResolver 側の修正で大半のケースが解決される。Transformer 側は `struct_fields` の取得ロジック（`reg.get(struct_name)` の呼び出し部分）の拡張が必要。

### 設計整合性レビュー

- **上位レベル一貫性**: TypeResolver のスコープスタックに型パラメータ制約を追加する設計は、既存の `scope_stack`（変数スコープ）、`current_fn_return_type`（関数コンテキスト）と同じパターン。パイプラインの依存方向（TypeResolver → Transformer）を維持
- **DRY**: `resolve_spread_source_fields` ヘルパーを抽出し、`merge_object_fields` と Transformer 側の両方で再利用。Option unwrap と型パラメータ解決のロジックを一箇所に集約
- **結合度**: TypeResolver に `type_param_constraints` を追加するが、これは TypeResolver 内部でのみ使用。外部 API への影響なし
- **Broken window**: `merge_object_fields` が `type_args` を無視している既存問題（`RustType::Named { name, .. }` の `..` で type_args を捨てている）。本 PRD のスコープ内で、type_args を使った `registry.instantiate` 呼び出しに修正する

### 影響範囲

- `src/pipeline/type_resolver/mod.rs` — `TypeResolver` struct に `type_param_constraints` フィールド追加
- `src/pipeline/type_resolver/expected_types.rs` — `merge_object_fields` 拡張、`resolve_spread_source_fields` ヘルパー追加
- `src/pipeline/type_resolver/expressions.rs` — `resolve_arrow_expr`、`resolve_fn_expr` で型パラメータ制約登録
- `src/pipeline/type_resolver/visitors.rs` — `visit_class_body` で型パラメータ制約登録（クラスの場合）
- `src/transformer/expressions/data_literals.rs` — `convert_object_lit` の `struct_fields` 取得ロジック拡張

## タスク

### T1: TypeResolver に型パラメータ制約マップを追加

- **作業**: `TypeResolver` struct に `type_param_constraints: HashMap<String, RustType>` フィールドを追加。`new()` で空マップ初期化。`resolve_arrow_expr` / `resolve_fn_expr` で関数の `ast::TsTypeParamDecl` から制約を `convert_ts_type` で変換し登録。関数スコープ終了時（`leave_scope` 直前）にクリア
- **完了基準**: ジェネリック関数内で `type_param_constraints` に制約が登録されることをユニットテストで検証。制約なし（`<T>`）の場合は登録されないことも検証
- **依存**: なし

### T2: `merge_object_fields` の Option unwrap 対応（I-269）

- **作業**: `merge_object_fields` 内のスプレッドソース型解決で `RustType::Option(inner)` を検出した場合、`inner` のフィールドを展開するロジックを追加。再帰的に解決する `resolve_spread_source_fields` ヘルパーメソッドを `expected_types.rs` に追加
- **RED**: `{ origin: "*", ...options }` で `options: Option<CORSOptions>` のテスト追加。`CORSOptions` のフィールドが expected type に含まれることをアサート
- **GREEN**: `resolve_spread_source_fields` で `Option` unwrap を実装
- **完了基準**: テスト通過。`merge_object_fields` が `Option<Named("X")>` をスプレッドソースとしてフィールド展開できる
- **依存**: なし

### T3: `merge_object_fields` の型パラメータ制約解決（I-268）

- **作業**: `resolve_spread_source_fields` に型パラメータ制約解決ロジックを追加。`RustType::Named { name, .. }` で `registry.get(name)` が `None` の場合、`self.type_param_constraints.get(name)` を確認し、制約型のフィールドを展開
- **RED**: `function f<E extends Env>(env: E) { return { ...env, extra: 1 } }` のテスト追加。`Env` のフィールドが expected type に含まれることをアサート
- **GREEN**: `resolve_spread_source_fields` に型パラメータ制約参照を実装
- **完了基準**: テスト通過。型パラメータがスプレッドソースの場合、制約型のフィールドが展開される
- **依存**: T1, T2（`resolve_spread_source_fields` ヘルパーに追加）

### T4: `merge_object_fields` の type_args によるインスタンス化

- **作業**: `resolve_spread_source_fields` で `RustType::Named { name, type_args }` の `type_args` が空でない場合、`registry.instantiate(name, type_args)` を呼び出してインスタンス化した TypeDef からフィールドを取得するように修正。既存の `registry.get(name)` フォールバックを維持
- **RED**: `Container<String>` のスプレッドで、フィールド型が `T` ではなく `String` にインスタンス化されることをテスト
- **GREEN**: `resolve_spread_source_fields` で `type_args` が空でない場合に `instantiate` を呼び出す
- **完了基準**: テスト通過。ジェネリック型のスプレッドでフィールド型が正しくインスタンス化される
- **依存**: T2（`resolve_spread_source_fields` ヘルパーに追加）

### T5: Transformer 側の対応

- **作業**: `src/transformer/expressions/data_literals.rs` の `convert_object_lit` で `struct_fields` を取得する箇所（`self.reg().get(struct_name)` → `TypeDef::Struct { fields, .. }`）を拡張。TypeResolver 側で expected type が設定されていれば Transformer は既にそれを使用するため、Transformer 側は expected type が未設定のフォールバックケースのみ対応
- **完了基準**: Transformer 側でも型パラメータ制約・Option unwrap を考慮したスプレッド展開ができる。既存テスト全通過
- **依存**: T2, T3, T4

### T6: 検証

- **作業**: 全テスト通過、clippy 0 警告、fmt pass、ファイル行数チェック。Hono ベンチマークで OBJECT_LITERAL_NO_TYPE の削減を定量検証
- **完了基準**: 既存テスト全通過。OBJECT_LITERAL_NO_TYPE が 48 件から削減されていること
- **依存**: T1-T5

## テスト計画

### ユニットテスト（TypeResolver）

- **型パラメータ制約登録**: ジェネリック関数内で `type_param_constraints` に制約が登録される
- **Option unwrap スプレッド**: `{ ...opt }` で `opt: Option<Struct>` → フィールド展開成功
- **型パラメータスプレッド**: `{ ...env }` で `env: E extends Env` → `Env` のフィールド展開
- **ジェネリック型インスタンス化**: `{ ...container }` で `container: Container<String>` → `T` が `String` に置換
- **制約なし型パラメータ**: `{ ...t }` で `t: T`（制約なし）→ フィールド展開不可（`None`）
- **ネスト**: `Option<E>` where `E extends Env` → Option unwrap + 制約解決の組み合わせ

### 統合テスト

- E2E テストは不要（expected type の内部改善であり、生成コードの構造は変わらない）
- スナップショットテストに影響がある場合は更新

## 完了基準

1. 全ユニットテスト通過（上記テスト計画の全項目）
2. 既存テスト全通過
3. clippy 0 警告、fmt pass、ファイル行数 1000 行以下
4. Hono ベンチマークで OBJECT_LITERAL_NO_TYPE 削減を検証（目標: 48 → 34 以下）
5. I-268（14件）と I-269（4件）の対象パターンが解消されていること
