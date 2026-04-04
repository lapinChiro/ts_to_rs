# Batch 4e: resolve_typedef エラー伝播 + type_aliases TsTypeInfo 移行

## Background

Batch 4d-C のレビューで、`resolve_typedef` 内に 10 箇所の `filter_map` + `.ok()` パターンが残存していることが判明した。これは Batch 4d-C で `resolve_type_literal_fields` に対して修正したのと同一の根本問題（エラー握りつぶしによるフィールド/パラメータ消失）である。

実証テスト:
```typescript
interface WithKeyof { name: string; keys: keyof typeof console; value: number; }
function test(w: WithKeyof): string { return w.name + w.keys; }
```
出力: `struct WithKeyof { name: String, value: f64 }` — `keys` フィールドが消失。`w.keys` アクセスは生成されるがコンパイルエラー。

また、`type_aliases.rs` の `convert_type_alias_items` 内で SWC AST を直接操作する `convert_method_signature` / `convert_property_signature` が残存している。Batch 4d-C で unions.rs / intersections.rs の同一パターンは移行済みだが、type_aliases.rs は未移行。

調査レポート: `report/post-4dc-review-issues.md`

## Goal

1. `resolve_typedef` 内の全 `.ok()` パターンを `?` に変更し、エラーを伝播する
2. `type_aliases.rs` の TsTypeLit 処理を TsTypeInfo 経由に移行する
3. テストカバレッジのギャップ（TG-1〜TG-10）を解消する
4. 全テストパス、clippy 0 warnings、ベンチマーク 113/158 以上を維持

## Scope

### In Scope

- `resolve_typedef` の全 `.ok()` パターンの `?` 変更（10 箇所）
- `resolve_method_sig` のパラメータ `.ok()` パターンの `?` 変更
- `collection.rs:513` の `resolve_field_def(...).ok()` の修正
- `type_aliases.rs` の `convert_type_alias_items` 内 TsTypeLit 処理の TsTypeInfo 移行
  - `convert_method_signature` → `resolve_method_info` に置換
  - `convert_property_signature` → `resolve_type_literal_fields` に置換
  - call signature / index signature の TsTypeInfo 経由化
- テストカバレッジギャップの解消（TG-1〜TG-10）
- doc comments の更新

### Out of Scope

- `interfaces.rs` の `convert_method_signature` / `convert_property_signature`（interface 変換の TsTypeInfo 移行は別 PRD）
- `transformer/functions/params.rs` の `convert_property_signature`（課題 3 として plan.md に記載、本 PRD 直後に対応）
- `collection.rs` の `resolve_typedef` 呼び出しパターンのリファクタリング（呼び出し元の構造変更は別スコープ）

## Design

### T1: resolve_typedef のエラー伝播（strict 方針）

**方針**: 全ての `filter_map(|x| f(x).ok())` を `map(|x| f(x)).collect::<Result<Vec<_>>>()?` に変更する。

**根拠**:
1. 部分的に正しい TypeDef は部分的に間違っている — downstream（transformer）が TypeDef のフィールド情報を参照するため、フィールド消失は不整合を引き起こす
2. 呼び出し元（`collection.rs`）は全て `if let Ok(resolved)` でハンドリング済み — TypeDef 全体の失敗は安全に処理される
3. `resolve_ts_type` が Err を返すのは稀なケース（`keyof typeof` 未解決、utility 型引数不足等）

**変更対象** (`src/ts_type_info/resolve/typedef.rs`):

| 行 | 現在 | 変更後 |
|----|------|--------|
| L27 | `.and_then(\|c\| resolve_ts_type(...).ok())` | `.map(\|c\| resolve_ts_type(...)).transpose()?` |
| L53 | `filter_map(\|f\| resolve_field_def(...).ok())` | `map(\|f\| resolve_field_def(...)).collect::<Result<_>>()?` |
| L69 | `filter_map(\|sig\| resolve_method_sig(...).ok())` | 同上 |
| L74 | `filter_map(\|sig\| resolve_method_sig(...).ok())` | 同上 |
| L134 | `filter_map(\|f\| resolve_field_def(...).ok())` | 同上 |
| L157 | `filter_map(\|p\| resolve_param_def(...).ok())` | 同上 |
| L179 | `resolve_ts_type(...).ok()?` → filter_map | `resolve_ts_type(...)?` → map |
| L190 | `resolve_ts_type(...).ok()?` → filter_map | 同上 |
| L260 | `filter_map(\|p\| resolve_param_def(...).ok())` | `map(\|p\| resolve_param_def(...)).collect::<Result<_>>()?` |

