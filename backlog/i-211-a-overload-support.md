# I-211-a: メソッドオーバーロード対応 + Union 型の合成 enum 変換

## 背景・動機

現在のローダー（`src/external_types.rs`）には 2 つの構造的な制約がある:

1. **メソッドオーバーロード非対応**: `convert_external_typedef`（`:202`）で `method.signatures.first()` のみ取得し、残りのシグネチャを破棄。ES 標準型はオーバーロードが多く（`Array.from()` は 3 シグネチャ等）、I-211-b で ECMAScript 型を追加する前に器を整備する必要がある
2. **Union 型の簡略化**: `convert_union_type`（`:318-320`）で複数メンバーの union を第 1 要素で代表。メソッドの戻り値型・パラメータ型が不正確になる

これらは I-211-b（ECMAScript 標準型追加）の前提条件であり、器を先に整備することで型データ投入時に情報が劣化しない。

## ゴール

1. `TypeDef::Struct` の `methods` が `HashMap<String, Vec<MethodSignature>>` になり、全オーバーロードを保持する
2. `resolve_method_return_type` が引数の数・型に基づいて最適なシグネチャを選択する
3. `convert_union_type` が既存の `SyntheticTypeRegistry::register_union` 基盤を使い、複数メンバー union を合成 enum に変換する（`RustType::Named` として返す）
4. 既存テストが全て通り、ベンチマークでエラー数が増加しない

## スコープ

### 対象

- `TypeDef::Struct.methods` の `Vec<MethodSignature>` 化と全参照箇所の更新
- `external_types.rs` の全シグネチャ保持 + `convert_union_type` の合成 enum 変換
- `load_builtin_types` の戻り値拡張（`SyntheticTypeRegistry` を返す）
- Pipeline での base synthetic types のシード処理
- `resolve_method_return_type` と `lookup_method_params` のオーバーロード解決ロジック

### 対象外

- ECMAScript 標準型の抽出・JSON 追加（I-211-b）
- E2E テスト・ベンチマーク効果測定（I-211-c）

## 設計

### 技術的アプローチ

#### 1. Union 型の合成 enum 変換（`RustType::Union` を IR に追加しない）

既存の union → enum 変換フロー（`pipeline/type_converter.rs:259-387`）:
1. TypeScript の `string | number` を検出
2. `SyntheticTypeRegistry::register_union(&members)` で合成 enum を生成・重複排除
3. `RustType::Named { name: "StringOrF64" }` を返す
4. Generator は `StringOrF64` を名前参照として出力、enum 定義は synthetic items として出力

**`external_types.rs` の `convert_union_type` も同じ基盤を使う**。`RustType::Union` を IR に追加する必要はない:

```rust
// Before: 複数メンバーは第1要素で代表（行318-320）
_ => convert_external_type(non_null[0])

// After: SyntheticTypeRegistry::register_union で合成 enum に変換
_ => {
    let member_types: Vec<RustType> = non_null.iter().map(|m| convert_external_type(m)).collect();
    let enum_name = synthetic.register_union(&member_types);
    RustType::Named { name: enum_name, type_args: vec![] }
}
```

**`load_builtin_types` の戻り値変更**:

```rust
// Before
pub fn load_builtin_types() -> Result<TypeRegistry> {
    load_types_json(BUILTIN_TYPES_JSON)
}

// After
pub fn load_builtin_types() -> Result<(TypeRegistry, SyntheticTypeRegistry)> {
    let mut synthetic = SyntheticTypeRegistry::new();
    let registry = load_types_json(BUILTIN_TYPES_JSON, &mut synthetic)?;
    Ok((registry, synthetic))
}
```

`convert_external_typedef` と `convert_union_type` に `&mut SyntheticTypeRegistry` を引き回す。

**Pipeline での base synthetic types のシード**:

`src/pipeline/mod.rs` で `load_builtin_types` から受け取った `SyntheticTypeRegistry` を、per-file synthetic に merge する:

```rust
// pipeline/mod.rs（行72付近）
// Before
let mut synthetic = SyntheticTypeRegistry::new();

// After
let mut synthetic = base_synthetic.clone();  // base_synthetic は load_builtin_types から
```

または各ファイル処理の最初に `file_synthetic.merge(base_synthetic.clone())` を呼ぶ。

