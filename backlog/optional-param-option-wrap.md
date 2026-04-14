# TS optional parameter の Rust `Option<T>` 統一ラップ (I-040)

## Background

TypeScript の `?:` optional parameter は「caller が省略できる、関数内では `undefined` として見える」意味論を持つ。Rust の canonical な表現は `Option<T>` で、`None` が「省略」を、`Some(v)` が「値あり」を表す（type-theory 的に同一対応）。

本プロジェクトの変換器は TS の optional param を以下の経路で処理するが、**optional フラグから `Option<T>` への変換が一貫していない**:

| 経路 | 正常動作 | 症状 |
|------|---------|------|
| 自由関数 / arrow / fn expression (`convert_param`, `src/transformer/functions/params.rs:91-95`) | ✓ | — |
| ビルトイン型 JSON loader (`convert_external_params`, `src/external_types/mod.rs:477-482`) | ✓ | — |
| interface method (`convert_method_signature`, `src/pipeline/type_converter/interfaces.rs:466-509`) | ✗ | `bar(y?: number)` → `fn bar(&self, y: f64)` |
| callable interface (`convert_callable_interface_as_trait`, `interfaces.rs:141-283`) | ✗ | `(y?: number): void` → `fn call_0(&self, y: f64)` |
| class method / constructor (`convert_ident_to_param`, `src/transformer/classes/members.rs:453-469`) | ✗ | `class F { bar(y?: number) }` → `fn bar(&self, y: f64)` |
| 埋込み fn 型 (`convert_fn_type_to_rust`, `src/pipeline/type_converter/utilities.rs:127-152`) | ✗ | `(y?: number) => void` (param position) → `Fn(f64) -> ()` |
| fn 型エイリアス (`try_convert_function_type_alias`, `src/pipeline/type_converter/type_aliases.rs:370-412`) | ✗ | `type F = (y?: number) => void` → `Fn(f64) -> ()` |
| registry MethodSignature (`resolve_param_def`, `src/ts_type_info/resolve/typedef.rs:531-548`) | ✗ | MethodSignature.params.ty が `F64` のまま (TypeResolver の expected-type propagation に波及) |
| type literal method (`resolve_method_info`, `src/ts_type_info/resolve/intersection.rs:506-537`) | ✗ | 匿名 interface method の optional が欠落 |

**合計 7 つの broken 経路**。いずれも同じ論理的問題の複数発現であり、完璧な TypeScript→Rust transpiler の定義に反する。

### 再現例 1: interface method

TS:
```ts
interface Foo {
  bar(x: number, y?: number): number;
}
function runit(f: Foo): number { return f.bar(1); }
```

現在の Rust 出力:
```rust
trait Foo {
    fn bar(&self, x: f64, y: f64) -> f64;  // y は Option<f64> であるべき
}
fn runit(f: &dyn Foo) -> f64 {
    f.bar(1.0)  // E0061: missing argument #2 — 生成された Rust がコンパイル不可
}
```

### 再現例 2: 既存 snapshot に焼き付いた broken 状態

`tests/snapshots/integration_test__callable_interface_async.snap` は以下の状態を assertion している:

```rust
trait AsyncProcessor {
    async fn call_1(&self, data: String, flag: bool) -> f64;  // ← bool (broken)
}
impl AsyncProcessorProcessDataImpl {
    async fn inner(&self, data: String, flag: Option<bool>) -> F64OrString { ... }  // ← Option<bool> (正)
}
```

trait signature と impl inner signature が不整合 (bug-affirming test)。

## Goal

**完了状態**: 本プロジェクトの全 8 経路で、TS `?:` optional parameter が Rust `Option<T>` に一貫してラップされる。

検証可能な基準:

1. 以下の入力 8 種すべてで optional param が `Option<T>` となる:
   - interface method (`interface F { m(y?: number) }`)
   - class method (`class F { m(y?: number) {} }`)
   - class constructor (`class F { constructor(y?: number) {} }`)
   - callable interface (`interface F { (y?: number): void }`)
   - 埋込み fn 型 (param 位置の `(y?: number) => void`)
   - fn 型エイリアス (`type F = (y?: number) => void`)
   - type literal method (`let f: { m(y?: number): void }`)
   - private method (`class F { #m(y?: number) {} }`)
