# I-386 (PRD-A-2): `resolve_type_ref` Step 3 (unknown error 化) と silent fallback テスト群の根絶

## Background

PRD-A (`I-383`) Discovery 段階では、`resolve_type_ref` の 3 階層判定 (型パラメータ / user 定義型 / unknown=error) を 1 PRD で完遂する想定だった。しかし PRD-A の T6 (Step 3 = unknown error 化) を実装した時点で、**73 件の既存テストが失敗** することが判明した。

事前検証 (`report/i382/probe-raw.log`) では Hono 158 fixture で Cluster 1b (20) + 1c (1) = 21 件の dangling のみが観測されていたが、unit test 群では更に多数の「TypeRegistry に未登録の型を `resolve_type_ref` に渡し、`RustType::Named` への silent fallback を期待する」コードパスが存在することが、PRD-A 実装中に発見された。

### 検証エビデンス (疑いようのない証拠)

| Evidence | 内容 |
|---|---|
| **E1** | T6 (Step 3) 未適用で `cargo test --lib` → **2223 passed, 0 failed** (baseline 健全) |
| **E2** | T6 (Step 3) のみ単独適用で → **2150 passed, 73 failed** |
| **E3** | failure log に `unknown type ref: <X>` 文字列が **36 回** 出現 (`anyhow::anyhow!("unknown type ref: {name}")` から発生) |
| **E4** | failure 出現 unknown identifiers (19 種類): `A`, `Active`, `Config`, `Container`, `Context`, `Counter`, `Foo`, `I`, `MyStruct`, `MyType`, `Options`, `Pair`, `Point`, `Request`, `Response`, `Success`, `T`, `Unknown`, `Widget` — すべて test fixture 由来の identifier |
| **E5 (決定的)** | `pipeline/type_converter/tests/collections.rs:42-59` の `test_convert_ts_type_named_with_type_args` が、空 `TypeRegistry::new()` に対して `convert_ts_type("Container<string>")` を呼び、戻り値 `RustType::Named { name: "Container", type_args: [String] }` を `assert_eq!` している = bug-affirming silent fallback assertion の動かぬ証拠 |
| **E6** | `transformer/expressions/tests/objects.rs:68` の `test_convert_expr_object_literal_empty` が `const e: Empty = {};` を変換、`Empty` 型は registry に未登録 = 同上 |

### Root Cause

既存実装と既存テストが体系的に「TypeRegistry に未登録の type ref を `RustType::Named` に silent fallback する」挙動に依存している。この silent fallback は `conversion-correctness-priority.md` の **Tier 1 (silent semantic change)** リスクを温存しており、CLAUDE.md「最も理想的でクリーンな実装」原則に重大に違反している。

更に、`pipeline-integrity.md` および `testing.md` の bug-affirming test 禁忌に該当するテスト多数の存在が立証された。これらは「不正な silent fallback を assert する」テストで、テスト自身が production の broken window を保護する形で機能していた。

## Goal

`resolve_type_ref` の Step 3 (unknown identifier に対する明示的 `Err("unknown type ref: <name>")`) を恒久的に有効化し、既存の silent fallback 依存テスト・production コードを **すべて根絶** する。完了時点で:

1. `resolve_type_ref` の default branch は `(型パラメータ scope / user 定義型 / unknown=error)` の 3 階層判定を完備に行う
2. unknown branch から `Err` が返ることを前提とした test fixture 群が、TypeRegistry への適切な事前登録 (または期待値の `Err` 化) を行う
3. PRD-A 完了後に再計測した failure 件数が **0** になる
4. `cargo test --lib` 全 pass
5. probe で Cluster 1b (20) + Cluster 1c (1) の dangling が **0 件**

## Scope

### In Scope