これにより:
- IR に新バリアント追加不要（11+ ファイルの match 変更が不要）
- Generator のフォールバック不要（合成 enum は通常の `Named` として出力される）
- 既存基盤の `register_union`（重複排除、enum 名生成）を再利用
- 暫定策ゼロ

**base synthetic types の全ファイル出力について**: ECMAScript 標準型のメソッドシグネチャに含まれる multi-member non-nullable union は極めて少数（主に `string | RegExp` パラメータ程度）。全ファイルに少数の enum 定義が含まれるオーバーヘッドは無視できる。

#### 2. `MethodSignature` の Vec 化

`src/registry.rs` の変更:

```rust
// TypeDef::Struct（行38）
methods: HashMap<String, Vec<MethodSignature>>,

// new_struct（行72）, new_interface（行87）
methods: HashMap<String, Vec<MethodSignature>>,

// substitute_types（行128-140）: Vec 内の全シグネチャを置換
methods: methods.iter().map(|(name, sigs)| {
    (name.clone(), sigs.iter().map(|sig| MethodSignature {
        params: sig.params.iter().map(|(n, ty)| (n.clone(), ty.substitute(bindings))).collect(),
        return_type: sig.return_type.as_ref().map(|ty| ty.substitute(bindings)),
    }).collect())
}).collect(),
```

全参照箇所の更新:

| ファイル | 行 | 箇所 | 変更内容 |
|---------|-----|------|---------|
| `src/registry.rs` | 38 | `TypeDef::Struct` フィールド | `HashMap<String, MethodSignature>` → `HashMap<String, Vec<MethodSignature>>` |
| `src/registry.rs` | 70-75 | `new_struct` | 引数型変更 |
| `src/registry.rs` | 85-90 | `new_interface` | 引数型変更 |
| `src/registry.rs` | 128-140 | `substitute_types` | Vec 内全要素を置換 |
| `src/registry.rs` | 207-218 | `is_trait_type` | `!methods.is_empty()` は HashMap の empty チェックなので変更不要 |
| `src/registry.rs` | 560-615 | `collect_from_class` | `methods.insert(name, MethodSignature{..})` → `methods.insert(name, vec![MethodSignature{..}])` |
| `src/registry.rs` | 656-695 | `collect_interface_methods` | 戻り値型変更 + `vec![..]` ラップ |
| `src/external_types.rs` | 202-229 | `convert_external_typedef` | 全シグネチャを Vec に収集（後述 T2） |
| `src/pipeline/type_resolver.rs` | 1625-1631 | `lookup_method_params` | Vec 対応（後述 T4） |
| `src/pipeline/type_resolver.rs` | 1633-1653 | `resolve_method_return_type` | Vec 対応（後述 T4） |
| `src/transformer/type_env.rs` | 53-63 | テスト | `vec![MethodSignature{..}]` に変更 |
| `src/transformer/functions/tests.rs` | 1103 | テスト | 同上 |
| `src/transformer/statements/tests.rs` | 2673 | テスト | 同上 |
| `src/transformer/expressions/tests.rs` | 4674, 6558 | テスト | 同上 |
| `src/registry.rs` | 1742, 1774 | テスト | 同上 |

#### 3. オーバーロード解決ロジック

`resolve_method_return_type`（行1633-1653）を拡張:

```rust
fn resolve_method_return_type(
    &self,
    obj_type: &RustType,
    method_name: &str,
    arg_count: usize,
    arg_types: &[Option<&RustType>],  // 解決済み引数型（Unknown は None）
) -> ResolvedType {
    // ... type_def 取得は既存と同じ ...
    match &type_def {
        Some(TypeDef::Struct { methods, .. }) => {
            let sigs = match methods.get(method_name) {
                Some(s) => s,
                None => return ResolvedType::Unknown,
            };
            resolve_overload(sigs, arg_count, arg_types)
        }
        _ => ResolvedType::Unknown,
    }
}
```

`resolve_overload` ヘルパー（5 段階解決）:

```
1. シグネチャが 1 つ → そのまま返す
2. 全シグネチャの戻り値型が同一 → その型を返す（最も多いケース）
3. 引数数で絞り込む → 1 つに絞れたらその戻り値型
4. 引数型の互換性で選択
5. フォールバック: 最初のシグネチャ
```