2. 上記 8 種の caller 呼び出しで arg が足りない場合、自動的に `None` が埋められて Rust がコンパイルする
3. `cargo test` の lib/integration/compile/E2E 全 pass
4. clippy 0 warnings / fmt 0 diffs
5. `test_throw_new_error_strips_some_wrap_with_builtins` 等、既存の Step 2 で確立したテストが全て通る
6. `callable-interface-async` 等の bug-affirming snapshot が正しい挙動に更新される

## Scope

### In Scope

- 上記 8 経路すべてで optional → `Option<T>` ラップを実装
- 全経路で使う共通ヘルパー `RustType::wrap_if_optional(self, optional: bool) -> RustType` を `src/ir/types.rs` に新設
- 既存の独立した「if optional { wrap_optional() }」パターンを全てこのヘルパー呼び出しに統一（DRY 違反の解消）
- `resolve_param_def` の doc コメント更新 (has_default のみへの言及を修正)
- 既存 snapshot のうち broken 挙動を assertion しているもの (bug-affirming) を正しい出力に更新
- TypeResolver 側の既存 Step 2 整合性テストが引き続き通ることを検証
- 単体 / integration / E2E テストの新規追加

### Out of Scope

- TS の default param (`x = value`) の挙動変更: 現在既に Option<T> ラップされており本 PRD の対象外
- TS の rest param (`...args: T[]`) の挙動変更: optional とは別カテゴリ
- TS の `strictNullChecks: false` 下での `undefined` 非伝播挙動: PRD Step 2 で `strictNullChecks: true` を extract-tool に固定済み
- `RustType::Fn` への optional/name フィールド追加: 方針 A により不要（Option<T> で意味論が完結）
- Phase A (compile_test skip 解消) および Phase B (RC-11) の作業

## Design

### Technical Approach

#### 共通ヘルパー

`src/ir/types.rs` の `impl RustType` に以下を追加:

```rust
impl RustType {
    /// Wraps the type in `Option<T>` when `optional` is `true`, returns unchanged otherwise.
    ///
    /// Canonical encoding site for TS's `x?: T` optional parameter semantics.
    /// Delegates to [`Self::wrap_optional`] for idempotency (double-wrap prevention).
    ///
    /// Call this at EVERY site that converts a TS callable-like parameter to IR,
    /// so that `Option<T>` consistently represents "caller may omit" across
    /// interface methods, class methods, constructors, callable interfaces,
    /// embedded fn types, fn type aliases, and registry method signatures.
    pub fn wrap_if_optional(self, optional: bool) -> RustType {
        if optional {
            self.wrap_optional()
        } else {
            self
        }
    }
}
```

#### 各経路の修正

各経路で「optional を読む」→「`wrap_if_optional` を呼ぶ」の 2 ステップに統一。

| # | ファイル:行 | 修正 |
|---|------------|------|
| S1 | `src/pipeline/type_converter/interfaces.rs:495-508` (`convert_method_signature`) | `ident.id.optional` を読み `ty.wrap_if_optional(optional)` で包む |
| S2 | `src/pipeline/type_converter/interfaces.rs:179-191` (`convert_callable_interface_as_trait::TsFnParam::Ident`) | 同上 |
| S3 | `src/transformer/classes/members.rs:453-469` (`convert_ident_to_param`) | `ident.id.optional` を読み wrap |
| S4 | `src/pipeline/type_converter/utilities.rs:132-145` (`convert_fn_type_to_rust`) | `TsFnParam::Ident` の optional を wrap して params に追加 |
| S5 | `src/pipeline/type_converter/type_aliases.rs:381-395` (`try_convert_function_type_alias`) | 同上 |
| S6 | `src/ts_type_info/resolve/typedef.rs:535-541` (`resolve_param_def`) | `param.has_default` を `param.optional \|\| param.has_default` に拡張、doc 更新 |
| S7 | `src/ts_type_info/resolve/intersection.rs:510-521` (`resolve_method_info`) | `p.optional` を読み wrap |
| S8 | `src/transformer/functions/params.rs:91-95` (`convert_param`) | 既存の inline ラップを `wrap_if_optional` 呼び出しに置換（DRY 統一） |
| S9 | `src/external_types/mod.rs:477-482` (`convert_external_params`) | 既存の inline ラップを `wrap_if_optional` 呼び出しに置換（DRY 統一） |