1. **`resolve_type_ref` Step 3 の再適用**: PRD-A の T6 で revert された Step 3 (`Err(anyhow::anyhow!("unknown type ref: {name}"))`) を恒久的に有効化
2. **PRD-A 完了後の failing test 再計測** (件数の確定): PRD-A の T7-T9 完了で B カテゴリ 16 件は自動解消する見込みなので、再計測値を正式 scope とする
3. **Test fixture 群の更新** (PRD-A 完了後の残存 failure 対応):
   - **A カテゴリ** (type_converter unit test 23 件): 各テストの fixture で参照型を `TypeRegistry::register(...)` 経由で事前登録
   - **C カテゴリ** (registry tests 10 件): 同上
   - **D カテゴリ** (transformer expressions/functions/classes tests 21 件): 同上 + `Empty` 等の type annotation を持つテストの fixture 更新
   - **E カテゴリ** (intentional silent fallback test 3 件): assert を `Err` 期待に変更
4. **Production コードの真の root cause 修正**: テスト fixture 更新では解決しない箇所 (resolve エラーを上位で握り潰している実装) を特定し、エラー伝播を明示化
5. **Promise 系の組み込み化**: PRD-A の T6 検証で発見した「Promise / PromiseLike が組み込み扱いされていない」漏れの修正
6. **Hono ベンチ影響の受け入れ**: lib.dom 型 (Cluster 1b) を含む union/struct 全体が変換 error として明示計上される。`bench-history.jsonl` に新 entry 追加し、error 数値の増加を「silent semantic change の可視化」として正式記録
7. **新規 lock-in テスト**: Step 3 の動作を保証するテスト (空 registry に未登録型 → `Err`) を `mod_tests.rs` に追加

### Out of Scope

- `generate_stub_structs` 関数自体の削除 (= I-382 本体 = PRD-B のスコープ)
- synthetic → user 型 import 生成 (= PRD-B のスコープ)
- lib.dom 型を `web_sys` 連携で実装可能な型に変換するロジック
- TypeScript lib.\*.d.ts のパース・自動取り込み
- 新 `RustType` variant の追加
- PRD-A の Step 1 (型パラメータ scope 判定) と Step 2 (user 定義型 branch) — これらは PRD-A で実装

## Design

### Technical Approach

#### 1. PRD-A 完了後の failing test 再計測 (Phase 1)

PRD-A (I-383) の T1-T9 + T10 (`__type` 経路調査) 完了後、Step 3 を再適用して `cargo test --lib` を走らせ、失敗テストを再計測する。当初 73 件のうち、PRD-A の T7-T9 で type_param scope が補完されることで B カテゴリ 16 件は解消する見込み。

再計測値が PRD-A-2 の正式 scope となる。

#### 2. Test fixture の対応方針

##### 2a. type_converter / transformer 系 (A + C + D カテゴリ)

各テストの fixture で参照される型を `TypeRegistry::register(name, TypeDef::Struct { ... })` 等で事前登録する。例:

**Before** (`pipeline/type_converter/tests/collections.rs:42-59`):
```rust
let ty = convert_ts_type(
    &prop.type_ann.as_ref().unwrap().type_ann,
    &mut SyntheticTypeRegistry::new(),
    &TypeRegistry::new(),  // 空 registry
)
.unwrap();
assert_eq!(
    ty,
    RustType::Named { name: "Container".to_string(), type_args: vec![RustType::String] }
);
```

**After**:
```rust
let mut reg = TypeRegistry::new();
reg.register(
    "Container".to_string(),
    TypeDef::new_struct(vec![], Default::default(), vec![TypeParam {
        name: "T".to_string(),
        constraint: None,
    }]),
);
let ty = convert_ts_type(
    &prop.type_ann.as_ref().unwrap().type_ann,
    &mut SyntheticTypeRegistry::new(),
    &reg,
)
.unwrap();
assert_eq!(
    ty,
    RustType::Named { name: "Container".to_string(), type_args: vec![RustType::String] }
);
```

これによりテストの意図 (「Container<string> が Named { name: 'Container', type_args: [String] } になる」) が保たれつつ、silent fallback 依存が消える。

##### 2b. 共通 fixture builder の検討

`pipeline/type_converter/tests/` 全体で同じパターンの fixture が並ぶため、共通ヘルパー `fn registry_with_type(name: &str, type_params: Vec<TypeParam>) -> TypeRegistry` の追加を検討する。ただし YAGNI 原則に従い、3 件以上の重複が確認できた場合のみ抽出する。

##### 2c. intentional silent fallback test (E カテゴリ)

