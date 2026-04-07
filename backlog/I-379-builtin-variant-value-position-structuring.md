# I-379: Builtin variant value-position 参照の構造化 (`Expr::Ident("None")` 撲滅)

## Background

I-378 で `Expr::Ident("Color::Red")` 等の display-formatted 修飾パス文字列を `Expr::EnumVariant` / `Expr::PrimitiveAssocConst` / `Expr::StdConst` に構造化し、`CallTarget` を 7 variant (`Free` / `BuiltinVariant` / `ExternalPath` / `UserAssocFn` / `UserTupleCtor` / `UserEnumVariantCtor` / `Super`) に分解した。これにより `Some(x)` / `Ok(v)` / `Err(e)` 等の **payload 付き** builtin variant 構築は `Expr::FnCall { target: CallTarget::BuiltinVariant(_), args }` として構造的に表現される。

しかし **payload なしの builtin variant 値式参照 `None`** は依然として `Expr::Ident("None".to_string())` で encode されており、I-378 が撲滅した broken window と同種の pipeline-integrity 違反が残存している。本 PRD はこれを解消する。

### 実測した現存サイト (生産コード 5 件 + テスト 6 件)

| # | Location | 現在の encoding | 真の意味 |
|---|---|---|---|
| 1 | `src/transformer/expressions/literals.rs:48` | `ast::Lit::Null(_) => Ok(Expr::Ident("None".to_string()))` | TS `null` リテラル → Rust `Option::None` 値 |
| 2 | `src/transformer/expressions/mod.rs:58` | `Expr::Ident("None".to_string())` (Option lowering early-return) | `null` / `undefined` を Option コンテキストで受けた場合の `None` |
| 3 | `src/transformer/expressions/mod.rs:95` | `"undefined" => Ok(Expr::Ident("None".to_string()))` | TS `undefined` 識別子 → Rust `Option::None` 値 |
| 4 | `src/transformer/expressions/calls.rs:715` | `result.push(Expr::Ident("None".to_string()))` | rest params 不足分の埋め |
| 5 | `src/transformer/expressions/data_literals.rs:330` | `fields.push((field_name.clone(), Expr::Ident("None".to_string())))` | 省略された Option フィールドの auto-fill |

加えてテスト 6 件で同形式が assertion されており、I-379 で同時に追従が必要。

### Root cause

`Expr::Ident(String)` が「単一識別子」「value 参照」「**builtin variant constructor (引数なし)**」の 3 つの意味を多重表現している。`None` という識別子は偶然 Rust の有効な expression として動作するため、generator 出力は正しいが、IR レベルの type-level 区別がない。これにより:

- walker は `Expr::Ident("None")` を user type 参照と区別する手段がない (現状は `is_external` フィルタが事後的に除外)
- "undefined", "null", "auto-fill" の 3 つの異なる構築経路が同じ encoding を共有しており、責務分離されていない
- 将来 `Some(x)` の値式参照 (`let f = Some;` のような関数値) を扱う際、`Expr::Ident("Some")` と `Expr::FnCall { CallTarget::BuiltinVariant(Some), [] }` の不一致がさらに悪化する

I-378 と同型の broken window: 「IR に display-formatted 文字列を保存禁止」(`pipeline-integrity.md`) の根本原則違反。

## Goal

完了時点で以下が **構造的に成立**:

1. `Expr::Ident("None")` がプロダクションコードに 0 ヒット (`grep -rn 'Expr::Ident("None"' src/ --include='*.rs' --exclude='*tests*'` で確認)
2. TS `null` / `undefined` および Option auto-fill / rest param fill 経由の `None` 値式は構造化された [`Expr::BuiltinVariantValue`] (新 variant) で表現される
3. `Expr::Ident::name == "None"` の文字列比較がプロダクションコードから消滅 (`mod.rs:73` の matches! を含む)
4. walker は値式の `None` を構造的に「builtin variant value (登録不要)」と区別する
5. Hono ベンチマーク後退ゼロ、`./scripts/check-file-lines.sh` パス、`cargo test` 全 pass

## Scope

### In Scope