S8 / S9 は既に正しく動いているが、全 8 + 外部 JSON loader を「単一のヘルパー呼び出し」で統一することで、将来の drift を構造的に防止する。

### Design Integrity Review

**Higher-level consistency**:
- 8 経路すべてが `RustType::wrap_if_optional` の単一呼び出しに収束 → transpiler 全体で optional 意味論が統一される
- Rust の canonical encoding (`Option<T>`) に寄せるため、生成コードの読者にとって surprise がない
- TypeResolver の expected-type propagation (Step 2 で整備) と自然に整合

**DRY**:
- 修正前: 9 箇所 (正常 3 + 異常 6) で inline に `if optional { .wrap_optional() }` を書いている (P1 DRY violation)
- 修正後: 単一ヘルパー `wrap_if_optional` を 9 箇所で呼び出し → knowledge 重複を排除

**Orthogonality**:
- `wrap_if_optional` は RustType の単一責務 (optional → Option) に閉じる
- 各呼び出し箇所は「AST から optional を読む」責務のみを持ち、型変換自体はヘルパーに委譲
- 凝集度が向上

**Coupling**:
- `ir::types` は pipeline/transformer の両方から既に使用されており、新メソッド追加は結合を増やさない
- 各呼び出し箇所から他モジュールへの依存は不変

**Broken windows (発見した既存問題)**:
- `P1`: DRY 違反（複数箇所で同じラップロジック）→ 本 PRD で解消
- `P2`: `resolve_param_def` doc に「has_default フラグに基づき Option ラップ」と書かれており、修正後は不正確 → 本 PRD で更新
- `P3`: `callable-interface-async` snapshot が bug-affirming → 本 PRD で修正

### Impact Area

**修正対象ファイル**:

- `src/ir/types.rs` — `wrap_if_optional` 追加
- `src/transformer/functions/params.rs` — S8 (DRY 統一)
- `src/transformer/classes/members.rs` — S3
- `src/pipeline/type_converter/interfaces.rs` — S1, S2
- `src/pipeline/type_converter/utilities.rs` — S4
- `src/pipeline/type_converter/type_aliases.rs` — S5
- `src/ts_type_info/resolve/typedef.rs` — S6 (+ doc 更新)
- `src/ts_type_info/resolve/intersection.rs` — S7
- `src/external_types/mod.rs` — S9 (DRY 統一)

**影響を受ける snapshot / テスト (bug-affirming 候補)**:

- `tests/snapshots/integration_test__callable_interface_async.snap` — call_1 の flag
- 他: 実装時に `cargo test` で `.new` snapshot として洗い出し、1 件ずつ correctness 検証

### Semantic Safety Analysis

本 PRD は型解決の挙動を変える (param.ty が `T` → `Option<T>` になるケースが増える)。`.claude/rules/type-fallback-safety.md` の 3-step 分析を実施:

**Step 1: 導入される変換パターン**

- 経路 S1-S7 において、optional param の ty が `T` から `Option<T>` に変わる (S8/S9 は既に Option<T> なので no-op)

**Step 2: 使用サイト分類**

1. **Trait method 宣言 (Item::Trait::methods)**:
   - `fn m(&self, y: f64)` → `fn m(&self, y: Option<f64>)`
   - Caller は `Option<f64>` を渡す必要がある。引数不足は `convert_call_args_inner` の fill-None (既存) が自動補完
   - Rust compile error で検出される変更 → **Safe (compile error or identical behavior)**

2. **Impl method 宣言 (Item::Impl::methods)**:
   - Trait の sig と impl の sig は必ず一致する必要がある → 同時に変わる
   - 既存コード (arrow/fn expression 経由で body を定義) 側は既に Option<T> を使用しているため整合性が**改善**する
   - → **Safe (identical behavior — both sides now consistent)**

3. **RustType::Fn (埋込 fn 型 / fn 型エイリアス)**:
   - `Fn(f64)` → `Fn(Option<f64>)`
   - caller は Option を渡す必要がある。fill-None (既存) が補完
   - → **Safe (compile error or identical behavior)**