3 件の以下のテストは「silent fallback を意図的に assert」しているため、テストの意図そのものを再評価する:

- `ts_type_info::resolve::intersection::tests::unresolvable_typeref_becomes_embed_field`
- `ts_type_info::resolve::tests::resolve_user_defined_type` (要詳細確認)
- `ts_type_info::resolve::utility::tests::test_resolve_inner_fields_with_conversion_not_found`

各々の意図:
- **`unresolvable_typeref_becomes_embed_field`**: 「未解決の typeref を embedded field として処理する」現状実装の挙動を assert。Step 3 で error 化されると、この embedded field 化 path が機能不全になる
  - 対応案: 「embedded field 化」が本当に必要な機能なら、その path を `resolve_type_ref` の Step 3 より前に分岐させる (新 API)。それ以外なら、現状実装と test を両方削除し、error 期待の新 test に置換
- **`test_resolve_inner_fields_with_conversion_not_found`**: utility (Partial/Pick/Omit 等) の対象型が registry に未登録のときの挙動を assert
  - 対応案: utility の意味論として、対象型未登録は variants 形式の error 化が正しい
- **`resolve_user_defined_type`**: テスト名から未登録 user 定義型を直接解決するテスト
  - 対応案: テスト fixture に事前登録を追加

各々個別判断を下し、PRD-A-2 の Task List で具体化する。

##### 2d. Production コードの真の root cause 修正

`unresolvable_typeref_becomes_embed_field` のように production 側で意図的に silent fallback を呼んでいる箇所がある場合、その path 自体が `conversion-correctness-priority.md` Tier 1 リスクなので、production 側の修正が必要。

該当箇所: `src/ts_type_info/resolve/intersection.rs` の embedded field 化ロジック (詳細は PRD-A 完了後に grep で再確認)

#### 3. Step 3 再適用

PRD-A の T6 で revert された 3 階層化を再適用:

```rust
_ => {
    let expected_count = reg.get(name).map(|td| td.type_params().len());
    match expected_count {
        Some(expected) => {
            let mut args = resolved_args;
            if args.len() > expected {
                args.truncate(expected);
            }
            Ok(RustType::Named {
                name: sanitize_rust_type_name(name),
                type_args: args,
            })
        }
        None => Err(anyhow::anyhow!("unknown type ref: {name}")),
    }
}
```

#### 4. Promise / PromiseLike の組み込み化

```rust
"Promise" | "PromiseLike" => Ok(RustType::Named {
    name: "Promise".to_string(),
    type_args: resolved_args,
}),
```

これは PRD-A の T6 検証で発見した silent fall-through バグの修正。

#### 5. Hono ベンチ影響の制御

PRD-A-2 完了で Hono ベンチの `error_instances` が増加する想定 (Cluster 1b の 20 件 + 関連する union/struct 全体が変換 error として明示計上される)。これは:

- silent semantic change を可視化する **正の変化**
- `bench-history.jsonl` に新 entry を追加し、増加が「PRD-A-2 起因」「expected」と明示
- 後続 PRD (PRD-B = I-382 本体) では Cluster 1b/1c は恒久的にエラー扱い

### Design Integrity Review

`prd-design-review.md` 3 観点での自己レビュー:

#### 凝集度

- `resolve_type_ref` の Step 3 は単一責務 (unknown branch を error 終端する) で、Step 1 / Step 2 と同一抽象レベル
- Promise/PromiseLike の組み込み化は単一責務 (lib builtin マッピング)
- ✅ 凝集度問題なし

#### 責務分離

- 「未知判定」(`resolve_type_ref`) と「stub 生成」(`external_struct_generator`) が完全分離される
- テスト fixture の事前登録は「テストの setup 責務」を明示化 (現状の暗黙 silent fallback よりも明示的)
- ✅ 責務分離問題なし

#### DRY

- 共通 fixture builder ヘルパーの抽出を検討 (2b)
- Step 3 の error message format (`unknown type ref: {name}`) は単一箇所
- ✅ DRY 問題なし

#### 結合度

- Step 3 再適用でテスト ↔ production の結合度は **減少** する (silent fallback の暗黙的依存が消える)
- ✅ 結合度問題なし