`lookup_method_params`（行1612-1631）も同様に拡張し、引数数で最適なシグネチャのパラメータ型を返す。

呼び出し元の変更:

| 箇所 | 行 | 変更内容 |
|-----|-----|---------|
| OptChain 内 | 1267 | `opt_call.args.len()` と引数型を渡す |
| 通常 Member call | 1499 | `call.args.len()` と引数型を渡す |

引数型は `call.args` を `resolve_expr` で解決済みの場合は `self.result.expr_types` から取得、未解決の場合は `None` を渡す。

### 設計整合性レビュー

- **高次の整合性**: 既存の union → 合成 enum 変換基盤（`SyntheticTypeRegistry::register_union`）を external types でも使用。新しい IR バリアント追加なし、Generator 変更なし。パイプライン全体で union の扱いが一貫する
- **DRY / 直交性**: union → enum 変換は `register_union` に一元化。オーバーロード解決は `resolve_overload` ヘルパーに集約。`resolve_method_return_type` と `lookup_method_params` が共有する
- **結合度**: `load_builtin_types` の戻り値変更で呼び出し元（`main.rs` の `build_base_registry`）に変更が波及。ただし呼び出し箇所は 1 箇所のみ
- **割れ窓**: `convert_union_type` の「第 1 要素で代表」を完全に解消。`lookup_method_params` も Vec 化に合わせて更新

### 影響範囲

| モジュール | 変更内容 |
|-----------|---------|
| `src/registry.rs` | `methods` フィールドの Vec 化 + `substitute_types` 更新 + テスト |
| `src/external_types.rs` | 全シグネチャ保持 + `convert_union_type` を `register_union` ベースに変更 + `load_builtin_types` 戻り値変更 |
| `src/pipeline/mod.rs` | base synthetic types のシード処理 |
| `src/pipeline/type_resolver.rs` | オーバーロード解決ロジック + シグネチャ拡張 |
| `src/main.rs` | `load_builtin_types` の新しい戻り値への対応 |
| テストファイル 4 件 | `MethodSignature` → `vec![MethodSignature]` |

## タスク一覧

### T1: `MethodSignature` の Vec 化

- **作業内容**:
  - `src/registry.rs:38` の `TypeDef::Struct.methods` を `HashMap<String, Vec<MethodSignature>>` に変更
  - `new_struct`（`:70`）, `new_interface`（`:85`） のシグネチャを変更
  - `substitute_types`（`:107-173`）で Vec 内全シグネチャの型パラメータ置換
  - `collect_from_class`（`:560-615`）で `vec![MethodSignature{..}]` にラップ
  - `collect_interface_methods`（`:656-695`）で戻り値型と insert を Vec 化
  - テストファイルの `MethodSignature` リテラル（`type_env.rs:60`, `functions/tests.rs:1103`, `statements/tests.rs:2673`, `expressions/tests.rs:4674,6558`, `registry.rs:1742,1774`）を `vec![..]` にラップ
  - `lookup_method_params`（`:1612-1631`）と `resolve_method_return_type`（`:1633-1653`）で Vec の先頭要素を使うように更新（T4 で完全なオーバーロード解決に置き換え）
- **完了条件**:
  - `cargo check` が通る
  - 既存テストが全て通る
- **依存**: なし

### T2: `external_types.rs` で全シグネチャ保持 + Union の合成 enum 変換

- **作業内容**:
  - `convert_external_typedef`（`:202-229`）で `method.signatures.first()` → 全シグネチャを `Vec<MethodSignature>` に収集
  - `convert_union_type`（`:302-328`）に `&mut SyntheticTypeRegistry` パラメータを追加。複数メンバーの union を `synthetic.register_union(&member_types)` で合成 enum に変換し、`RustType::Named { name: enum_name }` を返す。`T | null/undefined → Option<T>` パターンは維持
  - `convert_external_type` にも `&mut SyntheticTypeRegistry` を引き回す（`convert_union_type` への到達パスのため）
  - `load_builtin_types` の戻り値を `Result<(TypeRegistry, SyntheticTypeRegistry)>` に変更
  - `load_types_json` に `&mut SyntheticTypeRegistry` パラメータを追加（`--tsconfig` モードでの呼び出しも対応）
  - 既存テスト（`:330-`）を更新し、`convert_union_type` が `RustType::Named`（合成 enum 参照）を返すことを検証するテストを追加