- `Expr` への 1 新 variant 追加: `Expr::BuiltinVariantValue(BuiltinVariant)` (payload なしの builtin variant 値式参照)
- 上記 5 構築サイトの新 variant 置換
- `mod.rs:73` の文字列比較 `matches!(&inner_result, Expr::Ident(name) if name == "None")` を構造的マッチに置換
- `IrVisitor::walk_expr` / `IrFolder::walk_expr` への新 variant 分岐追加 (リーフとして処理、`visit_user_type_ref` 不発火)
- generator `generate_expr` の新 variant rendering (`BuiltinVariant::None.as_rust_str()` → `"None"`)
- `Expr::is_trivially_pure` / `is_copy_literal` の新 variant 対応 (`true` / `true`、`None` は f64 と同じく Copy)
- walker (`TypeRefCollector::visit_expr`) の新 variant 認識 (no-op 確認)
- `test_fixtures::all_exprs` への新 fixture 追加
- 全関連テストの新形式追従 (production 5 件 + tests 6 件)
- 新規追加テスト (rendering / 構築 / 不変条件 / 走査)

### Out of Scope

- `Some(x)` / `Ok(v)` / `Err(e)` の value-position 参照 (例: `let f = Some;`) — TS では極めて稀で実例なし。`None` 専用変換で十分
- `Item::StructInit::name: String` の display-formatted `"Enum::Variant"` 形式 — 別 broken window (TODO `[broken-window:StructInit::name]`)、独立 PRD
- Pattern 側の `Pattern::UnitStruct { path: vec!["None"] }` — I-380 (Pattern 構造化 + walker 完全 IrVisitor 化) のスコープ
- `PATTERN_LANG_BUILTINS` ハードコード — I-380 のスコープ

## Design

### Technical Approach

#### 1. `Expr::BuiltinVariantValue` variant 追加

`src/ir/expr.rs::Expr` に追加:

```rust
pub enum Expr {
    // ... 既存 36 variant ...

    /// payload なしの builtin variant 値式参照。例: `None`。
    ///
    /// payload 付きの builtin variant 構築 (`Some(x)` / `Ok(v)` / `Err(e)`) は
    /// `Expr::FnCall { target: CallTarget::BuiltinVariant(_), args }` を使う。
    /// 本 variant は **値リテラルとしての** builtin variant 参照を構造化する。
    ///
    /// 現状の構築サイト: TS `null` / `undefined` / Option auto-fill / rest param
    /// 不足分の埋めはすべて `BuiltinVariant::None` を生成する。`Some` / `Ok` / `Err`
    /// の値式参照 (関数として渡す等) は TS で実例がないため未対応。
    BuiltinVariantValue(BuiltinVariant),
}
```

「`Some` / `Ok` / `Err` の値式参照は実例がないなら variant を 1 つに絞れ」という議論もあるが (`OptionNoneLit` 専用 variant)、`BuiltinVariantValue(BuiltinVariant)` の方が:
- 既存 `BuiltinVariant` 型を再利用 (DRY)
- 将来的な拡張 (`let f = Some;`) に対応可能
- generator/walker の match arm が `_ => unreachable!()` ではなく構造的に網羅可能

`is_trivially_pure: true` (定数参照、副作用ゼロ)、`is_copy_literal: true` (`None: Option<T>` は `T: Copy` のとき Copy だが、`None` 自体の構築は Copy 値であり eager 評価安全)。

#### 2. `IrVisitor` / `IrFolder` 拡張

`walk_expr` の新 variant arm: `Expr::BuiltinVariantValue(_) => {}` (リーフ、user type ref 不発火)。`fold.rs` の `walk_expr` も対称に `e @ Expr::BuiltinVariantValue(_) => e` で恒等折返し。

#### 3. generator

`generate_expr` の新 variant: `Expr::BuiltinVariantValue(v) => v.as_rust_str().to_string()`。`BuiltinVariant::None.as_rust_str()` は `"None"` を返すため、出力は I-379 前後で byte-for-byte 同一 (semantically safe)。

#### 4. Transformer 構築サイト書き換え

| Site | 旧 | 新 |
|---|---|---|
| `literals.rs:48` | `Expr::Ident("None".to_string())` | `Expr::BuiltinVariantValue(BuiltinVariant::None)` |
| `mod.rs:58` | 同上 | 同上 |
| `mod.rs:95` | 同上 | 同上 |
| `calls.rs:715` | 同上 | 同上 |
| `data_literals.rs:330` | 同上 | 同上 |
| `mod.rs:73` 構造マッチ | `matches!(&inner_result, Expr::Ident(name) if name == "None")` | `matches!(&inner_result, Expr::BuiltinVariantValue(BuiltinVariant::None))` |

#### 5. walker

`TypeRefCollector::visit_expr` は `walk_expr` のデフォルトに委ねる。新 variant は user type ref を持たないため何もしない (構造的に保証)。`Expr::Ident("None")` の特殊扱いコードがあれば削除 (現状なし)。