### Impact Area

- `src/ts_type_info/resolve/mod.rs` (Step 3 再適用 + Promise/PromiseLike 組み込み化)
- `src/pipeline/type_converter/tests/{collections,interfaces,intersections,structural_transforms,type_aliases,unions}.rs` (A カテゴリ test fixture 更新)
- `src/registry/tests/{generics,interfaces,functions}.rs` (C カテゴリ test fixture 更新)
- `src/transformer/expressions/tests/objects.rs` (D カテゴリ)
- `src/transformer/functions/tests/{destructuring,fn_decl,params}.rs` (D カテゴリ)
- `src/transformer/classes/tests/param_prop.rs` (D カテゴリ)
- `src/transformer/statements/tests/expected_types.rs` (D カテゴリ)
- `src/transformer/tests/classes.rs` (D カテゴリ)
- `src/ts_type_info/resolve/intersection.rs` (E カテゴリ — production の真の root cause 修正)
- `src/ts_type_info/resolve/utility.rs` (E カテゴリ)
- `src/ts_type_info/resolve/mod.rs` (E カテゴリ + 新 lock-in test)
- `src/ts_type_info/resolve/mod_tests.rs` (`resolve_user_defined_type` の修正 + Step 3 lock-in test 追加)

### Semantic Safety Analysis

本 PRD は `resolve_type_ref` の type fallback を **狭める** (現状の silent Named 化を error 化) ため、`type-fallback-safety.md` の 3 ステップ分析を適用:

**Step 1: 型 fallback パターン**

| パターン | PRD-A 完了時点 | PRD-A-2 完了時点 |
|---|---|---|
| 型パラメータ参照 (`M`) | scope ありで `Named { type_args: vec![] }` | 同 (変更なし) |
| user 定義型参照 (`HTTPException`) | TypeRegistry にあれば Named | 同 (変更なし) |
| lib.dom 型 (`BufferSource`) | silent Named fall-through | **`Err("unknown type ref: BufferSource")`** |
| `__type` | 同 | **同 Err** |
| 真の未知 | 同 | **同 Err** |

**Step 2: 各 usage site の分類**

- 既存テスト fixture が「未登録型を Named 化する silent fallback」を assert していたのは **bug-affirming test** で、それ自身が Tier 1 リスクの保護装置。Step 3 適用で破壊することは「silent semantic change の可視化」であり、type-fallback-safety **完全準拠**
- production の embedded field 化 path (`unresolvable_typeref_becomes_embed_field` の対象実装) は同じく Tier 1 リスクの温存。Step 3 適用で発覚した path を明示エラー化することは Safe 改善

**Step 3: Verdict**

- 全パターン: **Safe** (silent → error は Tier 1 リスクの解消で、安全側への変化)
- UNSAFE パターン: なし

## Task List

**Note**: 本 PRD のタスク数値は PRD-A 完了前の想定値で、PRD-A 完了後に T0 で再計測した上で確定する。

### T0: PRD-A 完了確認 + 再計測 (本 PRD 開始の prerequisite)

- **Work**:
  - PRD-A (I-383) の T1-T11 が完了していることを確認
  - PRD-A の `cargo test --lib` 全 pass を確認
  - `resolve_type_ref` の Step 3 を T6 と同じ形 (`Err(anyhow::anyhow!("unknown type ref: {name}"))`) で再適用
  - `cargo test --lib 2>&1 | tee /tmp/i386-baseline.log` を実行
  - failures 件数を集計し、本 PRD の正式 scope (件数) を確定
  - PRD-A-2 マスタープランの「再計測値」セクションを更新
- **Completion criteria**:
  - 再計測値が確定 (期待: 73 - 16 (B カテゴリの T7-T9 解消分) = 57 件前後)
  - PRD-A-2 内の各 task list が再計測値で更新されている
  - Step 3 を一旦 revert し baseline (cargo test 全 pass) に戻す
- **Depends on**: PRD-A (I-383) 全 task

### T1: A カテゴリ test fixture 更新 (type_converter unit test)