4. **Registry MethodSignature.params**:
   - param.ty = `F64` → `Option<F64>`
   - TypeResolver の expected-type propagation により、call site で arg が Option<F64> 期待されるように
   - string literal arg → `Some("..."..to_string())` wrap / 数値 literal → `Some(1.0)` wrap
   - **懸念**: Step 2 で remapped methods (is_remapped_method) の末尾 optional param は expected-type 伝播を skip している。それ以外のユーザー定義メソッドでは伝播が働く
   - ユーザーの意図 = optional 引数を Option として扱う → Some() wrap は正しい挙動
   - → **Safe (identical semantic — caller wraps with Some, callee uses .unwrap_or_default() or match)**

5. **Anonymous type literal method (`let f: { m(y?: number): void }`)**:
   - `resolve_method_info` 経由で Method.params.ty が Option<T> になる
   - 同じ integer of trait method 宣言と同一の分析 → **Safe**

**Step 3: Verdict per pattern**

全パターンで **Safe**。silent semantic change は発生しない（Rust compiler が型不整合を検出する、または現在の broken 状態より正しい動作になる）。

### Bug-affirming test identification procedure

task list の T8 で以下の手順を実施:

1. `cargo test` を実行し、`.snap.new` として保存されるファイルを全列挙
2. 各 `.snap.new` を元 `.snap` と diff
3. 差分の各行に対し、新旧どちらが「TS 入力の意味論的に正しい Rust か」を手動判定
4. 新側が正しければ snapshot 更新 (`cargo insta accept`)、旧側が正しければ production コードのバグ（本 PRD で修正漏れ）として再調査
5. 手動判定の結果を本 PRD の completion notes に記録

## Task List

### T1: `RustType::wrap_if_optional` 追加 + 単体テスト

- **Work**: `src/ir/types.rs` の `impl RustType` に `pub fn wrap_if_optional(self, optional: bool) -> RustType` を追加。内部は `if optional { self.wrap_optional() } else { self }`。doc コメントを PRD の Technical Approach 通りに記述
- **Completion criteria**:
  - 4 件の単体テスト pass:
    - `test_wrap_if_optional_true_wraps`
    - `test_wrap_if_optional_false_passthrough`
    - `test_wrap_if_optional_true_idempotent` (既に Option<T> の場合は二重ラップしない)
    - `test_wrap_if_optional_preserves_inner_type_var` (TypeVar など複雑な inner 型)
  - `cargo test --lib -- wrap_if_optional` pass
- **Depends on**: None

### T2: `convert_method_signature` (interface method) の修正

- **Work**: `src/pipeline/type_converter/interfaces.rs:495-508` の `TsFnParam::Ident` 分岐で `ident.id.optional` を読み、`ty = ty.wrap_if_optional(optional)` でラップ
- **Completion criteria**:
  - 単体テスト 1 件追加: `test_convert_method_signature_optional_param_wraps_in_option`
    - 入力: `interface F { bar(y?: number): void }`
    - 期待: Method.params[0] = `Param { name: "y", ty: Some(Option<F64>) }`
  - 単体テスト 1 件追加: `test_convert_method_signature_required_param_not_wrapped`
- **Depends on**: T1

### T3: `convert_callable_interface_as_trait` の修正

- **Work**: `src/pipeline/type_converter/interfaces.rs:179-191` の `TsFnParam::Ident` 分岐で同様に修正
- **Completion criteria**:
  - 単体テスト 1 件追加: `test_convert_callable_interface_optional_param_wraps`
    - 入力: `interface F { (x: number, y?: number): void }`
    - 期待: call_0 の Method.params[1] = `Param { name: "y", ty: Some(Option<F64>) }`
- **Depends on**: T1

### T4: `convert_ident_to_param` (class method / constructor) の修正

- **Work**: `src/transformer/classes/members.rs:453-469` で `ident.id.optional` を読み wrap
- **Completion criteria**:
  - 単体テスト 1 件追加: `test_convert_class_method_optional_param_wraps`
  - 単体テスト 1 件追加: `test_convert_class_constructor_optional_param_wraps`
- **Depends on**: T1

### T5: `convert_fn_type_to_rust` (埋込 fn 型) の修正