**`resolve_typedef` (Struct) の methods 処理** (L55-62):
```rust
// 現在: unwrap_or_default() でメソッドシグネチャ解決失敗を無視
let resolved_sigs = sigs.into_iter()
    .map(|sig| resolve_method_sig(sig, reg, synthetic))
    .collect::<anyhow::Result<Vec<_>>>()
    .unwrap_or_default();
```
→ `.unwrap_or_default()` を `?` に変更。

### T2: collection.rs の resolve_field_def 修正

`src/registry/collection.rs:513`:
```rust
// 現在:
resolve_field_def(field_ts, reg, synthetic).ok()
// 変更後:
resolve_field_def(field_ts, reg, synthetic).ok()  // 呼び出し元が filter_map 内
```

この箇所は `collect_class_property_fields` 内の `filter_map` で、class プロパティのフィールド収集時に使用。class プロパティは TypeDef 全体ではなく個別フィールドの収集なので、ここは `filter_map` が適切（TypeDef レベルではなく、個別フィールドの寛容な収集）。

**判断**: この箇所は変更しない。理由: class プロパティの収集は「できるものだけ収集」のセマンティクスが正しい（TypeDef のような「完全な定義」ではない）。

### T3: type_aliases.rs の TsTypeInfo 移行

`src/pipeline/type_converter/type_aliases.rs` の `convert_type_alias_items` 内、L184-295 の TsTypeLit 処理を TsTypeInfo 経由に書き換える。

**変更概要**:

1. **L184**: `TsType::TsTypeLit(lit)` のマッチは型分類として残す（SWC AST の型判別のみ）
2. **L198-243**: Call signature only → TsTypeInfo に変換し、`TsFnSigInfo` から FnType を構築
3. **L246-262**: Methods only → TsTypeInfo に変換し、`resolve_method_info` で Method を構築
4. **L264-295**: Properties + index signature → TsTypeInfo に変換し、`resolve_type_literal_fields` でフィールド構築

**Call signature の TsTypeInfo 経由化**:
`TsCallSignatureDecl` は `convert_to_ts_type_info` で `TsTypeLiteralInfo` の `call_signatures` フィールドに変換される。TsTypeLiteralInfo から `TsFnSigInfo` を取得し、パラメータ/戻り値を `resolve_ts_type` で解決する。

**新設関数**: `resolve_fn_sig_info` は不要 — call signature の処理はここだけで使われるため、inline で処理する。

### Design Integrity Review

- **凝集度**: T1 は resolve/typedef.rs 内の閉じた変更。T3 は type_aliases.rs 内の閉じた変更。それぞれ単一責務
- **DRY**: T3 で `convert_method_signature` / `convert_property_signature` の使用を排除し、resolve 版に統一。DRY 違反の解消
- **結合度**: type_aliases.rs → resolve の依存が増えるが、4d-C と同じく意図的（resolve が型解決の唯一の窓口）
- **上位設計との一致**: Batch 4d-B で確立した「SWC → TsTypeInfo → resolve」パイプラインに type_aliases.rs も統一される

### Semantic Safety Analysis

**T1（エラー伝播変更）**: TypeDef の解決が以前より厳格になる。部分解決で成功していたケースが失敗するようになるが、`collection.rs` の `if let Ok(resolved)` で安全にハンドリングされる。TypeDef が登録されない場合、downstream は「未解決の型」として処理し、変換エラーとして報告される。これは Tier 2（コンパイルエラー）であり、Tier 1（サイレント意味変更）よりも安全。

**T3（TsTypeInfo 移行）**: Batch 4d-C と同一パターン。`resolve_type_literal_fields` は `vis: Some(Visibility::Public)` を返すが、type_aliases.rs で生成する Struct/Trait のフィールドも最終的に pub になるため、挙動は同一。

## Task List

### T1: resolve_typedef のエラー伝播修正

- **Work**:
  - `src/ts_type_info/resolve/typedef.rs`: 全 10 箇所の `.ok()` / `filter_map` を `?` / `map` + `collect::<Result<_>>()?` に変更
  - `resolve_method_sig` (L260) のパラメータ `filter_map` も同様に修正
  - L55-62 の `.unwrap_or_default()` を `?` に変更
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス
- **Depends on**: なし

### T2: resolve_typedef テストカバレッジ強化