- **Work**:
  - `src/pipeline/type_converter/tests/{collections,interfaces,intersections,structural_transforms,type_aliases,unions}.rs` の対象テストを開く
  - 各テストの fixture で `TypeRegistry::new()` に対して、参照される型を `register` で事前登録するように変更
  - 同パターンが 3 件以上で見つかる場合は共通ヘルパー `fn registry_with_type(name: &str, type_params: Vec<TypeParam>) -> TypeRegistry` を抽出
- **Completion criteria**:
  - A カテゴリの 23 件 (再計測値) すべて pass
  - 既存の他テスト全 pass
- **Depends on**: T0

### T2: C カテゴリ test fixture 更新 (registry tests)

- **Work**:
  - `src/registry/tests/{generics,interfaces,functions}.rs` の対象テストを開く
  - dependency 型を `register` で事前登録
- **Completion criteria**:
  - C カテゴリの 10 件 (再計測値) すべて pass
- **Depends on**: T0 (T1 と並列可能)

### T3: D カテゴリ test fixture 更新 (transformer tests)

- **Work**:
  - `transformer/expressions/tests/objects.rs` (`Empty`, `Wrapper` 等の事前登録)
  - `transformer/functions/tests/{destructuring,fn_decl,params}.rs`
  - `transformer/classes/tests/param_prop.rs`
  - `transformer/statements/tests/expected_types.rs`
  - `transformer/tests/classes.rs`
  - `TctxFixture::from_source` 系 helper が型を auto 登録するモードを持つかどうか確認し、必要なら拡張
- **Completion criteria**:
  - D カテゴリの 21 件 (再計測値) すべて pass
- **Depends on**: T0 (T1/T2 と並列可能)

### T4: E カテゴリ — intentional silent fallback test の根本対応

- **Work**:
  - `ts_type_info::resolve::intersection::tests::unresolvable_typeref_becomes_embed_field` を精査
    - production 側 (`intersection.rs` の embedded field 化ロジック) が silent fallback に依存しているか確認
    - 依存している場合: production 側のロジックを「`Err` を上位に伝播する」形に変更し、テストも同期して `Err` 期待に書き換え
    - 依存していない場合: テスト fixture に事前登録
  - `ts_type_info::resolve::utility::tests::test_resolve_inner_fields_with_conversion_not_found` を精査・対応
  - `ts_type_info::resolve::tests::resolve_user_defined_type` を精査・対応
- **Completion criteria**:
  - 3 件すべて pass
  - production の真の root cause (silent fallback を意図的に呼ぶ箇所) が修正されている
- **Depends on**: T0

### T5: `resolve_type_ref` Step 3 + Promise 組み込み化の恒久適用

- **Work**:
  - PRD-A の T6 で revert された Step 3 (`Err(anyhow::anyhow!("unknown type ref: {name}"))`) を再適用
  - Promise / PromiseLike を組み込みリストに追加
  - 新規 lock-in test `test_resolve_type_ref_returns_error_for_unknown_name` を `mod_tests.rs` に追加
- **Completion criteria**:
  - 新 lock-in test pass
  - `cargo test --lib` 全 pass (T1-T4 で fixture 更新済みのため)
- **Depends on**: T1, T2, T3, T4

### T6: probe 再投入 + Hono 全件検証

- **Work**:
  - probe instrumentation を `external_struct_generator::generate_stub_structs` に再投入
  - `cargo build --release && /home/kyohei/ts_to_rs/target/release/ts_to_rs /tmp/hono-clean -o /tmp/hono-bench-output 2>/tmp/i386-probe.log`
  - probe ログから dangling refs を集計し、Cluster 1b (20) + 1c (1) = 21 件が **0 件** であることを確認
  - probe instrumentation を撤去
- **Completion criteria**:
  - probe ログで Cluster 1b/1c の dangling 0 件
  - probe instrumentation がコードから撤去されている
- **Depends on**: T5

### T7: Hono ベンチ実行 + bench-history 更新

- **Work**:
  - `./scripts/hono-bench.sh` を実行
  - `bench-history.jsonl` に新 entry を追加
  - 増加した error の内訳を `report/i382/bench-impact-i386.md` に記録
- **Completion criteria**:
  - 新 entry が追加され、PRD-A-2 起因の error 増加が記録されている