- **完了条件**:
  - テスト: `{"signatures": [sig1, sig2]}` を parse → `Vec<MethodSignature>` に 2 要素格納
  - テスト: `{"kind": "union", "members": [{"kind": "string"}, {"kind": "number"}]}` → `RustType::Named { name: "StringOrF64" }` を返し、synthetic に `StringOrF64` enum が登録される
  - テスト: `{"kind": "union", "members": [{"kind": "string"}, {"kind": "null"}]}` → `RustType::Option(Box::new(RustType::String))`（既存挙動維持）
  - 既存テストが全て通る
- **依存**: T1（`Vec<MethodSignature>`）

### T3: Pipeline の base synthetic シード + `main.rs` 対応

- **作業内容**:
  - `src/main.rs` の `build_base_registry` で `load_builtin_types()` の新しい戻り値 `(TypeRegistry, SyntheticTypeRegistry)` を受け取る
  - `SyntheticTypeRegistry` を pipeline に渡す経路を整備（`transpile` / `transpile_directory` 等の関数シグネチャに追加）
  - `src/pipeline/mod.rs` で per-file synthetic registry を base synthetic から clone して初期化（行72付近: `SyntheticTypeRegistry::new()` → `base_synthetic.clone()`）
- **完了条件**:
  - `cargo check` が通る
  - 既存テストが全て通る
  - `load_builtin_types()` の synthetic に登録された合成 enum が、各ファイルの出力に含まれる
- **依存**: T2（`load_builtin_types` 戻り値変更）

### T4: オーバーロード解決ロジック

- **作業内容**:
  - `resolve_method_return_type`（`:1633-1653`）のシグネチャを `arg_count: usize, arg_types: &[Option<&RustType>]` を追加
  - `resolve_overload` ヘルパー関数を実装（5 段階解決: 単一 → 同一戻り値 → 引数数 → 引数型互換 → フォールバック）
  - `lookup_method_params`（`:1612-1631`）を Vec 対応に更新（引数数でシグネチャ選択）
  - 呼び出し元（`:1267` OptChain, `:1499` Member call）で引数数と引数型を収集して渡す
- **完了条件**:
  - テスト: `[sig_0arg_string, sig_1arg_number]` で引数 0 個 → `String` 戻り値
  - テスト: `[sig_0arg_string, sig_1arg_number]` で引数 1 個 → `Number` 戻り値
  - テスト: 全シグネチャの戻り値型が同一 → 引数に依らずその型を返す
  - テスト: マッチなし → 最初のシグネチャの戻り値型
  - 既存テストが全て通る
- **依存**: T1（Vec 化）, T2（全シグネチャ保持）

## テスト計画

### 単体テスト（新規追加）

- **`external_types.rs`**: 複数シグネチャ JSON の parse テスト。`convert_union_type` が `RustType::Named`（合成 enum 参照）を返すテスト。`T | null` の `Option<T>` 維持テスト。合成 enum の `SyntheticTypeRegistry` への登録確認テスト
- **`registry.rs`**: `Vec<MethodSignature>` の `substitute_types` テスト（ジェネリック型パラメータ置換が全シグネチャに適用されること）
- **`type_resolver.rs`**: オーバーロード解決の 4 パターン（単一、同一戻り値、引数数マッチ、フォールバック）

### 回帰テスト

- 既存の全テストが通ること
- ベンチマークでエラー数が増加しないこと（I-211-a はデータモデル変更のみで、新型データは追加しないため、結果は変わらないはず）

## 完了条件

1. `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
2. `cargo fmt --all --check` が通る
3. `cargo test` が全テスト通過
4. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` が通る
5. `TypeDef::Struct.methods` が `HashMap<String, Vec<MethodSignature>>` になっている
6. `external_types.rs` が全シグネチャを保持している
7. `convert_union_type` が `SyntheticTypeRegistry::register_union` を使い、`RustType::Named` を返す（暫定策なし）
8. `load_builtin_types` が `(TypeRegistry, SyntheticTypeRegistry)` を返し、pipeline で base synthetic がシードされる
9. `resolve_method_return_type` が引数数・型に基づくオーバーロード解決を行う
10. ベンチマークでエラーインスタンス数が増加していない