- **Work**: `src/pipeline/type_converter/utilities.rs:132-145` の `TsFnParam::Ident` 分岐で optional を読み、params Vec に push する前に wrap
- **Completion criteria**:
  - 単体テスト 1 件追加: `test_convert_fn_type_optional_param_wraps`
    - 入力: `(x: number, y?: number) => void` (type 位置)
    - 期待: `RustType::Fn { params: [F64, Option<F64>], return_type: () }`
- **Depends on**: T1

### T6: `try_convert_function_type_alias` の修正

- **Work**: `src/pipeline/type_converter/type_aliases.rs:381-395` で同様に修正
- **Completion criteria**:
  - 単体テスト 1 件追加: `test_fn_type_alias_optional_param_wraps`
    - 入力: `type MyFn = (x: number, y?: number) => number`
    - 期待: TypeAlias の ty が `Fn(F64, Option<F64>) -> F64`
- **Depends on**: T1

### T7: `resolve_param_def` + `resolve_method_info` の修正

- **Work**:
  - `src/ts_type_info/resolve/typedef.rs:531-548` の `resolve_param_def` で `param.optional || param.has_default` でラップ (`wrap_if_optional` を使う)
  - doc コメントを「optional / has_default 両方で Option ラップ」に更新
  - `src/ts_type_info/resolve/intersection.rs:506-537` の `resolve_method_info` で `p.optional` を読み wrap
- **Completion criteria**:
  - 単体テスト 2 件追加:
    - `test_resolve_param_def_optional_wraps`
    - `test_resolve_method_info_optional_wraps`
  - doc コメント反映確認
- **Depends on**: T1

### T8: DRY 統一 — `convert_param` と `convert_external_params` を `wrap_if_optional` 経由に

- **Work**:
  - `src/transformer/functions/params.rs:91-95` の inline `if optional { wrap_optional() }` を `wrap_if_optional(optional)` に置換
  - `src/external_types/mod.rs:477-482` で同様に置換
  - 既存テストが引き続き通ることを確認
- **Completion criteria**:
  - 既存テスト全 pass (無変更)
  - 置換サイトの目視確認で inline if 分岐が消えている
- **Depends on**: T1

### T9: 既存 snapshot の bug-affirming 分析と更新

- **Work**: Semantic Safety Analysis の手順に従い:
  1. `cargo test` で `.snap.new` 列挙
  2. 各 diff を手動レビューして「新出力 = 正しい意味論」か判定
  3. 正しければ `cargo insta accept`、間違いなら production バグとして T1-T8 に戻る
  4. 判定結果を PRD completion notes に記録
- **Completion criteria**:
  - 全 snapshot が更新済み
  - PRD の末尾に「更新した snapshot 一覧と判定理由」を記録
- **Depends on**: T2, T3, T4, T5, T6, T7, T8

### T10: 統合テスト追加 (compile-level 保証)

- **Work**: `tests/integration_test.rs` に以下を追加:
  - `test_interface_method_optional_param_compiles`: TS interface + optional method, caller は 1 引数 / 2 引数両方 → Rust コンパイル成功を assertion (出力に `Option<f64>` が含まれ、caller は `f.bar(1.0, None)` / `f.bar(1.0, Some(2.0))`)
  - `test_class_method_optional_param_compiles`: TS class method + optional
  - `test_fn_type_alias_optional_param_compiles`: TS type alias + optional param
- **Completion criteria**:
  - 3 件の integration テスト pass
- **Depends on**: T2, T3, T4, T5, T6, T7, T8

### T11: E2E テスト拡張

- **Work**: `tests/e2e/scripts/` に新規 script `optional_params.ts` を追加:
  - interface method の optional param を呼び出し (arg 有 / 無)
  - class method の optional param を呼び出し
  - fn type alias の optional param を呼び出し
  - 各呼び出しで TS tsx と Rust runtime の stdout が一致することを検証
- **Completion criteria**:
  - `test_e2e_optional_params` pass
  - `tests/e2e_test.rs` に関数追加
- **Depends on**: T2, T3, T4, T5, T6, T7, T8

### T12: compile_test skip 状態の確認

- **Work**: `tests/compile_test.rs` の skip list に optional param 関連が残っていないか確認。もし `callable-interface-async` など本 PRD で正しくなる fixture が skip されている場合、除去
- **Completion criteria**:
  - skip list のレビュー完了 (comment に記載、追加の skip 解除があれば適用)