- **Depends on**: T6

### T8: /quality-check + master-plan 更新

- **Work**:
  - `cargo fix --allow-dirty --allow-staged`
  - `cargo fmt --all --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
  - `report/i382/master-plan.md` の進捗表で T2.A2 を `done` に更新
- **Completion criteria**:
  - 0 errors / 0 warnings / 全 test pass
- **Depends on**: T7

## Test Plan

### 新規テスト

1. `test_resolve_type_ref_returns_error_for_unknown_name` (`mod_tests.rs`): 空 `TypeRegistry::new()` に対して `resolve_type_ref("CompletelyUnknown", &[], &reg, &mut synthetic)` が `Err` を返すことを assert (regression lock-in)
2. `test_resolve_type_ref_resolves_promise_with_args`: `Promise<T>` が `Named { name: "Promise", type_args: [Named("T")] }` を返すことを assert (組み込み化の検証)
3. T1-T4 で更新されたテストは「silent fallback assertion」から「事前登録された型を Named 化」に意図が変わり、bug-affirming → 正当な assertion になる

### Test Coverage Review (Impact Area)

#### Production Code Quality Issues

| # | Location | Category | Severity | Action |
|---|---|---|---|---|
| P1 | `ts_type_info/resolve/mod.rs:429-442` (PRD-A 後の状態) | silent fallback (Tier 1 リスク) | **High** | T5 で Step 3 再適用 |
| P2 | `ts_type_info/resolve/intersection.rs` (該当行 PRD-A 後に再特定) | 意図的な silent fallback (embedded field 化) | High | T4 で根本修正 |
| P3 | `ts_type_info/resolve/utility.rs` (該当行) | utility 対象型未登録時の silent fallback | High | T4 で対応 |
| P4 | `ts_type_info/resolve/mod.rs` Promise 組み込み漏れ | builtin マッピング不完全 | Medium | T5 で追加 |

#### Test Coverage Gaps

| # | Missing Pattern | Technique | Severity | Action |
|---|---|---|---|---|
| G1 | unknown type ref が `Err` を返すこと (Step 3 lock-in) | C1 branch | **High** | T5 で追加 |
| G2 | Promise / PromiseLike の組み込み変換 | 等価分割 | Medium | T5 で追加 |
| G3 | 既存 73 件の bug-affirming test の意図再評価 | 質的レビュー | High | T1-T4 で個別対応 |

## Completion Criteria

1. T0-T8 のすべての completion criteria を満たす
2. `resolve_type_ref` の default branch に Step 3 (`Err("unknown type ref: ...")`) が恒久的に存在する
3. `cargo test --lib` 全 pass (再計測値のすべての failure 0)
4. probe で Cluster 1b (20) + 1c (1) = **21 件 0 件**
5. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
6. `cargo fmt --all --check` 0 diff
7. `bench-history.jsonl` に新 entry 追加
8. `report/i382/master-plan.md` の進捗表で T2.A2 が `done`
9. **`generate_stub_structs` 関数は本 PRD では削除しない** (= PRD-B のスコープ)

### Impact 推定の検証 (3 件 trace)

PRD-A-2 の影響推定値「73 件の bug-affirming test 解消」は、`/tmp/i383-step3-only.log` の実測 + `/tmp/i383-failures.txt` 全 73 件の trace に基づく。3 件の代表的 trace:

1. **`test_convert_ts_type_named_with_type_args` (A カテゴリ)**: 空 registry に `Container<string>` 渡し → Step 3 で `Err("unknown type ref: Container")` → T1 で `Container` を事前登録 → Named 返却 → assertion 通過
2. **`test_member_access_on_type_param_with_constraint` (B カテゴリ)**: `function f<E extends Env>` の `E` が scope 漏れ → PRD-A T7 (関数 generic scope push) で解消 → 本 PRD で対応不要
3. **`unresolvable_typeref_becomes_embed_field` (E カテゴリ)**: production の embedded field 化が silent fallback に依存 → T4 で production + test を同期修正 → `Err` 期待に置換

各 trace で「再計測後 0 件達成」までの execution path が確認可能。