### Design Integrity Review

`.claude/rules/design-integrity.md` チェック:

- **Higher-level consistency**: parser → transformer → generator パイプライン整合性に合致。Transformer が意味論を判定し IR に構造化、Generator が rendering に専念
- **DRY**: `BuiltinVariant` 型を再利用 (新型導入なし)。`as_rust_str()` 呼び出しを 1 箇所に集約
- **Orthogonality**: `BuiltinVariantValue` は単一意味論「payload なし builtin variant 値式参照」を担う。`FnCall { CallTarget::BuiltinVariant }` (構築) と直交分離
- **Coupling**: 依存方向は `walker → IrVisitor → expr.rs (BuiltinVariant)`。新規循環なし
- **Broken windows 検出**:
  - `Pattern::UnitStruct { path: vec!["None"] }` — Pattern 側の同種 broken window。I-380 で対応
  - `Item::StructInit::name: String` の `"Enum::Variant"` — 別クラス、別 PRD

### Impact Area

**変更ファイル**:
- `src/ir/expr.rs` — `Expr::BuiltinVariantValue` variant 追加 + `is_trivially_pure` / `is_copy_literal` 拡張 + 単体テスト
- `src/ir/mod.rs` — (`pub use` 追加不要、既に `BuiltinVariant` を export 済)
- `src/ir/visit.rs` — `walk_expr` リーフ追加
- `src/ir/visit_tests.rs` — TagRecorder + variant test set 拡張
- `src/ir/fold.rs` — `walk_expr` リーフ追加
- `src/ir/test_fixtures.rs::all_exprs` — fixture 追加
- `src/generator/expressions/mod.rs` — `generate_expr` 新 variant arm
- `src/generator/expressions/tests.rs` — rendering テスト追加
- `src/pipeline/external_struct_generator/mod.rs` (`TypeRefCollector::visit_expr` の StructInit case の隣に新 variant の no-op を明示するか、デフォルト walk に委ねる)
- `src/transformer/expressions/literals.rs` — Lit::Null サイト
- `src/transformer/expressions/mod.rs` — Option early-return + undefined + 構造マッチ
- `src/transformer/expressions/calls.rs` — rest params fill サイト
- `src/transformer/expressions/data_literals.rs` — auto-fill サイト
- `src/transformer/expressions/tests/literals.rs` / `optional_semantics.rs` / `objects.rs` / `calls/rest_params_tests.rs` — テスト追従

### Semantic Safety Analysis

| 変更 | 旧 generator 出力 | 新 generator 出力 | 意味論差 |
|---|---|---|---|
| `Expr::Ident("None")` → `BuiltinVariantValue(None)` | `None` | `None` | **完全一致** (Safe) |
| `is_trivially_pure: true` (新 variant) | (旧) `Expr::Ident(_) => true` | `BuiltinVariantValue(_) => true` | 既存挙動維持 (Safe) |
| `is_copy_literal: true` (新 variant) | (旧) `Expr::Ident(_) => false` (Ident は copy_literal でなかった) | `BuiltinVariantValue(_) => true` | **byte-diff 可能性**: Option default が `unwrap_or_else(\|\| None)` → `unwrap_or(None)` に変化 |

**Verdict**: silent semantic change なし。byte-diff は idiomatic 改善方向。Hono ベンチで具体的影響を確認 (T0 で grep)。

## Task List

### T0: baseline + expected diff の事前特定

- **Work**: `./scripts/hono-bench.sh` 実行 → 158 fixture の生成 Rust を `/tmp/hono-baseline-i379/` に退避。`grep -rn "unwrap_or_else(|| None)" /tmp/hono-baseline-i379/` で `is_copy_literal: true` 化による expected byte-diff サイトを列挙
- **Completion criteria**: baseline 取得完了、expected diff サイトリストが文書化される

### T1: `Expr::BuiltinVariantValue` variant 追加

- **Work**: `src/ir/expr.rs` `enum Expr` に `BuiltinVariantValue(BuiltinVariant)` 追加。`is_trivially_pure: true` / `is_copy_literal: true` 拡張。新 variant の単体テスト追加
- **Completion criteria**: `cargo check` pass。purity test pass

### T2: visit/fold 拡張 + test_fixtures 追加

- **Work**: `walk_expr` 両方にリーフ arm 追加。`visit_tests.rs` の `TagRecorder` + variant 網羅テストに新タグ追加。`test_fixtures::all_exprs` に fixture 追加
- **Completion criteria**: walker_visits_every_expr_variant / identity_folder_preserves_all_expr_variants が新 variant を含めて pass