- **Depends on**: T9

### T13: 最終品質ゲート

- **Work**: `cargo test` 全 pass, `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings, `cargo fmt --all --check` 0 diffs
- **Completion criteria**: すべて clean
- **Depends on**: T10, T11, T12

## Test Plan

| テスト種別 | 対象 | 内容 |
|-----------|------|------|
| Unit | `wrap_if_optional` | optional true/false, 既に Option, TypeVar |
| Unit | `convert_method_signature` | optional / required / mixed |
| Unit | `convert_callable_interface_as_trait` | callable with optional |
| Unit | `convert_ident_to_param` | class method / constructor with optional |
| Unit | `convert_fn_type_to_rust` | embedded fn type with optional |
| Unit | `try_convert_function_type_alias` | fn type alias with optional |
| Unit | `resolve_param_def` | optional / has_default / both / neither |
| Unit | `resolve_method_info` | anonymous type literal method with optional |
| Integration | compile 成功 | 3 経路で Rust 出力がコンパイルする |
| E2E | runtime 動作 | tsx と Rust 出力の stdout 一致 |
| Snapshot | 全 fixture | bug-affirming snapshot の修正 |

**Boundary 分析**:
- 0 optional param (全 required): 現状維持 (snapshot 無変化)
- 1 optional param (末尾): Option<T> にラップ
- 複数 optional param: すべて Option<T>
- required + optional mix: required は unchanged, optional のみ wrap

**Partition 分析**:
- Context: {interface, class method, ctor, callable, embedded fn, fn type alias, type literal, free fn, external}
- Optional: {true, false}
- Has default: {true, false}
全組み合わせを Unit テストでカバー

## Completion Criteria

- [ ] `RustType::wrap_if_optional` 追加され全 8 + 2 経路から呼び出されている (9 箇所)
- [ ] optional param を含む TS 入力 (interface / class / ctor / callable / embedded fn / fn type alias / type literal method) がすべて `Option<T>` で Rust に変換される
- [ ] 新規 unit テスト (10+ 件) / integration テスト (3 件) / E2E テスト (1 script) すべて pass
- [ ] 既存 snapshot で bug-affirming だったものは正しい出力に更新され、判定理由が PRD notes に記録済み
- [ ] `cargo test` 全 pass (lib / integration / compile / E2E)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
- [ ] `cargo fmt --all --check` 0 diffs
- [ ] `resolve_param_def` の doc コメントが修正後の挙動を正しく説明している
- [ ] plan.md の「次のタスク」セクションから本 PRD 参照を削除 (完了時の /backlog-management 処理)
- [ ] TODO から I-040 を削除 (完了項目の削除ルール)

**Impact 検証 (3 代表 instance のコードパス追跡)**:

1. `interface Foo { bar(y?: number) }` + `f.bar(1)`:
   - `convert_method_signature`:466 で optional を読み wrap → Method.params = [Option<F64>]
   - `convert_call_args_inner`:791 で Option param に対して arg 不足時に None fill
   - 生成: `f.bar(None)` → Rust compile ✓

2. `type F = (y?: number) => void` + `f(1)`:
   - `try_convert_function_type_alias`:381 で optional を読み wrap → RustType::Fn params = [Option<F64>]
   - TypeResolver の `set_call_arg_expected_types`:164-205 の Ident 分岐で RustType::Fn の params を伝播
   - convert_call_args_inner が Option に対して None fill
   - 生成: `f(None)` → Rust compile ✓

3. `class Foo { bar(y?: number) {} }` + `new Foo().bar()`:
   - `convert_ident_to_param`:453 で optional を読み wrap → Method.params = [Option<F64>]
   - 同様に fill-None
   - 生成: `foo.bar(None)` → Rust compile ✓

3 つの代表 instance すべてで修正点が実行パス上の failure point を解消することを確認。

## Notes

- PRD 開始時点の Hono bench: clean 71.5% (113/158) / errors 59。本 PRD 完了後の変動は完了時に追記
- 本 PRD は Step 2 (RC-2) の徹底レビュー中に発見された pre-existing defect。Step 2 の scope には含めず、独立 PRD として起票