- **Work**:
  - `src/ts_type_info/resolve/mod_tests.rs` に以下のテストを追加:
    - `resolve_typedef` の Struct バリアント: フィールド解決失敗時に TypeDef 全体が Err になることを検証（TG-1）
    - `resolve_typedef` の Function バリアント: パラメータ解決失敗時に Err（TG-2）
    - `resolve_typedef` の ConstValue バリアント: フィールド/要素解決失敗時に Err（TG-3）
    - `resolve_field_def` のエラーパス（TG-4）
    - `resolve_param_def` のエラーパス（TG-5）
    - `resolve_method_sig` のパラメータ/戻り値解決失敗（TG-10 相当）
- **Completion criteria**: 新規テスト全パス、既存テスト全パス
- **Depends on**: T1

### T3: type_aliases.rs の TsTypeInfo 移行

- **Work**:
  - `src/pipeline/type_converter/type_aliases.rs`:
    - L198-243 (call signature only): TsTypeInfo 変換、`TsFnSigInfo` からパラメータ/戻り値を `resolve_ts_type` で解決
    - L246-262 (methods only): TsTypeInfo 変換、`resolve_method_info` で Method 構築
    - L264-295 (properties + index): TsTypeInfo 変換、`resolve_type_literal_fields` でフィールド構築。index signature は `resolve_type_literal` に委譲
  - `convert_method_signature` と `convert_property_signature` の使用を排除
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス、`convert_method_signature` が interfaces.rs のみで使用されていること
- **Depends on**: なし（T1 と並行可能）

### T4: type_aliases テストカバレッジ強化

- **Work**:
  - `src/pipeline/type_converter/tests/type_alias_forms.rs` に以下のテストを追加:
    - mixed パターン（methods + properties）のテスト（TG-7）
    - properties-only でメソッドも存在する場合のテスト（TG-8）
    - call signature の複数オーバーロードテスト（TG-6）
- **Completion criteria**: 新規テスト全パス
- **Depends on**: T3

### T5: doc comments 更新 + 品質確認

- **Work**:
  - `typedef.rs` の `resolve_typedef` に doc comment 更新（エラーハンドリング方針の明記）
  - `type_aliases.rs` の `convert_type_alias_items` に doc comment 追加
  - `cargo fix --allow-dirty --allow-staged && cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test > /tmp/test-result.txt 2>&1` → 全パス確認
  - `./scripts/hono-bench.sh` → 113/158 以上維持
  - `./scripts/check-file-lines.sh` → 全ファイル 1000 行以下
- **Completion criteria**: 0 errors, 0 warnings, 全テストパス、ベンチマーク 113/158 以上
- **Depends on**: T1-T4

## Test Plan

- **T1 の正しさ**: T2 のテストで検証（エラーケースで TypeDef 全体が Err になること）
- **T3 の正しさ**: 既存テスト全パス + T4 のテスト追加
- **リファクタリングの安全性**: 既存テスト全パス（出力が変わらないことの保証）
- **ベンチマーク前後比較**: 113/158 から変化なしまたは改善

## Completion Criteria

1. `cargo test` 全テストパス
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
3. `cargo fmt --all --check` パス
4. `resolve_typedef` 内に `.ok()` / `.unwrap_or_default()` が存在しない
5. `resolve_method_sig` 内に `filter_map` + `.ok()` が存在しない
6. `type_aliases.rs` 内で `convert_method_signature` / `convert_property_signature` を使用していない
7. Hono ベンチマーク: 113/158 以上（±0 or 改善）
8. テストカバレッジギャップ TG-1〜TG-10 が全て解消

## コード参照一覧

### 変更対象（resolve 側）
- `src/ts_type_info/resolve/typedef.rs:27,53,55-62,69,74,134,157,179,190,260` — `.ok()` / `.unwrap_or_default()` パターン

### 変更対象（type_converter 側）
- `src/pipeline/type_converter/type_aliases.rs:184-295` — TsTypeLit 処理全体
- `src/pipeline/type_converter/type_aliases.rs:249-252` — `convert_method_signature` 使用
- `src/pipeline/type_converter/type_aliases.rs:267-269` — `convert_property_signature` 使用

### テスト追加対象
- `src/ts_type_info/resolve/mod_tests.rs` — TG-1〜TG-5, TG-10
- `src/pipeline/type_converter/tests/type_alias_forms.rs` — TG-6〜TG-9

### 参照のみ（変更なし）
- `src/registry/collection.rs:513` — `resolve_field_def(...).ok()` は class プロパティ収集のため維持
- `src/pipeline/type_converter/interfaces.rs:379` — `convert_method_signature` は interface 変換で継続使用