### T3: generator rendering + テスト

- **Work**: `generate_expr` に新 variant arm 追加。rendering 単体テスト追加
- **Completion criteria**: `BuiltinVariantValue(None)` → `"None"` 確認

### T4: Transformer 5 構築サイト書き換え

- **Work**: 5 production サイトを新 variant に置換。`mod.rs:73` の文字列マッチを構造マッチに置換
- **Completion criteria**: `cargo check` pass。`grep -rn 'Expr::Ident("None"' src/ --include='*.rs'` がプロダクションコード 0 ヒット

### T5: テスト追従

- **Work**: 6 テストファイルの assertion を新形式に置換 (`literals.rs` / `optional_semantics.rs` / `objects.rs` / `calls/rest_params_tests.rs`)
- **Completion criteria**: `cargo test --lib` 全 pass

### T6: walker / pattern walker 追加検証

- **Work**: `TypeRefCollector` が新 variant を user type として登録しないことを walker_tests で確認
- **Completion criteria**: 新規 walker test 追加 + pass

### T7: Hono ベンチ + quality-check

- **Work**: `./scripts/hono-bench.sh` 実行 → 後退ゼロ確認。T0 baseline と diff 比較 → expected `unwrap_or_else(|| None)` → `unwrap_or(None)` 系のみであることを確認。`cargo fix` → `cargo fmt` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo test`
- **Completion criteria**: クリーン 114/158 維持、エラー 54 維持、dir 157/158 維持、警告 0、テスト pass、行数閾値内

### T8: TODO + plan.md 更新

- **Work**: `[broken-window:Lit::Null]` を TODO から削除 (5 サイト全消去)。`plan.md` の Batch 11c-fix-2-d-2 (本 PRD) を完了マーク
- **Completion criteria**: ドキュメント更新完了

## Test Plan

### T1 単体 (新規)

- `Expr::BuiltinVariantValue(BuiltinVariant::None).is_trivially_pure() == true`
- `Expr::BuiltinVariantValue(BuiltinVariant::None).is_copy_literal() == true`
- 4 BuiltinVariant 全てで構築可能 (Some/None/Ok/Err) — 将来拡張のための網羅性テスト

### T3 単体 (新規)

- `generate_expr(&Expr::BuiltinVariantValue(BuiltinVariant::None))` → `"None"`
- 4 variant の rendering 確認 (Some → `"Some"`, etc.)

### T4 統合 (新規)

- TS `const x: number | null = null;` → `let x: Option<f64> = None;` (構造的に検証)
- TS `const x = undefined;` → `let x = None;`
- TS `function f(x: number, y: number = 0) {}; f(1)` → rest param fill で `None` が構造化される
- discriminated union object literal で省略フィールドの auto-fill が新 variant を生成

### T6 walker 単体 (新規)

- `Expr::BuiltinVariantValue(_)` を含む item を walker に通したとき、refs に "None" / "Some" / "Ok" / "Err" が登録されないこと

### T7 byte-diff 検証

- T0 baseline と byte 比較。expected diff: `unwrap_or_else(|| None)` → `unwrap_or(None)` 系のみ
- それ以外の diff は意味論的等価性を 3 ケース手動検証

## Completion Criteria

- [ ] `grep -rn 'Expr::Ident("None"' src/ --include='*.rs' --exclude='*tests*'` プロダクションコード 0 ヒット
- [ ] `grep -rn 'name == "None"' src/` プロダクション 0 ヒット (構造マッチに置換済)
- [ ] `cargo test` 全 pass
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 警告 0
- [ ] `cargo fmt --all --check` pass
- [ ] `./scripts/check-file-lines.sh` pass
- [ ] `./scripts/hono-bench.sh` 後退なし (114/158 / 157/158 / err 54)
- [ ] T0 で取得した expected diff のみが byte-diff として観測される
- [ ] `TODO` から `[broken-window:Lit::Null]` セクションが削除されている
- [ ] `plan.md` の本バッチが完了マーク

## References

- I-378 PRD: `Batch 11c-fix-2-d` の `Expr::Path` 構造化 (本 PRD と同型の broken window 撲滅)
- I-378 PRD-DEVIATION D-1: `is_trivially_pure` / `is_copy_literal` の意味論 (本 PRD でも同方針)
- `.claude/rules/pipeline-integrity.md`: IR に display-formatted 文字列を保存禁止
- `.claude/rules/conversion-correctness-priority.md`: silent semantic change > compile error > unsupported
