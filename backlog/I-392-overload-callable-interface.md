# I-392: Callable interface の完全な型保持変換

## 改訂履歴

- **2026-04-13 #17 (Phase 9C 完了)**: P9.3 (type substitution) + P9.4 (select_overload Stage 2 修正) 完了。
  - P9.3: `apply_type_substitution` helper 追加。convert_callable_trait_const / resolve_fn_type_info /
    build_delegate_impl に substitution 統合。`callable-interface-generic` fixture 追加
  - P9.4: select_overload Stage 2 削除 (5→4 stage)。void-only multi-overload の arity 選択修正
  - 最終状態: 全テスト pass (lib 2368, integration 96), clippy 0, fmt 0
- **2026-04-13 #16 (Phase 9B 完了)**: P9.2 (resolve_fn_type_info widest 書き換え + INV-6) 完了。
  - `resolve_fn_type_info` に `synthetic` 引数追加、callable interface case で
    `compute_widest_signature` を呼び widest params/return を返す
  - `select_overload(..., 0, &[])` を helpers.rs から完全撤去
  - INV-6 完全達成: `unwrap_promise_and_unit` / `unwrap_promise_type` を
    `RustType::unwrap_promise()` に統一、standalone 関数 2 つ削除
  - 新 unit test: multi overload callable interface の arrow body が widest 型で resolve
  - 最終状態: 全テスト pass (lib 2366), clippy 0, fmt 0
- **2026-04-13 #15 (Phase 9A 完了)**: Phase 9 前提 (dead code 削除) + P9.1 (arity validation) 完了。
  - `return_wrap_ctx` / `spawn_nested_scope_with_wrap` 削除。#[allow(dead_code)] 0 件
  - arity validation (INV-4): `trait_type_args.len() != trait_type_params.len()` で hard error
  - error-case fixture + compile_test skip + integration test
  - INV-8 説明更新 (factory method 強制目的に変更)
  - 最終状態: 全テスト pass (lib 2365, integration 95), clippy 0, fmt 0
- **2026-04-13 #14 (Phase 8 完了)**: Phase 8 (Const instance + 統合チェックポイント) 完了。
  - P8.1: `convert_callable_trait_const` に `Item::Const` emission 追加。
    `const getCookie: GetCookieGetCookieImpl = GetCookieGetCookieImpl;` 形式の
    module-level const instance 生成。4 fixture の snapshot 更新
  - P8.2: 変換側統合チェックポイント。
    callable-interface-inner / callable-interface-async の fixture body 単純化
    (Option narrowing I-360 回避、PRD H3 fixture body 制限に準拠)。
    compile_test.rs の skip リストから callable-interface 系 6 fixture を全て復帰。
    `async-class-method` の `skip_compile_with_builtins` stale entry も同時修正
    (P4.2 exit criteria の incomplete 分)。
    `Box<dyn Fn(` が callable-interface snapshot に残っていないことを確認。
    doc comment stale 修正 (convert_callable_trait_const)。
    全テスト pass、clippy 0、fmt 0
  - Phase 9 前提として `return_wrap_ctx` / `spawn_nested_scope_with_wrap` 削除判断を明記
  - 最終状態: 全テスト pass (lib 2365, integration 94, compile 4, E2E 88), clippy 0, fmt 0
- **2026-04-12 #12 (Phase 6 完了)**: Phase 6 (Return wrap) 完了。
  - P6.0: `return_wrap_ctx` field + `spawn_nested_scope_with_wrap` factory
  - P6.1: `return_wrap.rs` — ReturnWrapContext, build_return_wrap_context, wrap_leaf,
    variant_for, unique_option_variant + unit test 12 件
  - P6.2-P6.4: wrap_body_returns + wrap_expr_tail (If/IfLet 対応) 実装。
    inner fn body への適用は TypeResolver 型情報不足で Phase 7 に先送り
  - P6.5: CLI synthetic items 結合修正 + builtin 名前衝突対策。
    根本原因トレース: CLI はデフォルトで builtin types を読み込み、
    Web Streams API `Transformer` がユーザー定義と衝突 → `classify_callable_interface`
    が NonCallable を返していた。fixture 名を `StringMapper` に変更で解消。
    `main.rs::transpile_file` に `render_referenced_synthetics_for_file` 呼び出しを追加
  - Phase 7 に return wrap 設計課題を詳細記載 (inner fn body の return type 設計判断)
  - 最終状態: 全テスト pass, clippy 0, fmt 0
- **2026-04-12 #11 (Phase 5 完了)**: Phase 5 (Marker struct + inner fn) 全 sub-phase 完了。
  - P5.1: `used_marker_names` + `allocate_marker_name` + `marker_struct_name` (unit test 5件)
  - P5.2: `Item::Struct` に `is_unit_struct` 追加 (60+ サイト更新)。generator unit test 2件
  - P5.3: `Expr::StructInit { fields: [] }` → unit struct syntax
  - P5.4: `convert_callable_trait_const` に widest 計算 + marker struct + inner fn 生成
  - deep deep review で Critical 修正: inner params を closure params (arrow 名) に変更。
    widest (interface) 名を使うと body の変数参照が不一致になる latent bug を防止。
    `callable-interface-param-rename` fixture で検証
  - /check_problem で追加: closure params の ty=None fallback + `callable-interface-inner`
    fixture (multi-overload widest inner fn 検証)
  - 最終状態: 全テスト pass, clippy 0, fmt 0
- **2026-04-12 #10 (Phase 4 完了)**: Phase 4 (Trait emission) 全 sub-phase 完了。
  - P4.1: `convert_callable_interface_as_trait` — callable interface → `Item::Trait`
    (call_0, call_1 method)。snapshot 3件更新、compile_test 3件一時除外
  - P4.2: `RustType::unwrap_promise()` + `is_promise()` 追加。trait method + class method
    の Promise<T> → T unwrap。async-class-method compile_test 復帰。INV-6 lint script
  - P4.3: `callable_trait_name_and_args` + `convert_callable_trait_const` skeleton
  - /check_job で発見・修正: `RustType::unwrap_promise()` / `is_promise()` unit test 8件追加、
    INV-6 既存関数置換を P9.2 に義務付け + Phase 13 に確認追加
  - /check_problem で発見・修正: `classify_callable_interface` に `is_interface` guard 追加
    (type alias 由来 callable type を NonCallable に判定、後続 phase の trait 未定義不整合を防止)。
    PRD P4.1 Exit の INV-2 lint 記述を修正 (TsTypeLiteralInfo と TypeDef の層の違い)
  - 最終状態: 2331 tests (全テスト pass), clippy 0, fmt 0
- **2026-04-12 #9 (Phase 3 完了 + 教訓反映)**: Phase 3 (Widest signature) 完了。
  - `overloaded_callable.rs` 新規作成 (compute_widest_params, compute_union_return,
    WidestSignature)
  - deep review で発見・修正: mixed void/non-void return → Option wrap
  - /check_problem で発見・修正: (1) async-class-method compile_test skip 追加
    (2) `RustType::Any` const skip ガード追加 (keyword-types fixture 破壊防止)
  - **教訓**: phase 完了時は `cargo test`（全テスト）を実行。`cargo test --lib` のみ
    では integration/compile test regression を見逃す → Phase 構造セクションに共通
    Exit 条件として追記
  - 最終状態: 2322 tests (全テスト pass), clippy 0, fmt 0
- **2026-04-12 #8 (Phase 2 完了)**: Phase 2 (Registry + classification) 全 sub-phase 完了。
  - P2.1: `CallableInterfaceKind` enum + `classify_callable_interface` (unit test 8 件)
  - P2.2: INV-2 lint script (既存 violation を warning 検出)
  - P2.3: Pass 2a (non-Var) / 2b (Var) 分割 + forward-declared callable test
  - P2.4: callable interface arrow → `ConstValue { type_ref_name }` 登録
  - deep deep review で発見・修正: (1) `collect_decl` の `d.name` 二重 match 解消
    (2) `call_sig + methods` / `call_sig + constructor` テスト追加
    (3) `collection` module を `pub(crate)` に変更 (Phase 4.3 アクセス用)
  - 最終状態: 2313 tests, clippy 0, fmt 0
- **2026-04-12 #7 (Phase 1 完了)**: Phase 1 (IR foundations) 全 sub-phase 完了。
  - P1.1: `Item::Const` variant 追加 (fold/visit/generator/test_fixtures 対応)
  - P1.2: `Method::is_async` field 追加 (全 17 構築サイト更新)
  - P1.3: generator async keyword 出力 (trait sig + impl method)
  - P1.4: `function.is_async` → `Method::is_async` propagation + fixture
  - P1.5: `convert_var_decl_module_level` rename + const-safe Lit init。
    deep deep review で発見・修正: (1) Str/Regex/BigInt の const-safe フィルタ追加
    (2) 型注釈なしリテラルの型推論 `infer_const_type` 追加
    (3) `libconst_primitive_out.rlib` 混入防止 (`.gitignore` に `*.rlib` 追加)
  - 最終状態: 2303 tests, clippy 0, fmt 0
- **2026-04-12 #6 (Phase 0 完了)**: Phase 0 全 sub-phase 完了。
  - P0.0: Baseline (2297 test, cov 91.63%, Hono 71.5%/58err)
  - P0.1: IfLet 発生確認 (ternary narrowing)、Match 不発生
  - P0.2: `RustType::unwrap_promise()` 未存在確認
  - P0.3: L2/L3/L4 verification (real 1 件: L2-4 indent)
  - P0.4: factory method refactor 完了 (12 サイト移行 + lint)
  - PRD 整合性修正: A'-4 Phase 9.2→9.3、dependency notes 矛盾解消、
    lint scope src/ 全体化
- **2026-04-12 #5 (Revision 3.3)**: 第三者視点でのフェーズ構成・チェックポイント
  レビューを反映。手戻りリスク最小化のための修正:
  - C1: P4.1〜P8.1 間の compile_test 破損ウィンドウ対策 (一時除外 + P8.2 統合
    チェックポイント新設)
  - C2: P1.5 scope を `Expr::Lit` のみに限定 (`Expr::Call` / `Expr::Ident` は
    follow-up PRD)。Exit 基準の矛盾 (compile pass 不可能) を解消
  - C3: P5.2 ZST derive 変更を marker 専用に限定 (`Item::Struct` に
    `is_unit_struct: bool` フラグ追加)。グローバル影響を排除
  - H1: P8.2 (変換側統合チェックポイント) 新設。Phase 9 以降の問題切り分けを容易化
  - H2: Phase 1 / Phase 2 の並列化可能性を dependency graph に注記
  - H3: P5〜P8 fixture body を simple に制限 (TypeResolver 未更新期間の対策)
  - H4: P0.3 Exit に Phase 12 scope cap 追加 (real 6 件以上でユーザー協議)
  - M1: P3.3 Exit に `cargo test --lib` 追加
  - M2: P4.3 skeleton を `todo!()` から最小 emit に変更
  - M3: P9.2 Entry に caller 全列挙を追加
  - M4: P0.4b に synthetic axis 分類基準を明示
  - M5: P1.5 に rename blast radius 確認を追加
  - L1: Cascade rollback strategy を Design section に追加
  - L2: P0.0 に snapshot 影響列挙を追加
  - L3: P2.3 Entry を P2.2 → P2.1 に修正 (不要な直列依存解消)
  - Phase 番号整理: P8.2 新設により Phase 9〜13 の phase 番号は不変 (P8.2 は
    Phase 8 内の sub-phase)
- **2026-04-12 #4 (Revision 3.2)**: Revision 3.1 への critical review (33 件) を
  反映。主要修正:
  - F1/F2: `max_by_key` 行番号を実コード (L160) に統一、`select_overload` 行範囲を
    実コード (L176-232) に修正
  - F4/F5: Transformer 直接構築サイトを 10 → 12 サイトに更新 (2 件漏れ + 4 件行番号
    ずれ修正)、`spawn_nested_scope` は「拡張」ではなく「新規作成」と訂正
  - C1: P5.4 の自己参照 Entry を解消、factory method refactor を Phase 0.4 に移動
  - C2: P9.2 / P9.3 順序 swap
  - C3: R4-C3 transformer 側 fix phase (P1.5) を新設
  - C4: `convert_callable_trait_const` 関数の skeleton 作成 phase (P4.3) を新設
  - C7: Phase 6 prelude の stale P6.5/P6.6 reference を削除
  - C8: P0.0 に Hono bench baseline + Hono 内 callable interface 使用調査を追加
  - C11: E2E script を Fixture table から別 section に分離
  - C12: P4.1 Exit に期待 snapshot 例を追加
  - C13: 旧 Phase 12 (quality gate) と旧 Phase 13 (L2/L3/L4 fix) の順序 swap —
    L2/L3/L4 real 項目 fix を Phase 12 に繰上、Final Quality gate を Phase 13 に移動
  - Phase 番号整理: 旧 P5.4/P5.5 → P0.4、旧 P5.6 → P5.4
- **2026-04-12 #3 (Revision 3.1)**: Round 1-3 (/check_job / deep / deep deep) の既存
  fix 一覧を section A' として追加。前 Revision は Round 4 の Critical のみ catalog
  していたため、Round 1-3 で fix された問題を次 session が再導入する risk があった。
  Round 1-3 の各 fix を preserve / restructure / reverse に分類し対応 phase を明示
- **2026-04-12 #2 (Revision 3)**: Revision 2 の critical review で発見した 24 件の
  問題を反映。前提事実 (line 番号、関数の存在/非存在、IR の state) を pre-I-392
  実装状態で再確認し、factual error を修正。Phase 0 に investigation / baseline /
  Item::Const 追加を追加配置。Phase 順序を dependency graph に合わせて修正
- **2026-04-12 #1 (Revision 2)**: 初回実装を revert し、verification 結果を反映した
  設計に改訂。細かい phase 分けと invariant 明示を導入。Revision 2 には line 番号の
  誤り (revert 前の番号を転記) や phase 順序の循環 (P0.3 → P1.1) 等の factual error
  があった
- **2026-04-xx (Revision 1)**: 初版。wrapper struct + inner Box<dyn Fn> 設計
  (Option A) で起票。実装中に ZST marker + trait 設計 (Option B) に変更されたが
  PRD 未更新

## Session 引継ぎ事実 (必ず最初に読む)

本 PRD は 2 回の失敗 (Revision 1 実装 + Revision 2 PRD 作成) を経ており、
次 session (Revision 3 実装) は以下の事実を前提とする:

### 前回 session の process 問題 (次 session で繰り返してはならない)

1. **Review finding を verification なしに信頼しない**
   - `/check_job` output は「要検証の候補」であり確定事実ではない
   - Summary 経由の findings には false alarm が混ざる (前 session で
     R4-C3 が典型例 — L1-5 revert を実行していたら新規 bug を導入していた)
   - 必ず実コードを読み、fixture を作って rustc で compile 確認してから fix 対象に含める
2. **結論を flip-flop しない**
   - 証拠不足で結論を出すと、次の証拠が入るたびに overhaul が発生する
   - 途中報告は「観察した事実」のみに留める。解釈・分類・fix 方向は全 fact が揃って
     から出す
3. **Scope を unilateral に縮小しない**
   - L3/L4 を「polish」として follow-up PRD に切り出す提案は user に却下された
   - 全項目を scope 内として扱い、分類は fact 収集後に user と相談
4. **git コマンドは一切実行しない**
   - `git status` / `log` / `diff` / `blame` / `show` を含め全ての git subcommand を
     user 経由で実行する。情報取得目的でも私が直接 git を呼ばない
5. **cargo commands (check/test/clippy/fmt) は実行許可を求めない**
   - これらは非破壊的なので、必要な時に自律的に実行する

### 前回 session で empirical に確認した事実 (証拠は `report/i392-round4-verification.md`)

Round 4 で発見された 7 件の Critical 問題が前回の実装で検出され、全て real silent
bug であることを fixture + rustc 出力で確認済:

- R4-C1/C2: `convert_callable_trait_const` の fallthrough 片側非対称
- R4-C3: **Summary 誤認**。真の問題は `convert_var_decl_arrow_fns` が non-arrow init
  を silent skip する **pre-existing gap** (L1-5 relaxation とは無関係)
- R4-C4: Marker struct の PascalCase collision
- R4-C5: Generic arity mismatch で free TypeVar 残存
- R4-C6: `any_enum_override` が widest と不整合な inner fn 生成
- R4-C7: `Method` struct に `is_async` field なし → async callable interface 破損

### 前 session Round 1-3 で実装された既存 fix (section A' に詳細)

Revision 1 実装中に Round 1 → Round 2 → Round 3 の /check_job で発見・修正された
問題群があり、本 PRD ではそれらを **preserve / restructure / reverse** のいずれか
として明示的に扱っている。特に重要な **reverse** (前回の fix を意図的に巻き戻す):

- **R2-L1-2 (TypeResolver fallthrough recovery) REVERSE**: Round 2 で追加された
  "I-392:" prefix catch + fallthrough が Round 4 で R4-C1/C2 silent bug の root
  cause と判明。INV-3 で fallthrough 全面禁止 (hard error) に統一
- **R3-L1-4 (Pass 2a/2b clone 削減 L1-5) REVERSE**: 機能 + 最適化の混在の典型例。
  Pass 1 snapshot 使用禁止、Pass 2a 完了後の snapshot のみ使用

section A' の table で全 Round 1-3 fix の扱いを個別に確認すること。本 PRD 実装時に
Round 1-3 で解決済の問題を再導入する risk を避けるため、各 phase Exit で preserve
状態を検証する

## Background

`convert_interface_as_fn_type` (`src/pipeline/type_converter/interfaces.rs:139-241`)
が `max_by_key(|s| s.params.len())` (L160 — 実コード empirical 確認済) で最長
overload のみ採用し、他の overload を silent に破棄している。

```typescript
interface GetCookie {
    (c: Context): Cookie;                                    // overload 1
    (c: Context, key: string): string | undefined;           // overload 2
    (c: Context, key: string, prefix?: PrefixOpts): string | undefined; // overload 3
}
```

現状の変換: `type GetCookie = Box<dyn Fn(Context, String, Option<PrefixOpts>) -> Option<String>>`
→ overload 1 の `Cookie` return type が消失。overload 1 で呼び出すコードが
silent semantic change。

Hono で 4 つの overloaded callable interface が実在 (GetCookie, GetSignedCookie,
SetHeaders, SetMetric)。

## Goal

Multi-overload callable interface を、各 overload の return type を正確に保持した
trait + marker + impl 表現に変換する。call site で overload resolution を行い、
正しい method と return type を生成する。

**Design decision**: Single-overload callable interface も同じ trait 構造に寄せる
(shape 統一のため)。ただしこの decision は trade-off を伴うため、下記「Trade-off
analysis」参照。

### Single-overload を trait 化する trade-off analysis

| 項目 | Pro (trait 化) | Con (現状の `type = Box<dyn Fn>` 維持) |
|---|---|---|
| I-392 code の branch 数 | 1 path | 2 path (single/multi) |
| 既存 snapshot 影響 | 大量書き換え (全 callable interface) | 最小 (multi-overload のみ) |
| runtime cost | trait dispatch のみ (`Box<dyn Fn>` より軽い) | `Box<dyn Fn>` 経由 |
| 他 Rust code との integrability | trait impl は interop しやすい | `Box<dyn Fn>` は受け入れにくい |
| Hono 影響 | GetValue 等の single も変換結果が変わる | GetCookie/etc 4 つのみ |

**選択**: **trait 化**を採用。理由: I-392 code の branch 数削減 (non-convergence
の原因の 1 つだった) と長期的な correctness (trait dispatch の方が interop しやすい)。
Hono 影響は大きいが P0.0 の Hono usage grep で事前把握 + Phase 11.2 で bench
regression を確認する。

## 前回実装 (Revision 1) の失敗要因と lessons learned

### 失敗の内容

Revision 1 の設計で実装を進めた結果、`/check_job deep deep deep` まで 4 ラウンドの
review で 50+ 問題が発見され、Round 4 で Critical 6 件が新規発見された
(非収束 pattern)。

### 失敗要因

1. **Review finding を verification なしに fix していた**: `/check_job` findings を
   全て「real issue」として扱い patch を重ねた。一部 false alarm であり、fix が
   新規 bug を導入する risk があった (R4-C3 が典型例)
2. **構造変更と最適化を混在させた**: 機能実装 (trait + marker + const)、最適化
   (L1-5 single-clone)、refactor (factory method, DRY 集約) を 1 ブランチで
   同時進行させた。最適化が silent bug を導入し循環に陥った
3. **Invariant を人間依存で保った**: Transformer の 10 の直接構築サイトで新 field
   (`return_wrap_ctx`) を手動で設定する設計だった
4. **Phase 分割が粗すぎた**: T1 が「struct + impl 生成」を 1 task にまとめており
   中間チェックポイントがなかった
5. **前提の empirical 検証を怠った**: PRD 内の line 番号・関数の存在等が推測で書かれ、
   Revision 2 でも 5 件の factual error を持ち越した

### Lessons learned

- **Phase は 1 task 1 check**: 各 phase は単一変更、完了時に cargo check + rustc
  compile test + 既存 test を通してから次へ
- **Invariant は型 or lint で enforce**: field visibility / newtype / exhaustive
  match / grep-based CI lint のいずれかで強制する
- **Empirical verification 優先**: PRD 起票時に必ず実コード line 番号と関数の存在を
  確認。fixture を書いて rustc compile まで確認してから phase 完了
- **機能と最適化を分離**: 機能実装完了後、quality check が全て green になってから
  別 PRD で最適化。本 PRD に optimization を含めない
- **前置 prerequisite を明示**: 既存 code の gap (R4-C3 等) が I-392 の前提として
  必要な場合、Phase 0 として本 PRD 内に明示

## Design

### アプローチ: Option B (trait + per-value ZST marker + private inner fn + const instance)

```typescript
interface GetCookie {
    (c: Context): Cookie;
    (c: Context, key: string): string | undefined;
}
const getCookie: GetCookie = (c, key?) => { /* ... */ };
```

→ 以下 6 要素を生成:

```rust
// 1. trait (per-overload method signature)
trait GetCookie {
    fn call_0(&self, c: Context) -> Cookie;
    fn call_1(&self, c: Context, key: String) -> Option<String>;
}

// 2. Synthetic union enum (divergent return)
enum CookieOrOptionString {
    Cookie(Cookie),
    OptionString(Option<String>),
}

// 3. Per-value ZST marker struct
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GetCookieGetCookieImpl;

// 4. Inner fn (private, widest signature)
impl GetCookieGetCookieImpl {
    fn inner(&self, c: Context, key: Option<String>) -> CookieOrOptionString {
        // arrow body, with each `return expr` wrapped in a union variant
    }
}

// 5. Trait delegate impl (narrows return)
impl GetCookie for GetCookieGetCookieImpl {
    fn call_0(&self, c: Context) -> Cookie {
        match self.inner(c, None) {
            CookieOrOptionString::Cookie(v) => v,
            _ => unreachable!("guaranteed by TS overload type"),
        }
    }
    fn call_1(&self, c: Context, key: String) -> Option<String> {
        match self.inner(c, Some(key)) {
            CookieOrOptionString::OptionString(v) => v,
            _ => unreachable!("guaranteed by TS overload type"),
        }
    }
}

// 6. Module-level const instance
const getCookie: GetCookieGetCookieImpl = GetCookieGetCookieImpl;
```

### Invariants (型 or lint で enforce)

| Invariant | 説明 | 強制方法 (具体) |
|---|---|---|
| INV-1 | Marker struct name は module 内で unique | `Transformer::used_marker_names: HashSet<String>` (per-module state)。衝突時に suffix append でも回避できない場合は hard error |
| INV-2 | callable interface の判定は **単一関数** `classify_callable_interface(def: &TypeDef) -> CallableInterfaceKind` に集約 | 全ての callable 判定箇所が同関数を import。grep-based CI lint `scripts/check-classify-callable-usage.sh` で `TypeDef::Struct.*is_interface.*call_signatures` パターンの直接 match を禁止 |
| INV-3 | conversion 側と call 側の fallback は **同一判定関数**を使う | INV-2 が成立すれば自動保証。加えて conversion 失敗は hard error (fallthrough 禁止) |
| INV-4 | generic type args の arity は必ず type params と一致 | arity mismatch 時は hard error (silent broken IR 禁止) |
| INV-5 | `Method::is_async` は arrow/fn の is_async と一致 | `Method` struct に `is_async: bool` field 追加。全 `Method { ... }` 構築サイトで明示的に set |
| INV-6 | Promise<T> は trait sig / delegate / inner 全てで consistent | Promise unwrap を単一関数 `RustType::unwrap_promise()` に集約。trait method / Item::Fn / Method の全 3 経路で同関数を呼ぶ |
| INV-7 | `any_enum_override` は callable interface arrow body では適用しない | widest signature を source of truth。`convert_callable_trait_const` は `any_enum_override` を呼ばない |
| INV-8 | Transformer 構築は factory method 経由のみ (field 追加時の leak 防止) | `scripts/check-transformer-construction.sh` lint で sub-module の struct literal 構築を禁止 (factory method 経由のみ許可)。元は `return_wrap_ctx` leak 防止目的で導入、Phase 9A で `return_wrap_ctx` 削除後も factory method 強制は新 field 追加時の安全策として維持 |
| INV-9 | 全 callable interface fixture は rustc で compile が通る | `tests/compile_test.rs` に全 fixture を登録 |

### Cascade rollback strategy (Revision 3.3 L1)

`incremental-commit.md` により各 phase 完了時に commit される。Phase N で Phase M
(M < N) の設計問題が発覚した場合:

1. Phase N の変更を discard
2. Phase M の commit まで git reset (user 実施)
3. Phase M から再設計・再実装
4. 再設計が必要な場合、影響する downstream phase の Entry/Work/Exit を PRD 上で修正
   してから実装に入る (PRD を事実に追従させる)

Phase 間 commit があるため、cascade rollback の最大損失は「最後の commit 以降の
変更」に限定される。

### Failure mode: conversion 失敗時の挙動 (明示的選択)

**選択**: **Hard error**。fallthrough (plain Fn path への silent degradation) は禁止。

理由:
- Revision 1 の fallthrough path は片側非対称 (conversion 側は fallthrough するが
  call 側は trait dispatch のまま) で R4-C1/C2 silent bug を生んだ
- INV-2/3 により judgment は単一関数に集約され、judgment 後の「変換処理」の失敗
  (例: wrap walker の type resolution 失敗) は hard error として surface させる
- `ideal-implementation-primacy.md` の silent semantic change 禁止に準拠

### Impact Area

| File | Change |
|------|--------|
| `src/ir/item.rs` | `Method` に `is_async: bool` 追加 (INV-5)、`Item::Const { vis, name, ty, value: Expr }` 新規 variant 追加、`Item::Struct` に `is_unit_struct: bool` 追加 (Revision 3.3 C3) |
| `src/ir/fold.rs`, `visit.rs`, `test_fixtures.rs`, `visit_tests.rs` | Method/Item::Const の visit/fold 対応 |
| `src/generator/mod.rs`, `expressions/mod.rs`, `tests.rs` | Method の async keyword、Item::Const generator、ZST struct の `struct Name;` 形式 |
| `src/pipeline/type_converter/interfaces.rs` (487 行) | `convert_interface_as_fn_type` (L139-241, L160 で max_by_key 採用) を trait 生成に変更 (Phase 4.1) |
| `src/pipeline/type_converter/overloaded_callable.rs` | **新規**: widest computation, union return, method naming (Phase 3) |
| `src/pipeline/type_resolver/helpers.rs` (327 行) | `resolve_fn_type_info` (L289-327) — 現 `select_overload(..., 0, &[])` bug を修正 + `synthetic` 引数追加 + callable interface は widest を返す (Phase 9.2) |
| `src/pipeline/type_resolver/call_resolution.rs` | Ident callee で `classify_callable_interface` を参照 (Phase 10.1) |
| `src/registry/collection.rs` (1239 行) | `classify_callable_interface` 単一関数の新規定義 (INV-2, Phase 2.1)。`collect_decl` Var branch (L268-) で arrow init の型注釈を consume し ConstValue 登録 (Phase 2.4) |
| `src/registry/mod.rs` (835 行) | Pass 2 を non-Var 先 / Var 後の 2 ステップに分割 (Phase 2.3)。`select_overload` (L176-232) の Stage 2 修正 (Phase 9.4) |
| `src/transformer/mod.rs` | `Transformer::used_marker_names` 追加 (INV-1, Phase 5.1)、`return_wrap_ctx` field (INV-8, Phase 6.1 前)、factory method 新規作成 (Phase 0.4) |
| `src/transformer/return_wrap.rs` | **新規**: ReturnWrapContext、wrap walker (polymorphic None 厳格化, Phase 6) |
| `src/transformer/functions/arrow_fns.rs` (現 180 行) | (a) `convert_var_decl_arrow_fns` → `convert_var_decl_module_level` rename + Lit init 対応 (Phase 1.5, Revision 3.3 C2 scope 限定) (b) `convert_callable_trait_const` 新規関数 (Phase 4.3〜8.1 で展開) |
| `src/transformer/expressions/calls.rs` (734 行) | `try_convert_callable_trait_call` 新規 (Phase 10.1) |
| `src/ir/item.rs` (205 行) | `Method` (L65-80) に `is_async: bool` 追加 (Phase 1.2)、`Item::Const` 新規 variant (Phase 1.1) |
| `src/ir/fold.rs`, `visit.rs`, `test_fixtures.rs`, `visit_tests.rs` | Method/Item::Const の visit/fold 対応 (Phase 1.1/1.2) |
| `src/generator/mod.rs`, `expressions/mod.rs`, `tests.rs` | Method の async keyword (Phase 1.3)、Item::Const generator (Phase 1.1)、`is_unit_struct: true` 時の `struct Name;` 形式 (Phase 5.2, Revision 3.3 C3) |
| `scripts/check-classify-callable-usage.sh` (新規) | INV-2 lint (Phase 2.2) |
| `scripts/check-transformer-construction.sh` (新規) | INV-8 lint (Phase 0.4b) |
| `scripts/check-promise-unwrap.sh` (新規) | INV-6 lint — 生成 Rust 内に literal `Promise<` が残らないことを確認 (Phase 4.2) |
| `tests/fixtures/callable-interface-*.input.ts` | 新規 fixture 多数。既存 `callable-interface.input.ts` は修正 (既存 snapshot 更新, Phase 4.1) |
| `tests/e2e/scripts/callable_interface.ts` + `tests/e2e_test.rs` | **新規 E2E test** (Phase 11.1) |
| `tests/compile_test.rs` | 全 callable-interface fixture を compile check エントリに登録 (Phase 11.1) |

## Phase 構造

各 phase の entry criteria / work / exit criteria / rollback / dependencies を明示。

### 全 Phase 共通の Exit 条件 (Phase 1 実装時の教訓)

**各 phase 完了時は `cargo test`（全テスト）を実行すること。`cargo test --lib` のみでは
不十分。** integration test (`tests/integration_test.rs`) と compile test
(`tests/compile_test.rs`) は既存 fixture の snapshot 変更や compile 可否に依存しており、
`--lib` では検出できない regression がある。

Phase 1 実装時に `cargo test --lib` のみで検証していたため、以下の 2 件が次 phase まで
発見されなかった:
- `async-class-method` fixture が `Promise<String>` で compile 不可 → compile_test skip 追加
- `keyword-types` fixture の `const anyVal: any = 42` が `Item::Const` に変換され
  snapshot 破壊 → `RustType::Any` の場合 skip するガード追加

**実行すべきコマンド**: `cargo test` (引数なし、全テスト実行)。
ただし所要時間が長い (約 80 秒) ため、開発中は `cargo test --lib` で高速検証し、
**phase 完了判定時のみ** `cargo test` を実行する。

### Phase dependency graph (G1 対応)

下記は phase 間の dependency を ASCII で表現したもの。矢印は前提依存 (A → B は
「B 開始前に A 完了必須」を意味する)。

```
Phase 0 (investigation + prerequisite refactor)
├── P0.0: Baseline + Hono grep + bench baseline
│   ├─→ P0.1: IfLet/Match 発生調査
│   ├─→ P0.2: Promise<T> 変換経路調査
│   ├─→ P0.3: L2/L3/L4 verification
│   └─→ P0.4a: factory method 新規作成 (independent refactor)
│       └─→ P0.4b: 12 サイト移行
│           └─→ P0.4c: INV-8 lint script
│
Phase 1 (IR foundations)
├── P1.1: Item::Const variant (← P0.x 全完了)
│   └─→ P1.2: Method::is_async field
│       └─→ P1.3: Method generator async keyword
│           └─→ P1.4: Item::Fn → Method propagation
│               └─→ P1.5: arrow_fns non-arrow init 対応 (R4-C3 transformer 側)
│
Phase 2 (Registry + classification) ─ Phase 1 と並列化可能 (H2)、直列推奨
├── P2.1: classify_callable_interface 定義 (← Phase 1 完了)
│   ├─→ P2.2: INV-2 lint script (P2.3 前提不要、Phase 2 内いつでも)
│   ├─→ P2.3: Pass 2a/2b split (← P2.1 完了、L3 修正)
│   │   └─→ P2.4: collect_decl 型注釈 consume
│
Phase 3 (Widest signature) ─ pure library code (← Phase 2 完了)
├── P3.1 → P3.2 → P3.3
│
Phase 4 (Trait emission)
├── P4.1: convert_interface_as_fn_type → trait (← Phase 3 完了)
│   └─→ P4.2: async + unwrap_promise (← P0.2 + P1.4)
│       └─→ P4.3: convert_callable_trait_const skeleton
│
Phase 5 (Marker struct + inner fn)
├── P5.1: used_marker_names (INV-1) (← Phase 4 完了)
│   └─→ P5.2: ZST struct emission
│       └─→ P5.3: StructInit unit syntax
│           └─→ P5.4: Inner fn emission (旧 P5.6、← P4.3 skeleton)
│
Phase 6 (Return wrap) ─ 詳細は P0.1 結果に依存
├── P6.0: return_wrap_ctx field + spawn_nested_scope_with_wrap factory (← Phase 5 完了)
│   └─→ P6.1: ReturnWrapContext + wrap_leaf
│       └─→ P6.2: Polymorphic None 厳格化
│           └─→ P6.3: Expr::If (ternary) wrap
│               └─→ P6.4: IfLet/Match 対応 (P0.1 結果依存)
│
Phase 7 (Trait delegate impl)
├── P7.1: per-overload delegate method (← Phase 6 完了)
│   └─→ P7.2: inner arg wrapping
│       └─→ P7.3: async 伝搬 (← P4.2)
│
Phase 8 (Const instance + 統合チェックポイント)
├── P8.1: Item::Const emission (← Phase 7 完了)
└── P8.2: 変換側統合チェックポイント (← P8.1 完了)
         compile_test 復帰 + end-to-end 検証

Phase 9 (Generic)
├── P9.1: arity validation (INV-4) (← P8.2 完了)
│   └─→ P9.2: resolve_fn_type_info widest ベース書換 (C2 swap 済)
│       └─→ P9.3: type substitution 単一 helper
│           └─→ P9.4: select_overload Stage 2 修正

Phase 10 (Call site dispatch)
├── P10.1: try_convert_callable_trait_call (← Phase 9 完了)
│   └─→ P10.2: select_overload integration
│       └─→ P10.3: Fallthrough symmetry

Phase 11 (Integration + coverage)
├── P11.1: compile_test.rs + E2E (← Phase 10 完了)
│   └─→ P11.2: Hono bench regression (P0.0 baseline と比較)

Phase 12 (L2/L3/L4 real 項目 fix) ─ P0.3 結果依存

Phase 13 (Final Quality gate) ─ 全 phase 完了後の最終確認
```

主な dependency note:
- **P0.4 系** (factory refactor) は他 phase に依存しない独立 refactoring で、
  P0.0 直後に実施可能
- **Phase 1 と Phase 2 は理論的に並列化可能** (Revision 3.3 H2): P2.1
  (`classify_callable_interface`) は IR 変更 (Item::Const, Method::is_async) に技術的に
  依存しない。ただし P2.1 Entry は「Phase 1 完了」を維持する (直列実行推奨 — 問題発生時の
  原因切り分けが容易になるため)。P4.3 が P1.5 + P2.1 の両方に依存するため、
  どちらを先に実行しても P4.3 の開始時期は変わらない
- **P1.5** (arrow_fns non-arrow fix) は P1.1 (Item::Const variant) + P1.4 完了後
- **P4.2** は P0.2 (Promise 調査) + P1.4 (is_async propagation) 必須
- **P6** 全体は P5 + P0.1 の結果に依存
- **P8.2** (統合チェックポイント) は P8.1 完了後。Phase 9 の前提
- **P11.2** は P0.0 の Hono bench baseline に依存
- **Phase 12** は P0.3 の verification 結果に依存 (real 項目数)
- **Phase 13** は全 code 変更完了後の最終確認なので Phase 12 より後に置く

### Phase 0: Baseline + Investigation + Prerequisites

機能実装に入る前に **調査と前提整備** を完了させる。全て read-only investigation
または、I-392 と独立した structural refactoring (P0.4)。

#### P0.0: Baseline 確認 + 既存状態の計測

- **Entry**: main branch 最新、作業ブランチは I-392 用に新規作成済 (user 実施)
- **Work**:
  1. `cargo check` pass 確認
  2. `cargo test` 全件 pass 確認 (現在の baseline: 2295 lib tests + integration)
  3. `cargo clippy --all-targets --all-features -- -D warnings` 0 warning 確認
  4. `cargo fmt --all --check` 0 diff 確認
  5. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` pass 確認
     (threshold は 89 で baseline 測定)
  6. 現 `tests/fixtures/callable-interface.input.ts` (30 行) と既存 snapshot
     (`integration_test__callable_interface.snap`) の内容を `report/i392-baseline-snapshots.md`
     に記録。Phase 4.1 で snapshot が書き換わる予定位置をマーク
  6b. **Snapshot 影響列挙** (Revision 3.3 L2):
      `grep -rn 'Box<dyn Fn' tests/snapshots/` で callable interface 型を含む
      snapshot を全列挙し `report/i392-baseline-snapshots.md` に追記。
      P4.1 Exit で全件の更新完了を確認するためのチェックリスト
  7. **Hono bench baseline の記録**: `./scripts/hono-bench.sh` 実行し、clean/error
     数を `bench-history.jsonl` に記録。この数値を Phase 11.2 の regression check
     の baseline とする
  8. **Hono 内 callable interface 使用の調査** (C18 対応): Hono source 内で
     `grep -rn ": GetCookie\|: GetSignedCookie\|: SetHeaders\|: SetMetric\|: GetValue"`
     で現 callable interface 使用サイトを列挙し、`report/i392-hono-usage.md` に
     記録。single overload 使用 vs multi overload 使用の数をカウント。trait 化に
     よる影響範囲の事前把握
- **Exit**:
  - 上記 5 件 (1-5) clean
  - `report/i392-baseline-snapshots.md` 作成済
  - `report/i392-hono-usage.md` 作成済
  - Hono bench baseline 記録済
- **Rollback**: なし (read-only)

#### P0.1: `Expr::IfLet` / `Expr::Match` 発生確認 (L2-1 root investigation)

- **Entry**: P0.0 完了
- **Work**: 以下を empirical に確認する fixture を書き、現 transpiler で変換して
  生成 IR を観察:
  1. `typeof narrowing + callable interface` → `Expr::IfLet` が生成されるか?
  2. `switch inside callable-interface arrow body` → `Expr::Match` が生成されるか?
  3. いずれも arrow の return 位置で発生するか? 発生位置 (Stmt::Return の Expr か?
     arrow expression body の値か?)
- **Exit**: 調査結果を `report/i392-ifletmatch-investigation.md` に fact として記録
  (「発生する」「発生しない」「どの経路」)。Phase 6 の設計をこの結果に依存させる。
  **Phase 6 の詳細設計は P0.1 完了まで確定しない** (結果を事前仮定しない)
- **Rollback**: なし (read-only investigation)

#### P0.2: 既存 Promise<T> 変換経路の調査

- **Entry**: P0.0 完了
- **Work**:
  1. `Item::Fn` の async / Promise<T> 変換経路を調査 (`grep -r "unwrap_promise\|is_async"`)
  2. 現状 arrow async fn が生成される時 `Promise<T>` が `T` に unwrap されるか確認
  3. Interface の trait method 位置 (`convert_interface_as_fn_type` と別関数)
     で Promise<T> はどう扱われるか確認
  4. `RustType::unwrap_promise()` method が既に存在するか確認 (存在しなければ
     Phase 4.2 で追加)
- **Exit**: 結果を `report/i392-promise-investigation.md` に fact 記録。Phase 4.2
  の設計はこの結果に依存
- **Rollback**: なし

#### P0.3: L2/L3/L4 verification (前 session 未完分)

- **Entry**: P0.0 完了
- **Work**: `report/i392-round4-verification.md` の未 verify 項目を fact gathering:
  - L2-1: `wrap_expr_tail` の IfLet/Match (P0.1 で確認済なら skip)
  - L2-3: string matching error discrimination — INV-2 で自動解消するか?
  - L2-4: generator match indent propagation
  - L3-1〜5: arrow type_params, error messages, suffix loop, compile_test coverage, E2E
  - L4-1〜3: string coerce depth, unreachable pattern, variant name fallback
- **Exit**:
  - 各項目の status が「real bug」「loud bug」「false alarm」「auto-solved」
    のいずれかに分類され `report/i392-round4-verification.md` に追記される
  - **Phase 12 scope cap** (Revision 3.3 H4): real bug の件数に応じて判断:
    - real 5 件以下 → Phase 12 で全て fix
    - real 6 件以上 → ユーザーと協議し Phase 12 scope を決定。
      優先度の低い項目は follow-up PRD に分離
- **Rollback**: なし

#### P0.4: Transformer factory method の新規作成 + 全直接構築サイト移行 (INV-8 の事前整備)

**C1 + F4 + F5 対応**: 旧 P5.4/P5.5 を Phase 0 に移動。これは機能追加ではなく
**invariant enforcement 目的の pre-existing gap 解消**。I-392 の他 phase に依存
しない独立した refactoring。

##### P0.4a: factory method の新規作成

- **Entry**: P0.0 完了
- **Work**:
  1. `src/transformer/mod.rs` に以下 factory methods を **新規作成**
     (現状 `spawn_nested_scope` 等は存在しないため "拡張" ではなく "新規作成"):
     ```rust
     impl<'a> Transformer<'a> {
         /// 共通: inherit mut_method_names clone、現 Transformer 状態から spawn
         pub(crate) fn spawn_nested_scope<'b>(&'b mut self) -> Transformer<'b>
         where 'a: 'b;

         /// Local synthetic registry を持つ sub-Transformer (fn body 用)
         pub(crate) fn spawn_nested_scope_with_local_synthetic<'b>(
             &'b mut self,
             local: &'b mut SyntheticTypeRegistry,
         ) -> Transformer<'b>
         where 'a: 'b;
     }
     ```
  2. Phase 6 で `return_wrap_ctx` field を追加する時に
     `spawn_nested_scope_with_wrap(ctx)` を**追加** (Phase 6.1 で扱う)
- **Exit**:
  - `cargo check` pass (factory method は定義済、まだ使われていない)
  - Factory method の unit test (dummy が sub-Transformer を得られることを確認)
- **Rollback**: diff discard

##### P0.4b: 12 production サイトを factory method に置換

- **Entry**: P0.4a 完了
- **Work**: 現存する 12 のproduction 直接構築サイト (grep 実証済) を factory method
  呼び出しに置換:
  ```
  arrow_fns.rs:58                      (自己の synthetic)
  destructuring.rs:52                  (自己の synthetic)
  destructuring.rs:140                 (自己の synthetic)
  functions/mod.rs:75                  (local synthetic — fn body 用)
  functions/mod.rs:146                 (local synthetic — fn body 用)
  expressions/functions.rs:125         (自己の synthetic)
  expressions/functions.rs:290         (確認して分類)
  expressions/functions.rs:313         (確認して分類)
  statements/loops.rs:545              (自己の synthetic)
  classes/members.rs:34                (自己の synthetic)
  classes/members.rs:201               (自己の synthetic)
  classes/members.rs:326               (自己の synthetic)
  ```
  **Synthetic axis 分類基準** (Revision 3.3 M4):
  - 現コードで `synthetic: &mut local_synthetic` (ローカル変数) を渡している →
    `spawn_nested_scope_with_local_synthetic`
  - 現コードで `synthetic: &mut self.synthetic` (親の synthetic を再利用) を渡している →
    `spawn_nested_scope`
  - 機械的に判別可能。上記リストの括弧内分類は参考 (要実コード確認)
  Test 内構築サイト (10+ 件) は scope 外 (test 専用)
- **Exit**:
  - `cargo test` 全件 pass (意味論は不変のはず)
  - Production 内 `Transformer {` literal の grep ヒット 0 件 (factory 以外で)
- **Rollback**: diff discard

##### P0.4c: INV-8 lint script

- **Entry**: P0.4b 完了
- **Work**: `scripts/check-transformer-construction.sh` 作成。production 内で
  `Transformer\s*\{` の直接構築を禁止 (factory method / `for_module` のみ許可)。
  具体的には tests/ ディレクトリ外、かつ `fn for_module\|fn spawn_` 定義行以外で
  `Transformer\s*\{` が出現したら fail
- **Exit**: lint script pass (P0.4b の結果で 0 件のはず)
- **Rollback**: なし

### Phase 1: IR foundations

#### P1.1: `Item::Const` variant を IR に追加

- **Entry**: P0.x 全完了
- **Work**:
  1. `src/ir/item.rs` に `Item::Const { vis: Visibility, name: String, ty: RustType, value: Expr }` variant を追加
  2. `src/ir/fold.rs`, `visit.rs`, `test_fixtures.rs`, `visit_tests.rs` で pattern match 対応
  3. `src/generator/mod.rs` で emission を追加: `const NAME: Ty = value;`
  4. `src/generator/tests.rs` に IR→source round-trip unit test 追加
- **Exit**:
  - `cargo check` pass
  - `cargo test --lib` 2295 + 新 test pass
  - Round-trip unit test で `Item::Const` が正しく emit される
- **Rollback**: なし (IR 追加のみ、既存動作不変)

#### P1.2: `Method::is_async` field を IR に追加 (INV-5)

- **Entry**: P1.1 完了
- **Work**:
  1. `src/ir/item.rs::Method` struct (現 L65-80) に `pub is_async: bool` 追加
  2. `fold.rs`, `visit.rs`, `visit_tests.rs`, `test_fixtures.rs` で対応
  3. 既存 `Method { ... }` 構築サイトを全 grep (`grep -rn 'Method\s*{' src/`) し、
     全サイトで `is_async: false` を明示的に追加 (現状 async method なし)
- **Exit**:
  - `cargo check` pass
  - `cargo test` 全件 pass
- **Rollback**: なし

#### P1.3: `Method` generator で async keyword 出力

- **Entry**: P1.2 完了
- **Work**:
  1. `src/generator/mod.rs` の Method 生成箇所で `is_async: true` の時 `async fn` を出力
  2. Unit test: `is_async: true` Method の round-trip
- **Exit**: generator unit test pass
- **Rollback**: なし

#### P1.4: `Item::Fn` → `Method` への is_async propagation 経路

- **Entry**: P1.3 完了
- **Work**:
  1. `grep -rn "async " tests/fixtures/` で async class method fixture の有無を
     empirical に確認し、結果を record (C21 対応)
  2. `src/transformer/classes/members.rs` (現在 class method 生成) で、fn が async
     の時 `Method::is_async = true` を set する経路を確立
  3. 既存 fixture で class 内 async method が存在する場合、正しく emit されることを
     snapshot で確認。存在しない場合、新規 fixture `tests/fixtures/async-class-method.input.ts`
     を作成して P1.4 の exit test に利用
- **Exit**:
  - `cargo test` pass
  - async class method fixture で `async fn method(...)` が snapshot に emit される
- **Rollback**: なし

#### P1.5: `convert_var_decl_arrow_fns` の non-arrow init 対応 (R4-C3 transformer 側 fix)

**C3 対応**: R4-C3 は arrow_fns.rs:28-32 の arrow-only filter が root cause。
Phase 2.4 は registry 側 (collection.rs) を直すが、transformer 側も直す必要がある。
本 phase で transformer 側を拡張する。

**Revision 3.3 C2 scope 限定**: `Expr::Call` / `Expr::Ident` の non-arrow init は
I-392 に必須ではなく、`const` vs `static` vs `lazy_static` の設計判断が必要なため
follow-up PRD に移す。本 phase は `Expr::Lit` のみ対応。

- **Entry**: P1.1 (Item::Const 追加) 完了 + P1.4 完了
- **Work**:
  1. `src/transformer/functions/arrow_fns.rs::convert_var_decl_arrow_fns`
     (現 L15-130) を拡張。関数名は **`convert_var_decl_module_level`** に rename
     (C23 対応 — 責務が arrow 限定から module-level const 全般に拡大したため)
  2. `grep -rn 'convert_var_decl_arrow_fns'` で全参照を列挙し更新 (Revision 3.3 M5)
  3. 現 L28-32 の `_ => continue` (arrow 以外を skip) を撤去
  4. Init pattern ごとに分岐:
     - `ast::Expr::Arrow(arrow)` → 現行ロジック (Item::Fn 生成) 継続
     - `ast::Expr::Lit(lit)` → const-safe リテラル (`Num`/`Bool`/`Null`) のみ
       `Item::Const` emit。`Str`/`Regex`/`BigInt` は Rust の `const` で非 const fn
       呼び出しが必要なため skip (Non-Goals に詳細記載)。
       型注釈なしの場合は `infer_const_type` で推論 (`Num→F64`, `Bool→Bool`)
     - その他 (`Call`, `Ident`, `Object`, `Array` 等) → 本 PRD scope 外。
       現行通り `continue` で skip (follow-up PRD で解決)
  5. `src/transformer/mod.rs:622` の dispatch を新関数名に更新:
     `Decl::Var(var_decl) => self.convert_var_decl_module_level(var_decl, vis, resilient)`
- **Exit**:
  - 新 fixture `tests/fixtures/const-primitive.input.ts` (`const n: number = 42;` +
    `function useN(): number { return n; }`) が rustc compile pass
  - 既存 test 全件 pass (既存の arrow init path は変更なし)
  - `grep -rn 'convert_var_decl_arrow_fns' src/` ヒット 0 件
- **Rollback**: diff discard

### Phase 2: Registry + classification (INV-2)

#### P2.1: `classify_callable_interface` 単一関数の定義

- **Entry**: Phase 1 完了
- **Work**:
  1. `src/registry/collection.rs` に以下を追加:
     ```rust
     pub enum CallableInterfaceKind {
         NonCallable,
         SingleOverload(MethodSignature),
         MultiOverload(Vec<MethodSignature>),
     }
     pub fn classify_callable_interface(def: &TypeDef) -> CallableInterfaceKind { ... }
     ```
  2. 既存 `is_callable_only` (`interfaces.rs`) との関係を定義:
     - `is_callable_only` は **AST レベル** (interface body に call sig のみか判定)
     - `classify_callable_interface` は **registry レベル** (TypeDef を分類)
     - 両者を共存させるが、`classify_callable_interface` が registry 側の一次判定
  3. unit test 3 件 (non-callable, single, multi)
- **Exit**: unit test pass
- **Rollback**: なし

#### P2.2: INV-2 lint script

- **Entry**: P2.1 完了
- **Work**: `scripts/check-classify-callable-usage.sh` を作成。registry/collection.rs
  と classify 関数本体以外で `TypeDef::Struct.*call_signatures` の直接 match を
  禁止する grep ベース lint
- **Exit**: lint script が pass (現状 classify 関数以外で該当 pattern なしのはず)
- **Rollback**: なし

#### P2.3: Pass 2 を non-Var 先 / Var 後に分割

- **Entry**: P2.1 完了 (Revision 3.3 L3 — P2.2 lint script はコード動作に影響
  しないため前提不要。P2.2 は Phase 2 完了前のいつでも実行可)
- **Work**:
  1. `src/registry/mod.rs::build_registry_with_synthetic` の Pass 2 (L818-) を
     2 段階に分割:
     - Pass 2a: `!matches!(decl, Decl::Var(_))` を先に resolve
     - Pass 2b: `Decl::Var` を後に resolve
  2. 共有 lookup snapshot は Pass 2a 完了後 (= reg.clone()) を取得。Pass 1 snapshot
     は使わない (Revision 1 の L1-5 relaxation の教訓)
- **Exit**:
  - 既存 test 全件 pass (順序変更だけで semantics 不変のはず)
  - unit test 追加: `const x: I = arrow` で I が後方宣言された場合の正常動作
- **Rollback**: なし

#### P2.4: `collect_decl` Var branch で型注釈を consume (D1 gap 対応)

- **Entry**: P2.3 完了
- **Work**:
  1. 現 `collect_decl` Var branch (collection.rs:268-294) で `ast::Expr::Arrow` init
     の場合、declarator の型注釈を読む
  2. 型注釈が `TsTypeRef` で参照先が callable interface (Pass 2a snapshot で
     `classify_callable_interface` == `Single|Multi`) の場合、`x` を
     `TypeDef::ConstValue { fields: [], elements: [], type_ref_name: Some(I) }`
     として registration
  3. 非 callable な interface / 関数 type alias の場合は従来通り `TypeDef::Function`
- **Exit**:
  - unit test: `const x: CallableI = arrow` → `TypeDef::ConstValue` 登録
  - unit test: `const x: NonCallableI = arrow` → `TypeDef::Function` 登録 (従来)
  - 既存 test 全件 pass
- **Rollback**: なし

### Phase 3: Widest signature computation

#### P3.1: `compute_widest_params`

- **Entry**: Phase 2 完了
- **Work**: `src/pipeline/type_converter/overloaded_callable.rs` を新規作成。
  各 position で overload ごとの型を収集し union 化 / 不在は Option wrap
- **Exit**: unit test 3 件 (same-arity different-type, different-arity, promise)
- **Rollback**: なし

#### P3.2: `compute_union_return`

- **Entry**: P3.1 完了
- **Work**: overload return type を dedup し synthetic union enum を生成。
  Promise<T> は `RustType::unwrap_promise()` (INV-6 に向けて) で unwrap 後 dedup
- **Exit**: unit test (divergent, non-divergent, all-promise)
- **Rollback**: なし

#### P3.3: `WidestSignature` 構造体と返却 API

- **Entry**: P3.2 完了
- **Work**: `struct WidestSignature { params, return_type, return_diverges }` を
  返す `compute_widest_signature` 関数
- **Exit**:
  - P3.1/P3.2 の test が引き続き pass
  - `cargo test --lib` 全件 pass (Revision 3.3 M1 — 新モジュール追加時の既存コード
    との型不整合を検出)
- **Rollback**: なし

### Phase 4: Trait emission

#### P4.1: `convert_interface_as_fn_type` を trait 化

- **Entry**: Phase 3 完了
- **Work**:
  1. `src/pipeline/type_converter/interfaces.rs::convert_interface_as_fn_type`
     (現 L139-241) の L160 `max_by_key` 削除 (F1 修正済 — 実行番号は L160)
  2. `classify_callable_interface` の結果に応じて trait 生成 (C16 補足: 既存
     `is_callable_only` **関数** は AST check として残し、その後に続く
     `type X = Box<dyn Fn(...)>` **emission path** を trait 生成に置換):
     - Single: `trait GetValue { fn call_0(&self, key: String) -> String; }`
     - Multi: 全 overload を `call_0`, `call_1`, ... として展開
  3. 既存 snapshot 書き換え: `tests/snapshots/integration_test__callable_interface.snap`
     の `type GetValue = Box<dyn Fn(...)>` / `type GetCookie = Box<dyn Fn(...)>`
     を trait 定義 + marker 構造に更新 (実装完了時に insta review)
- **Exit**:
  - 新規 fixture `callable-interface-simple-trait.input.ts` が rustc compile pass
  - 既存 `callable-interface.input.ts` の snapshot が以下形式 (C12 対応) で更新:
    ```rust
    // Single overload: GetValue
    trait GetValue { fn call_0(&self, key: String) -> String; }

    // Multi overload: GetCookie (2 overloads, divergent return)
    trait GetCookie {
        fn call_0(&self, c: String) -> String;
        fn call_1(&self, c: String, key: String) -> f64;
    }

    // その他の現 snapshot 要素 (Body struct, BodyCache type alias, getValue const)
    // は Phase 5-8 で marker struct + const として確定する
    ```
  - 生成 Rust が rustc compile pass (trait 定義段階で — 新規 simple-trait fixture)
  - **compile_test 一時除外** (Revision 3.3 C1): 既存 `callable-interface.input.ts` は
    trait 化後〜P8.2 の間 compile 不可になるため、`tests/compile_test.rs` から
    一時的にエントリを除外する。P8.2 で復帰
  - 完全な marker 構造 (struct + impls + const) は Phase 5-8 で完成
  - **INV-2 lint 注記**: `type_aliases.rs` の `call_signatures` 参照は
    `TsTypeLiteralInfo` (SWC AST 中間表現) のフィールドであり、`TypeDef::Struct` の
    `call_signatures` とは別の型。`classify_callable_interface` は `TypeDef` 用のため
    `type_aliases.rs` には適用不可。INV-2 lint は `TypeDef` レベルの直接検査のみ対象
- **Rollback**: interfaces.rs diff を discard (user 実施)

**Note (構築 signature)**: `interface Factory { new (config): Factory; name: string; }`
のような construct signature は現在も emit されていない。本 PRD では **変更なし**
(引き続き emit されない)。

#### P4.2: async overload の `is_async` 伝搬 + `unwrap_promise()` 集約

- **Entry**: P0.2 (Promise 調査) + P1.4 (is_async propagation) + P4.1 完了
- **Work**:
  1. `RustType::unwrap_promise()` method を `src/ir/types.rs` に追加 (既存なら skip。
     P0.2 調査結果に基づく)。実装は `Named { name: "Promise", type_args: [inner] }`
     → `inner`、それ以外は self passthrough
  2. Promise<T> 戻りの overload を検出
  3. trait method を `async fn`、return type を `T` に unwrap
  4. INV-6 enforcement のため `scripts/check-promise-unwrap.sh` を作成。生成 Rust
     に literal `Promise<` が残らないことを grep で確認 (C9 対応)
- **Exit**:
  - fixture `callable-interface-async.input.ts` で rustc compile pass
  - `scripts/check-promise-unwrap.sh` pass (生成 Rust に `Promise<` なし)
  - **async-class-method compile_test 復帰** (Phase 3 で skip 追加): Promise unwrap 実装後、
    `tests/compile_test.rs` の `skip_compile` / `skip_compile_with_builtins` から
    `"async-class-method"` を削除し compile pass を確認
- **Rollback**: なし

#### P4.3: `convert_callable_trait_const` 関数 skeleton + entry routing

**C4 + C5 対応**: `convert_callable_trait_const` 関数の新規作成と、arrow_fns.rs
からの呼び出し元 routing を明示的に確立する。本 phase は後続 P5-P8 で body を
埋めていくための骨格を作る。

- **Entry**: P4.2 完了
- **Work**:
  1. `src/transformer/functions/arrow_fns.rs` に以下の helper を追加:
     ```rust
     /// Returns (trait_name, type_args) if `var_rust_type` refers to a
     /// callable-interface trait (classify_callable_interface == Single|Multi).
     /// Otherwise returns None.
     fn callable_trait_name_and_args(
         &self,
         var_rust_type: Option<&RustType>,
     ) -> Option<(String, Vec<RustType>)>;
     ```
     内部で `registry.get(name)` → `classify_callable_interface(&def)` を呼ぶ
  2. `convert_callable_trait_const` 関数 skeleton を追加 (L585 P5.x / P7 / P8 で body 充実):
     ```rust
     fn convert_callable_trait_const(
         &mut self,
         value_name: &str,
         trait_name: &str,
         trait_type_args: &[RustType],
         arrow: &ast::ArrowExpr,
         vis: Visibility,
         resilient: bool,
     ) -> Result<Vec<Item>>;  // Result, not Option — hard error on failure (INV-3)
     ```
     **Skeleton は `todo!()` 不可** (Revision 3.3 M2 — test 中に panic する)。
     最小の「trait 定義 item のみ emit」とし、Phase 5 以降で marker / inner /
     delegate / const を追加していく
  3. `convert_var_decl_module_level` (P1.5 で rename 済) の arrow init path で、
     `callable_trait_name_and_args` が `Some` の場合に `convert_callable_trait_const`
     を呼ぶ routing を追加
- **Exit**:
  - unit test: `const x: Adder = (a, b) => a + b` で `convert_callable_trait_const`
    が呼ばれる (mock でも可)
  - `cargo check` pass
- **Rollback**: diff discard

### Phase 5: Marker struct + inner fn 生成

**Revision 3.3 H3 — fixture body 制限**: Phase 5〜8 の期間中、TypeResolver は
旧式の `select_overload(..., 0, &[])` を使用しており、arrow body の expected type
解決は正しくない可能性がある (P9.2 で修正予定)。この期間中の test fixture body は
以下に制限する:
- 単一 return 文 (`return value;`) または expression body
- 型注釈付き変数 (型推論に依存しない)
- narrowing / 複雑な制御フローは避ける

複雑な body のテストは Phase 11 (E2E test) に委ねる。

#### P5.1: `Transformer::used_marker_names` (INV-1)

- **Entry**: Phase 4 完了
- **Work**:
  1. `src/transformer/mod.rs::Transformer` に
     `used_marker_names: std::collections::HashSet<String>` field 追加
  2. `Transformer::for_module` で empty HashSet 初期化
  3. 構築時に衝突検出する `allocate_marker_name(&mut self, base: &str) -> String`
     method 追加: base → base1 → base2 ... で unique 化
  4. `marker_struct_name(trait_name: &str, value_name: &str) -> String` 関数を
     `src/transformer/functions/arrow_fns.rs` に定義。本体は
     `format!("{trait_name}{}Impl", to_pascal_case(value_name))`
- **Exit**:
  - unit test `marker_name_pascalcase_lowercase_value`: `const getCookie: GetCookie`
    → `GetCookieGetCookieImpl`
  - unit test `marker_name_pascalcase_short_value`: `const g1: GetCookie` →
    `GetCookieG1Impl`
  - unit test `marker_name_pascalcase_snake_value`: `const request_handler: Handler`
    → `HandlerRequestHandlerImpl`
  - unit test `marker_name_distinct_for_distinct_values`: 異なる value で異なる marker name
  - unit test `marker_name_collision_suffix_loop`: `const a: I` と `const A: I` で
    `IAImpl` と `IAImpl1` が allocate される (R4-C4 対応)
- **Rollback**: なし

#### P5.2: ZST marker struct の generator emission

**Revision 3.3 C3**: ZST derive 変更は callable interface marker **専用**。全空
struct に適用するとグローバル影響 (既存の `interface Marker {}` 等) が出るため、
`Item::Struct` に `is_unit_struct: bool` フラグを追加して判定する。

- **Entry**: P5.1 完了
- **Work**:
  1. `src/ir/item.rs::Item::Struct` に `is_unit_struct: bool` field を追加
     (default `false`、既存挙動不変)
  2. `fold.rs`, `visit.rs`, `test_fixtures.rs` で新 field 対応
  3. `src/generator/mod.rs` で `Item::Struct { is_unit_struct: true, fields: [],
     type_params: [] }` を以下の形式で emit (R2-L2-1 preserve):
     ```
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
     struct Name;
     ```
  4. `is_unit_struct: false` (既存パス) の挙動は不変
     (`#[derive(Debug, Clone, PartialEq)]\npub struct Name {..}`)
  5. callable interface marker struct の生成時 (P8.1) に `is_unit_struct: true` を set
- **Exit**:
  - generator unit test: `is_unit_struct: true` の struct が上記 derive list 付きで
    `struct Name;` 形式で emit される
  - generator unit test: `is_unit_struct: false` の空 field struct は既存形式を維持
  - 非空 field struct の既存 test 引き続き pass
  - 既存 snapshot に変更なし (`cargo test` で snapshot diff 0)
- **Rollback**: なし

#### P5.3: `Expr::StructInit` の unit struct 形式 emission

- **Entry**: P5.2 完了
- **Work**: generator で `Expr::StructInit { name, fields: [], base: None }` を
  `Name` (unit struct syntax) として emit
- **Exit**: unit test pass
- **Rollback**: なし

**Note**: 旧 P5.4 (factory method refactor) と旧 P5.5 (INV-8 lint) は Phase 0.4
(P0.4a/b/c) に移動した。旧 P5.6 (inner fn emission) は以下 P5.4 にリネーム。

#### P5.4: Inner fn emission (marker impl)

- **Entry**: P5.3 完了 + P4.3 (convert_callable_trait_const skeleton) 完了
- **Work**: `convert_callable_trait_const` 内で private `inner` method を生成。
  - widest params + widest return を使う
  - **`any_enum_override` は呼ばない** (INV-7, R4-C6 対応)
  - Method 生成時 `is_async` は `arrow.is_async` を伝搬
- **Exit**:
  - fixture `callable-interface-inner.input.ts` で inner fn の signature が
    widest 型になる snapshot (本 fixture を Test Plan table にも追加必須 — C6)
  - (trait impl 未完なので単体では compile 不可。cargo check は crate レベルで pass)
- **Rollback**: なし

### Phase 6: Return wrap (divergent returns)

**C7 + C15 + C22 対応**: P0.1 の調査結果が決定するまで、Phase 6 の詳細設計は
仮置きとする。P6.4 で P0.1 結果に応じて対応方針を決定する。**前提仮定は置かない**
(「発生しない」も「発生する」も仮定しない)。

#### P6.0: `Transformer::return_wrap_ctx` field 追加 + `spawn_nested_scope_with_wrap` factory 追加

- **Entry**: Phase 5 完了
- **Work**:
  1. `src/transformer/mod.rs::Transformer` struct に private field
     `return_wrap_ctx: Option<ReturnWrapContext>` を追加 (default None)
  2. `Transformer::for_module` で `return_wrap_ctx: None` に初期化
  3. 既存 factory method (P0.4a で作成) に wrap ctx 伝搬を追加:
     - `spawn_nested_scope` — wrap ctx を **強制的に None** に (leak 防止、INV-8)
     - `spawn_nested_scope_with_local_synthetic` — 同 None
  4. 新 factory `spawn_nested_scope_with_wrap(&mut self, ctx: ReturnWrapContext)`
     を追加。これは callable interface arrow body 専用
- **Exit**:
  - `cargo test` pass (意味論不変 — field 追加のみ、どこからも Some に set しない)
  - `scripts/check-transformer-construction.sh` (P0.4c) pass (新 field が追加された
    後も production 内の直接構築が 0 件であることを確認)
- **Rollback**: なし

#### P6.1: `ReturnWrapContext` 構造と `wrap_leaf`

- **Entry**: P6.0 完了
- **Work**: `src/transformer/return_wrap.rs` を新規作成。
  1. `ReturnWrapContext { enum_name: String, variant_by_type: Vec<(RustType, String)> }` 構造体定義
  2. `wrap_leaf` 関数の signature (R2-L2-3 preserve):
     ```rust
     fn wrap_leaf(
         ir_expr: Expr,
         ast_arg: &ast::Expr,  // 型レベル必須参照。Option<&ast::Expr> は禁止
         ctx: &ReturnWrapContext,
     ) -> Result<Expr>
     ```
  3. Error message は **SWC source span (byte 範囲) を必ず含む** (R3-L1-5 preserve):
     ```rust
     anyhow::anyhow!(
         "cannot wrap return expression at byte {span_lo}..{span_hi} — ...",
         span_lo = ast_arg.span().lo.0,
         span_hi = ast_arg.span().hi.0,
     )
     ```
  4. Polymorphic None 判定用の `variant_by_type.iter().find(...)` logic
- **Exit**:
  - unit test `variant_for_exact_match` (exact RustType match)
  - unit test `variant_for_option_narrowing_fallback` (T → Option<T> narrowing)
  - unit test `variant_for_returns_none_when_no_match`
  - unit test `polymorphic_none_unique_option_variant_picks_it` (unique Option 変異で選択)
  - unit test `polymorphic_none_zero_option_variants_returns_none` (Option 変異なしで None)
  - unit test `polymorphic_none_multiple_option_variants_refuses` (複数 Option で None 返却 = 曖昧)
  - unit test `coerce_string_literal_adds_to_string` (StringLit を `.to_string()` に変換)
  - unit test `coerce_string_literal_passthrough_for_non_literal` (非 literal は passthrough)
  - unit test `wrap_in_variant_constructs_fncall_with_user_enum_ctor`
  - unit test `build_return_wrap_context_collects_unique_variants` (enum_name 抽出 + variant 重複排除)
  - unit test `build_return_wrap_context_unwraps_promise_in_variants` (Promise<T> を T に unwrap)
  - unit test `build_return_wrap_context_dedupes_identical_returns` (同一型 overload で variant 1 個)
- **Rollback**: なし

#### P6.2: Polymorphic None 厳格化 (INV-3 派生)

- **Entry**: P6.1 完了
- **Work**: `Option<Any>` return に対して wrap context に `Option<_>` variant が
  1 個以外ある場合は hard error (silent guess 禁止)
- **Exit**: fixture `callable-interface-polymorphic-none-ambiguous.input.ts` で
  変換が error (test で `expect_err`)
- **Rollback**: なし

#### P6.3: `wrap_expr_tail` for `Expr::If` (ternary)

- **Entry**: P6.2 完了
- **Work**: ternary (`Expr::If` from Cond) の then/else branch を per-branch AST で
  再帰 wrap
- **Exit**: fixture `callable-interface-ternary-return.input.ts` で rustc compile pass
- **Rollback**: なし

#### P6.4: P0.1 結果に応じた IfLet/Match 対応

- **Entry**: P6.3 完了
- **Work**:
  - P0.1 で発生しないと判明 → `wrap_expr_tail` の IfLet/Match arm は
    `unreachable!()` で dead code marker (YAGNI)
  - P0.1 で発生すると判明 → 発生元 (narrowing 変換 / switch→match 変換) で
    wrap ctx を見て per-branch inline wrap。walker の IfLet/Match arm は
    「既に wrapped」を assume
- **Exit**: 該当 fixture で rustc compile pass
- **Rollback**: なし

#### P6.5: CLI single-file モードの synthetic items 結合 + builtin 名前衝突対策

**Phase 6 /check_problem で発見された 2 つの問題**:

**問題 1: CLI synthetic items 結合欠落**
CLI (`main.rs::transpile_file`) が `file.rust_source` を直接書き出しており、
`pipeline_output.synthetic_items` に分離された callable interface marker struct / impl
が出力に含まれない。lib API (`extract_single_output`) と `pipeline::transform_module`
は `render_referenced_synthetics_for_file` で synthetic items を結合しているが、
CLI の single-file path のみ欠落していた。

修正: `main.rs::transpile_file` に `render_referenced_synthetics_for_file` 呼び出しを
追加。3 箇所 (lib.rs, pipeline/mod.rs, main.rs) で同一の結合パターンを使用するが、
各箇所で `files` の取得方法と後続処理が異なるため DRY 共通化は coupling を増やす。
`render_referenced_synthetics_for_file` 関数自体が共通ロジックとして機能している。

**問題 2: builtin types と fixture の名前衝突**
CLI はデフォルトで `--no-builtin-types` 未指定 → `use_builtin_types = true`。
Builtin types に Web Streams API の `Transformer` interface が含まれ、
`callable-interface-param-rename.input.ts` の `interface Transformer` と名前衝突。
`TypeRegistry::merge` がビルトイン定義とユーザー定義をマージし、`methods` が非空に
なるため `classify_callable_interface` が `NonCallable` を返した。

**根本原因トレース**:
1. CLI: `build_base_registry(input_dir, use_builtin_types=true)` →
   `load_builtin_types()` → Web Streams API の `Transformer` (methods: flush/start/transform)
2. Pipeline L69: `shared_registry = input.builtin_types.unwrap_or_default()` →
   builtin `Transformer` が既に存在
3. Pipeline L72: `shared_registry.merge(&file_registry)` → ユーザーの `Transformer`
   (call_signatures のみ) が builtin の `Transformer` (methods あり) にマージ
4. マージ後: `TypeDef::Struct { methods: {flush, start, transform}, call_signatures: [1] }`
5. `classify_callable_interface`: `!methods.is_empty()` → `NonCallable`

修正: fixture の interface 名を `Transformer` → `StringMapper` に変更
(builtin と衝突しない名前)。builtin/user 名前衝突のマージ戦略改善は I-392 scope 外
(Non-Goals に記載)

- **Entry**: P6.4 完了
- **Work**:
  1. `main.rs::transpile_file` に synthetic items 結合を追加
  2. `callable-interface-param-rename.input.ts` の interface 名を `StringMapper` に変更
  3. CLI 出力と lib API 出力の一致を検証
- **Exit**:
  - `cargo test` 全テスト pass
  - CLI 出力に marker struct が含まれることを確認
  - clippy 0, fmt 0

### Phase 7: Trait delegate impl

#### 設計判断: Inner fn return wrap (Option B 採用 — empirical 検証完了)

**Empirical 検証結果** (2026-04-13):

TypeResolver が callable interface arrow body 内の return 式に対して `expr_types` に
型情報を設定しているかを検証した。

**結論: Option B (variant wrap + TypeResolver 型情報) は viable。**

根拠:
1. `visit_stmt` の `Stmt::Return` ハンドラ (`visitors.rs:539-553`):
   `resolve_expr(arg)` → `expr_types[span] = Known(ty)` で確実に格納
2. `resolve_arrow_expr` (`fn_exprs.rs:114-211`): arrow パラメータを
   `declare_var(name, type, span)` でスコープに登録し、body を `visit_stmt` で walk
3. `resolve_expected_fn_info` (`fn_exprs.rs:72-98`): callable interface の
   `call_signatures` からパラメータ型を抽出し、arrow params に伝搬
4. `Expr::Ident("c")` の場合: `resolve_expr_inner` → `lookup_var("c")` →
   スコープスタックからパラメータ型 (`String`) を返却 → `expr_types[span_of_c] = String`
5. 既存テスト `test_callable_interface_return_type_propagated_to_arrow` が
   arrow body 内の式の型解決を証明

**Option A は不可能** (Rust の型システムで 1 fn に複数 return type を持てない)。

#### 採用アーキテクチャ: 二相分離アプローチ

変換パイプライン (`convert_stmt`, `convert_expr`, `spawn_nested_scope`) を変更せず、
return wrapping を独立した pre/post processing として実装する。

**Phase A (型収集)**: 変換前に SWC arrow body を walk し、全 return leaf 式の型を
`FileTypeResolution::expr_types` から span ベースで収集。
`Vec<(Option<RustType>, (u32, u32))>` (型 + span for error reporting) として保持。

**Phase B (IR wrapping)**: 変換後の IR body を walk し、return/tail leaf 式を
Phase A で収集した型で positionally マッチして variant wrap。

**Positional invariant**: SWC と IR の return leaf 式は depth-first order で同一順序。
Transformer は文の構造と return の順序を保存するため、この不変条件は成立。

##### wrap_leaf の variant 決定優先順位

```
1. Polymorphic None (is_none_expr → unique_option_variant)
2. Literal inference (infer_variant_from_expr — 既存ロジック)
3. TypeResolver 型 (expr_type → variant_for) ← NEW
4. Single non-Option variant fallback
5. Hard error (variant 決定不能)
```

##### Phase 6 インフラの変更

| 対象 | 変更 |
|------|------|
| `wrap_leaf` (return_wrap.rs:108) | `ast_arg: &ast::Expr` → `expr_type: Option<&RustType>`, `span: Option<(u32, u32)>` |
| `wrap_body_returns` (arrow_fns.rs:440) | `arrow: &ArrowExpr` → `types: Iterator<Item = ReturnLeafType>`、再帰的 Stmt::If/IfLet walk 追加 |
| `wrap_expr_tail` (arrow_fns.rs:467) | `ast_arg: &ast::Expr` → `types: Iterator<Item = ReturnLeafType>` |
| 新規: `collect_return_leaf_types` | SWC arrow body を walk し return leaf 式の型を収集 |

##### このアプローチの利点

- **変換パイプライン無変更**: `convert_stmt`, `convert_expr`, `spawn_nested_scope` を変更しない → INV-8 と矛盾しない
- **IR 無変更**: Pipeline Integrity 原則準拠 (IR に ephemeral な型注釈を追加しない)
- **テスト容易**: 型収集と wrapping が独立した pure function でそれぞれ unit test 可能

##### delegate method のフロー

inner fn が enum を返すため、delegate は match で unwrap:
```rust
fn call_0(&self, c: String) -> String {
    match self.inner(c, None) {
        F64OrString::String(v) => v,
        _ => unreachable!(),
    }
}
fn call_1(&self, c: String, key: String) -> f64 {
    match self.inner(c, Some(key)) {
        F64OrString::F64(v) => v,
        _ => unreachable!(),
    }
}
```

#### P7.0: Inner fn return wrap infrastructure — 完了 (2026-04-13)

- **Entry**: Phase 6 完了 + 設計判断完了
- **Work**:
  1. `collect_return_leaf_types(arrow, type_resolution)` 新規作成 (return_wrap.rs):
     SWC arrow body を depth-first walk し、全ブロック構造 (If/Switch/For/ForIn/ForOf/
     While/DoWhile/Try/Labeled) を再帰的に走査。return leaf 式の型を
     `FileTypeResolution::expr_types` から収集。ternary branches も再帰的に展開
  2. `wrap_leaf` signature 変更: `(ir_expr, ast_arg, ctx)` →
     `(ir_expr, expr_type: Option<&RustType>, span: Option<(u32, u32)>, ctx)`。
     variant 決定に TypeResolver 型を追加 (priority 3)
  3. `wrap_expr_tail` 変更: `(expr, ast_arg, ctx)` →
     `(expr, types: &mut Iterator, ctx)`。types から型を消費して wrap_leaf に渡す。
     iterator 枯渇時は即 Err (positional invariant 違反検知)
  4. `wrap_body_returns` 変更: `(stmts, arrow, ctx)` →
     `(stmts, types: &mut Iterator, ctx)`。再帰的に全ブロック構造
     (If/IfLet/While/WhileLet/ForIn/Loop/Match/LabeledBlock) を walk
  5. `convert_callable_trait_const` に統合: ReturnWrapContext 構築 →
     collect_return_leaf_types → arrow 変換 → wrap_body_returns 適用
  6. `#[allow(dead_code)]` 削除: return_wrap.rs の `#![allow(dead_code)]`、
     overloaded_callable.rs の `#![allow(dead_code)]` を削除
- **Exit**: unit test 7 件 (collect 5 件 + wrap_leaf 2 件) +
  `callable-interface-inner` fixture で divergent return が variant wrap される
- **Rollback**: なし

##### P7.0 レビュー対応 (2026-04-13)

- **C-1**: SWC/IR 両側の return 走査に全ブロック構造 (for/while/try/match/labeled 等)
  を追加。初期実装は If/Block/Switch のみで、for/while/try 内の return で
  positional invariant が破れるリスクがあった
- **I-1**: types iterator 枯渇時のサイレント fallback を即 Err に変更。
  positional invariant 破壊時に silent wrong wrapping ではなく明示的エラーで検出
- **finally 内 return**: SWC 側で `try_stmt.finalizer` の collect を除外。
  IR 側では finally body が `scopeguard::guard` クロージャ内に封入されるため
  `wrap_body_returns` が walk せず位置不一致になるため
- **SeqExpr**: SWC 側の `ast::Expr::Seq` collect を除外。IR に Seq variant がなく
  Transformer で変換エラーになるため collect が無意味

##### P7.0 スコープ外事項

- ~~**`return_wrap_ctx` field / `spawn_nested_scope_with_wrap` method の削除**~~:
  **Phase 9A で解決済**。不要と判断し削除完了
- **for-of ループ変数の TypeResolver 型解決不足**: `for (const item of items)` の
  `item` の型が TypeResolver で `Unknown` になるケースがある (配列要素型の推論が未対応)。
  callable interface arrow body で for-of ループ変数を return する場合、wrap_leaf の
  priority 3 (TypeResolver 型) がスキップされ priority 4 (single non-Option fallback)
  に fall through する。根本修正は TypeResolver の for-of 要素型推論 (別イシュー)。
  現時点では fallback で正しい variant が選ばれるケースが大半

#### P7.1: per-overload delegate method + inner 呼び出し — 完了 (2026-04-13)

- **Entry**: P7.0 完了 (inner fn body が enum variant を返す状態)
- **Work**:
  - `build_delegate_impl` + `build_delegate_method` を arrow_fns.rs に新規作成 (Result 型)
  - `Item::Impl { for_trait: Some(TraitRef) }` で trait impl を生成
  - Non-divergent (同一 return type): `self.inner(args...)` 直接返却
  - Divergent (異なる return type): `match self.inner(args...) { Variant(v) => v, _ => unreachable!() }`
  - Overload arity 超のパラメータは `None` を渡す
  - Optional 化されたパラメータは `Some(arg)` で wrap
  - `ReturnWrapContext::variant_for` を `pub(crate)` に変更
  - `variant_for` 失敗時は explicit error (`unwrap_or("Unknown")` を排除)
- **Exit**: 全 4 callable-interface fixture で delegate impl が正しく生成
- **Rollback**: なし

#### P7.2: inner arg wrapping (Some / variant ctor) — 完了 (2026-04-13)

- **Entry**: P7.1 完了
- **Work**:
  - `wrap_delegate_arg` 関数を新規作成: overload 型と widest 型の差分に応じて
    bare arg / `Some(arg)` / `EnumName::Variant(arg)` / `Some(Variant(arg))` を選択
  - `build_delegate_method` の arg 構築を `wrap_delegate_arg` に委譲
- **Exit**: unit test 4 件 (bare / Some / variant / Some+Variant)
- **Rollback**: なし

#### P7.3: async 伝搬 for delegate method — 完了 (2026-04-13)

- **Entry**: P4.2 (trait async) + P7.2 完了
- **Work**:
  - async inner fn の delegate 呼び出しに `.await` を追加 (`Expr::Await`)
  - `compute_union_return` で Promise unwrap してから union 作成 (async divergent return の
    enum 名が `PromiseF64OrPromiseString` → `F64OrString` に修正)
  - inner fn return type の冗長な `if arrow.is_async` 条件を簡潔化
    (`compute_union_return` が先に unwrap するため不要になった)
  - `callable-interface-async.input.ts` 新規 fixture (single + multi-overload async)
- **Exit**: callable-interface-async fixture で async trait + async delegate が正しく生成
- **Rollback**: なし

##### P7.1-P7.3 レビュー対応 (2026-04-13)

- **Pipeline Integrity 違反修正**: `CallTarget::Free("Some")` → `BuiltinVariant::Some`、
  `CallTarget::Free("Enum::Variant")` → `UserEnumVariantCtor { enum_ty, variant }`。
  `wrap_in_variant`, `wrap_leaf` (polymorphic None), `wrap_delegate_arg`, `infer_variant_from_expr`
  の全箇所を修正。walker が user type ref を正しく追跡し import 生成に必要な型参照が
  登録されるようになった
- **`infer_variant_from_expr` パターンマッチ修正**: 構築側を `BuiltinVariant::Some` に
  変更した際、マッチ側が `Free("Some")` のまま残っていたバグを修正。
  **全 5 分岐** (StringLit, NumberLit, BoolLit, Some, Unknown) の unit test を追加し、
  同種のバグが今後検出されるようにした
- **`compute_union_return` Promise unwrap**: async overload の union enum 名が
  Promise-wrapped 型名になるバグを修正

##### Phase 7 スコープ外事項

- **`return_wrap_ctx` field / `spawn_nested_scope_with_wrap` method の削除**
  (`src/transformer/mod.rs:44-49`, `mod.rs:100-116`):
  **Phase 9A で解決済**。不要と判断し削除完了
- **Promise unwrap + Unit 除去パターンの DRY 化**: `.map(|ty| ty.unwrap_promise()).and_then(|ty| if Unit then None else Some)` パターンが 3 箇所に存在
  (`convert_callable_trait_const` inner return type、`build_delegate_method` delegate return type、
  `convert_callable_interface_as_trait` trait method return type)。各箇所でコンテキストが
  微妙に異なり、共有すると結合度が上がるため現時点では許容。Phase 9 以降の refactoring で
  `RustType::unwrap_promise_to_return_type()` convenience method として統合を検討
- **生成コードの match arm インデント**: snapshot 上 match arm が column 0 に配置される
  (generator の formatting 問題)。機能的影響なし、Phase 12 (L2/L3/L4 fix) で対応検討
- **non-async arrow with `Promise<T>` return type**: TypeScript では `async` キーワードなしで
  `Promise<T>` を返す関数が書ける。現在の trait 生成は `Promise<T>` → `async fn -> T` と
  一律変換するため、non-async arrow の場合に trait impl が `async fn` を要求する不整合が
  生じる。これは callable interface 固有ではなく、trait 生成の一般的な設計課題 (別イシュー)

### Phase 8: Const instance emission

#### P8.1: `Item::Const` emission from `convert_callable_trait_const`

- **Entry**: Phase 7 完了
- **Work**: `convert_callable_trait_const` の末尾で `Item::Const { ty: MarkerName,
  value: Expr::StructInit { name: MarkerName, fields: [] } }` を emit
- **Exit**: fixture で `const getCookie: GetCookieGetCookieImpl = GetCookieGetCookieImpl;`
  が出力される
- **Rollback**: なし

#### P8.2: 変換側統合チェックポイント (Revision 3.3 H1)

trait + marker + inner + delegate + const の全要素が揃った時点の end-to-end 検証。
Phase 9 (Generic) 以降で発見される問題が「generic 固有」か「基本構造の欠陥」かを
切り分けるための checkpoint。

- **Entry**: P8.1 完了
- **Work**:
  1. 既存 `callable-interface.input.ts` を変換し、生成 Rust が rustc compile pass
     であることを確認
  2. `callable-interface-inner.input.ts` (divergent return) を変換し、
     rustc compile pass 確認
  3. P4.1 で一時除外した `tests/compile_test.rs` の callable-interface エントリを
     **復帰** (既存 + P4.1〜P8.1 で追加した全 fixture):
     callable-interface, callable-interface-param-rename, callable-interface-inner,
     callable-interface-async, call-signature-rest, interface-mixed
  4. `cargo test --lib` 全件 pass
  5. 生成 Rust 内に `Box<dyn Fn(` パターンの callable interface 型が残っていない
     ことを grep 確認
- **Exit**:
  - 上記 1-5 全件 green
  - `tests/compile_test.rs` に callable-interface 関連 fixture が全件登録済
  - 変換側 (trait 定義 → marker → inner → delegate → const) が non-generic ケースで
    end-to-end 動作
  - **`cargo test` (全テスト) pass** — `cargo test --lib` ではなく全テストで regression 確認
- **Rollback**: なし (検証のみ。問題発見時は該当 phase に戻って修正)

### Phase 9: Generic callable interface

#### Phase 9 前提: `return_wrap_ctx` / `spawn_nested_scope_with_wrap` 削除 — **完了**

P7.0 で二相分離アプローチを採用し、scope-based wrapping は不要と確定。
`return_wrap_ctx` field と `spawn_nested_scope_with_wrap` method を削除済。
全 Transformer 構築サイト (production + test 13 箇所) から `return_wrap_ctx: None` を除去。
production code の `#[allow(dead_code)]` は 0 件。

#### P9.1: arity validation (INV-4)

- **Entry**: P8.2 (統合チェックポイント) 完了
- **Work**:
  1. `trait_type_args.len() != trait_type_params.len()` の場合は hard error
  2. **error-case fixture の compile_test 対応**: `callable-interface-generic-arity-mismatch.input.ts`
     は意図的に変換 error を発生させる。`tests/compile_test.rs` は `transpile_collecting`
     の Err で panic するため、`skip_compile` / `skip_compile_with_builtins` 両方に
     `"callable-interface-generic-arity-mismatch"` を追加する。
     (P8.2 で `"callable-interface"` prefix を除去済のため、prefix match では skip されない)
- **Exit**: fixture で変換 error (unit test で `expect_err`)、compile_test pass
- **Rollback**: なし

#### P9.2: `resolve_fn_type_info` を widest ベースに書き換え (C2 先行)

**C2 対応**: 旧 P9.3 を先行させる。`resolve_fn_type_info` の select_overload 呼び出し
を先に撤廃してから、P9.3 で substitution を widest 結果に積む。

- **Entry**: P9.1 完了
- **Work**:
  0. **Caller 全列挙** (Revision 3.3 M3、P8.2 時点で empirical 確認済):
     `resolve_fn_type_info` の caller は production 3 箇所 + test 1 箇所:
     - `fn_exprs.rs:93` (arrow expected type)
     - `call_resolution.rs:46` (Named type → return type lookup)
     - `call_resolution.rs:160` (call site parameter type resolution)
     - `tests/expected_types/callback_fallback.rs:293` (test)
  1. 現 `helpers.rs:289-327` の `resolve_fn_type_info` は
     `select_overload(call_signatures, 0, &[])` と arg_count=0 hardcoded (L317)。
     これは callable interface の arrow body に対する expected type 計算としては
     根本的に誤り (arrow body は特定 overload 向けではなく widest 向けのため)
  2. `resolve_fn_type_info` の signature に `synthetic: &mut SyntheticTypeRegistry`
     引数を追加 (F3 対応 — `compute_widest_signature` が synthetic を要求するため)。
     **設計注記**: TypeResolver context (`TypeResolverVisitor`) は
     `self.synthetic: &mut SyntheticTypeRegistry` を保持している (visitors.rs)。
     `fn_exprs.rs:93` と `call_resolution.rs` の caller から `synthetic` を伝搬する
     経路を確立する必要がある。`resolve_fn_type_info` が `pub(super)` のため
     TypeResolver module 内でのみ呼ばれ、全 caller が synthetic を保持するため
     伝搬は straightforward
  3. `TypeDef::Struct { call_signatures, .. }` の case で
     `compute_widest_signature(call_signatures, synthetic)` を呼び、widest の
     params / return_type を返す (INV-7 の widest 一貫性)
  4. `select_overload(..., 0, &[])` 呼び出しを **完全撤去**
  5. **INV-2 lint 部分解消**: `helpers.rs:316` の `!call_signatures.is_empty()` 判定が
     `compute_widest_signature` 呼び出しに置換されることで 1 件解消。
     他の violation (ts_type_info 3 件、intersection 1 件、type_aliases 1 件、
     registry/mod.rs 1 件) は `TsTypeLiteralInfo` / `TypeDef` merge ロジック等の
     別 context のため P9.2 scope 外。lint script の `exit 0` → `exit 1` 変更は
     全 violation 解消後 (Phase 13 または follow-up) に実施
  6. ~~`overloaded_callable.rs` の `#![allow(dead_code)]` 削除~~: **P7.0 で削除済**。
     skip
  7. **INV-6 完全達成: 既存 Promise unwrap 関数を `RustType::unwrap_promise()` に置換**:
     - `unwrap_promise_and_unit()` (`src/pipeline/type_resolver/helpers.rs:233`) →
       `RustType::unwrap_promise()` + Unit filter に分解。呼び出し元
       (`fn_exprs.rs:61,95`, `visitors.rs:89`) を更新
     - `unwrap_promise_type()` (`src/transformer/functions/helpers.rs:33`) →
       `RustType::unwrap_promise()` に置換。呼び出し元 (`functions/mod.rs:29,113`) を更新
     - 置換後、既存 standalone 関数 2 つを削除
     - **Exit 追加**: `grep -rn 'unwrap_promise_type\|unwrap_promise_and_unit' src/` ヒット 0 件
- **Exit**:
  - 既存 test pass
  - 新 unit test: multi overload callable interface の arrow body が widest 型で
    resolve される (single overload でも同じ path)
  - `scripts/check-classify-callable-usage.sh` で `helpers.rs` の violation が消滅
    していることを確認 (他 module の violation は P9.2 scope 外のため残存許容)
  - ~~`overloaded_callable.rs` に `#![allow(dead_code)]` がないこと~~ — P7.0 で削除済
  - `grep -rn 'unwrap_promise_type\|unwrap_promise_and_unit' src/` ヒット 0 件 (INV-6)
- **Rollback**: なし

#### P9.3: type substitution の単一 helper 化 (旧 P9.2 を後置)

- **Entry**: P9.2 完了
- **Work**:
  1. `apply_type_substitution(sig: &MethodSignature, params: &[TypeParam], args: &[RustType])
     -> MethodSignature` helper を単一定義
  2. 以下の箇所から呼び出し:
     - `arrow_fns.rs::convert_callable_trait_const` (widest signature に substitution)
     - `arrow_fns.rs::build_delegate_impl` / `build_delegate_method`
       (P7.1 で追加。generic 引数を delegate method の params/return に反映)
     - `calls.rs::try_convert_callable_trait_call` (call 時の overload 選択前)
     - `helpers.rs::resolve_fn_type_info` (arrow body expected type 計算時 —
       P9.2 で widest を返した結果にさらに substitution を積む)
- **Exit**: fixture `callable-interface-generic.input.ts` で concrete 型が substitute
  され rustc compile pass
- **Rollback**: なし

#### P9.4: `select_overload` Stage 2 の評価と修正 (D3 対応)

- **Entry**: P9.3 完了
- **Work**:
  1. 現 `select_overload` (`mod.rs:176-232` — F2 修正済) の Stage 2 "all return types
     identical → return first" を評価
  2. Void-only multi-overload (例: `Logger { (msg): void; (msg, meta): void }`)
     で 2-arg call が常に overload 0 に dispatch される silent bug を再現確認
  3. Stage 2 を削除し、Stage 3 (arity filter) を最初に走らせる
- **Exit**:
  - 新 unit test: void-only multi-overload で arg 数に応じた正しい overload が選択
  - 既存 test 全件 pass
- **Rollback**: なし

### Phase 10: Call site dispatch

#### P10.1: `try_convert_callable_trait_call` 新規

- **Entry**: Phase 9 完了
- **Work**: `src/transformer/expressions/calls.rs` に `try_convert_callable_trait_call`
  を追加。Ident call の callee を `classify_callable_interface` で判定、
  callable なら `Expr::MethodCall { object: Ident, method: call_N, args }`
- **Exit**: fixture で `getCookie(ctx, "k")` が `getCookie.call_1(...)` になる
- **Rollback**: なし

#### P10.2: `select_overload` integration at call site

- **Entry**: P10.1 完了
- **Work**: call site で arg count + type から overload を select (P9.4 の
  修正版 `select_overload` を呼ぶ)
- **Exit**: fixture `callable-interface-overload-select-*.input.ts` 3 ケース
- **Rollback**: なし

#### P10.3: Fallthrough symmetry 確認

- **Entry**: P10.2 完了
- **Work**: `classify_callable_interface` が conversion 側と call 側で同じ判定を
  返すことを unit test で確認。`convert_callable_trait_const` の error は hard
  error (fallthrough 禁止)
- **Exit**: fixture で error path の `expect_err` test
- **Rollback**: なし

### Phase 11: Integration + coverage

#### P11.1: compile_test 確認 (INV-9) + E2E test

- **Entry**: Phase 10 完了
- **Work**:
  1. **compile_test 自動包含の確認**: P8.2 で `"callable-interface"` prefix を
     `skip_compile` / `skip_compile_with_builtins` 両リストから除去済のため、
     Phase 9-10 で追加した全 callable-interface-*.input.ts fixture は自動的にテスト対象。
     ただし以下の error-case fixture は P9.1 で個別 skip 追加済:
     - `callable-interface-generic-arity-mismatch` (変換 error を意図)
     `cargo test --test compile_test` 全件 pass を確認
  2. **E2E test 追加** (R3 test addition + R4-L3-5 gap 解消):
     - `tests/e2e/scripts/callable_interface.ts` を作成。`function main(): void` で
       divergent return, generic, async 等の callable interface 使用例を全て含み
       `console.log` で observable output を生成。
       **Phase 5-8 で単純化した fixture body** (callable-interface-inner,
       callable-interface-async) の divergent return multi-path テストを
       E2E test でカバーする
     - `tests/e2e_test.rs` に `test_e2e_callable_interface_ts_rust_stdout_match`
       関数を追加、`run_e2e_test("callable_interface")` を呼ぶ
     - TS 実行 (`tsx`) と変換 Rust 実行 (`cargo run`) の stdout が完全一致することを
       確認
- **Exit**:
  - `cargo test --test compile_test` 全件 pass
  - `cargo test --test e2e_test test_e2e_callable_interface_ts_rust_stdout_match`
    pass
- **Rollback**: なし

#### P11.2: Hono 4 callable interface の動作確認

- **Entry**: P11.1 完了
- **Work**:
  1. `./scripts/hono-bench.sh` 実行
  2. GetCookie / GetSignedCookie / SetHeaders / SetMetric 関連の conversion 結果を
     観察。それぞれ個別 error がないか確認
  3. 現 bench baseline (pre-I-392 state) と比較して regression 0
- **Exit**: Hono bench regression 0 (clean/error 数が同等以上)
- **Rollback**: なし (変更なし、測定のみ)

### Phase 12: L2/L3/L4 real 項目の fix (Phase 13 から繰上)

**C13 対応**: 旧 Phase 13 (L2/L3/L4 fix) を先行させ、旧 Phase 12 (quality gate) を
後置する。Code 変更が quality gate 後に入るのを防ぐ。

- **Entry**: Phase 11 完了 + P0.3 の verification 結果 (確定済)
- **Work**: P0.3 の結果: real bug **1 件のみ** (L2-4: generator match indent cosmetic)。
  5 件以下のため scope cap なし (Revision 3.3 H4)。各 real 項目を個別 sub-phase で fix:
  - 各 sub-phase は「fact → fix → exit test」の 1 task 1 check 単位
  - real 6 件以上の場合は P0.3 Exit で scope 決定済
- **Exit**: scope 内の全 real 項目が fix 済、`cargo test --lib` pass
- **Rollback**: 各 sub-phase 単位で diff discard

### Phase 13: Final Quality gate

- **Entry**: Phase 12 完了
- **Work**:
  1. `cargo test` 全件 pass
  2. `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
  3. `cargo fmt --all --check` 0 diff
  4. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` pass
  5. Coverage が 89 + 2 以上なら threshold を 1 上げる (CLAUDE.md ratchet ルール)
  6. `scripts/check-classify-callable-usage.sh` pass (INV-2 lint)
  7. `scripts/check-transformer-construction.sh` pass (INV-8 lint)
  8. `scripts/check-promise-unwrap.sh` pass (INV-6 lint)
  9. Production 内 `Transformer\s*\{` 0 ヒット確認 (invariant 最終確認)
  10. `#[allow(dead_code)]` が production code に残っていないこと確認
      (`grep -rn 'allow(dead_code)' src/ --include='*.rs' | grep -v tests`)。
      Phase 9A で `return_wrap_ctx` / `spawn_nested_scope_with_wrap` を削除済のため
      残存 0 件のはず
  11. `async-class-method` が `compile_test.rs` の skip リストから完全除外されていること確認
      (P4.2 で `skip_compile` から復帰済、P8.2 で `skip_compile_with_builtins` からも復帰済)
  12. INV-6 完全達成確認: `grep -rn 'unwrap_promise_type\|unwrap_promise_and_unit' src/`
      ヒット 0 件 (P9.2 で置換済のはず)
  13. INV-8 lint が引き続き pass していること確認 (`scripts/check-transformer-construction.sh`)。
      Phase 9A で `return_wrap_ctx` 削除済のため、INV-8 の目的は「新 field 追加時の factory
      method 強制」に変化。lint script 自体は不変
- **Exit**: 全 quality gate clean
- **Rollback**: なし

## 発見された課題一覧 (本 PRD 作業中に発見、scope 内外問わず記載)

本 section は `@feedback_no_silent_scope_reduction.md` / `@feedback_investigation_reports.md`
に従い、発見した課題を全て記録する。

### A. Round 4 (deep deep deep) で empirical に確認された Critical 問題

| ID | 問題 | verification 結果 | 本 PRD での phase |
|---|---|---|---|
| R4-C1 | convert_callable_trait_const fallthrough 片側非対称 | rustc E0599 確認 | INV-2/3 + Phase 10.3 で解決 |
| R4-C2 | expression body arrow で R4-C1 と同様 | rustc E0599 確認 | 同上 |
| R4-C3 | 非 callable const 含む全 non-arrow const の silent drop | rustc E0425 確認。**pre-existing gap**、L1-5 とは無関係 | **Phase 1.5** (transformer 側 = `convert_var_decl_module_level` rename + **Lit のみ対応**, Revision 3.3 C2) + Phase 2.4 (registry 側)。Call/Ident は follow-up PRD |
| R4-C4 | PascalCase collision で同名 marker struct 生成 | rustc E0428 + E0119 確認 | Phase 5.1 で解決 |
| R4-C5 | generic arity mismatch で free TypeVar 残存 | rustc E0425 + E0107 確認 | Phase 9.1 で解決 |
| R4-C6 | any_enum_override vs widest の型 mismatch | rustc E0425 確認 | Phase 5.4 (INV-7, 旧 P5.6) で解決 |
| R4-C7 | async callable interface で Method::is_async 欠如 | rustc E0425 確認 | Phase 1.2 (Method::is_async 追加) + Phase 1.3 (generator async keyword) + Phase 1.4 (propagation) + Phase 4.2 (Promise unwrap) + Phase 7.3 (delegate async) で解決 |

証拠: `report/i392-round4-verification.md`

### A'. Round 1-3 (/check_job / deep / deep deep) の既存 fix と Revision 3 での扱い

Revision 1 実装中に Round 1 → Round 2 → Round 3 の review を経て、以下の問題が発見 → fix されている。
Revision 3 では同じ問題を再導入しないよう、各 fix を **preserve / restructure / reverse** のいずれかで
明示的に扱う。**Reversal は Round 4 で silent bug の原因と判明したための意図的な巻き戻し**。

#### A'-1. Round 3 Critical (Tier 1 silent bug)

| ID | Round 3 で発見 / fix | Revision 3 での扱い | 対応 phase |
|---|---|---|---|
| R3-C1 | `return_wrap_stack` field の全 nested-scope leak (nested fn / class method / fn expr 他 13 サイト) を factory method 経由構築で構造的解決 | **Preserve**: 本 PRD 全体で factory method 経由のみ許可 (INV-8) | **Phase 0.4a + 0.4b + 0.4c** (旧 P5.4/P5.5 から移動、C1 対応) |
| R3-C2 | Polymorphic None ambiguity: 複数 `Option<_>` variant で `return null/undefined` は first-match heuristic ではなく hard error | **Preserve**: INV-3 で hard error 維持 | Phase 6.2 |

#### A'-2. Round 1-3 L1 fixes (Design foundation)

| ID | Round で発見 / fix | Revision 3 での扱い | 対応 phase |
|---|---|---|---|
| R1-L1-1 | Generic callable interface (`interface Mapper<T,U>` の type substitution を marker / inner / trait impl 全適用) | **Preserve** + **Restructure** (3 サイトを単一 helper `apply_type_substitution` に集約) | Phase 9.1 (arity validation) + Phase 9.3 (substitution 単一 helper。C2 対応で旧 P9.2 → P9.3 に swap) |
| R2-L1-2 | TypeResolver fallthrough recovery: `apply_return_wrap` の `as any` 起因 error を "I-392:" prefix で catch、`convert_callable_trait_const` で plain fn path に fallthrough | **REVERSAL**: Round 4 で R4-C1/C2 silent bug の root cause と判明。INV-3 で fallthrough 全面禁止、hard error に統一。L2-R3 の string matching 回避も自動達成 | Phase 10.3 (symmetry 確認) |
| R2-L1-3 | `wrap_expr_tail` の `Match` / `IfLet` recursion + AST threading: ternary だけでなく match expression body の各 arm を個別 wrap | **部分 Preserve + empirical 確認必要**: Phase 6.3 で Expr::If (ternary) は preserve。Match/IfLet は P0.1 で発生の有無を empirical 調査。発生する場合は emission 時 inline wrap、発生しない場合は dead code 削除 (YAGNI) | P0.1 + Phase 6.4 |
| R3-L1-4 | Pass 2a/2b の `reg.clone` を 2 回 → 1 回に削減 (L1-5 optimization)。Pass 1 snapshot を共有、callable check を `is_interface` のみに緩和 | **REVERSAL**: Revision 3 は Pass 2a 完了後の snapshot を使う (Pass 1 snapshot 禁止)。L1-5 relaxation は「機能と最適化の混在」の典型例で、前 session の非収束の原因の 1 つ | Phase 2.3 |
| R3-L1-5 | Error message に SWC source span (byte 範囲) を含む診断改善 | **Preserve**: Phase 6.1 の wrap walker error に span を含める要件として明記 | Phase 6.1 Exit |

#### A'-3. Round 1-3 L2/L3 fixes (design polish)

| ID | Round で発見 / fix | Revision 3 での扱い | 対応 phase |
|---|---|---|---|
| R2-L2-1 | ZST marker struct → `struct Name;` 形式 + `Copy/Eq/Hash` derive (Rust 慣習) | **Preserve**: Phase 5.2 で `struct Name;` 形式、derive list を `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` と明記 | Phase 5.2 |
| R2-L2-2 | `Expr::StructInit` empty fields → `Name` 形式 (unit struct と整合) | **Preserve** | Phase 5.3 |
| R2-L2-3 | `wrap_leaf` の `ast_arg: &ast::Expr` (型レベル必須参照、`Option<&ast::Expr>` を廃止) | **Preserve**: Phase 6.1 で signature を `fn wrap_leaf(ir_expr: Expr, ast_arg: &ast::Expr, ctx: &ReturnWrapContext) -> Result<Expr>` と明記 | Phase 6.1 |
| R2-L2-4 | `unwrap_promise` を `RustType::unwrap_promise()` method に集約 (DRY 違反解消) | **Preserve**: INV-6 | Phase 4.2 + Phase 7.3 |
| R2-L2-5 | doc/impl 整合 + visibility 適正化 (多数) | **暗黙的**: 各 phase 完了時の `cargo clippy` + `cargo fmt` + `cargo doc` で自動検証。明示 phase なし、最終的には **Phase 13 Final Quality gate** の clippy/fmt check で enforce される (C10 解消) | Phase 13 で自動 check |

#### A'-4. Round 3 test additions

前 Revision 1 で追加された test 群。Revision 3 でも作成する必要あり。

| Test | Revision 3 での扱い | 対応 phase |
|---|---|---|
| `tests/fixtures/callable-interface-divergent.input.ts` | Test Plan 表に記載済 | Phase 7.1 |
| `tests/fixtures/callable-interface-expr-body.input.ts` | Test Plan 表に記載済 | Phase 6.3 |
| `tests/fixtures/callable-interface-generic.input.ts` | Test Plan 表に記載済 | Phase 9.3 |
| `tests/fixtures/callable-interface-polymorphic-none-ambiguous.input.ts` | Test Plan 表に記載済 | Phase 6.2 |
| `tests/e2e/scripts/callable_interface.ts` + `test_e2e_callable_interface_ts_rust_stdout_match` | **追加必須**: E2E test で TS 実行 stdout と変換 Rust 実行 stdout の match 確認。R4-L3-5 の E2E coverage gap も併せて解消 | Phase 11.1 |
| `marker_struct_name` unit test (pascal case 変換 + collision suffix loop) | **追加必須** | Phase 5.1 Exit |
| `build_return_wrap_context` unit test (enum_name 抽出、variant_by_type 生成、Promise unwrap) | **追加必須** | Phase 6.1 Exit |
| `Item::Const` generator unit test (IR → Rust source round-trip) | Phase 1.1 Exit で記載済 | Phase 1.1 |
| `polymorphic_none_*` unit tests 3 件 (zero / unique / multiple option variants) | **追加必須** | Phase 6.2 Exit |
| `test_callable_interface_polymorphic_none_ambiguous_errors` (変換 error の guard test) | Phase 6.2 で `expect_err` test として記載済 | Phase 6.2 |

### B. Round 4 で発見された L2/L3/L4 (verification 未完)

本 PRD Phase 0.3 で verification を完了させる。現時点では claim のみ記録:

#### L2 (未 verify)

- **R4-L2-1**: `wrap_expr_tail` の `IfLet`/`Match` branch (P0.1 + P6.4 で対応)
- **R4-L2-2**: Transformer の直接構築サイト **12** production + 10+ test
  (Phase 0.4a/b/c で factory 化 + lint。**10 → 12 に修正、F4 対応**)
- **R4-L2-3**: `msg.contains("I-392:")` string matching error discrimination
  (INV-2/3 で fallthrough 自体を禁止するため自動解消予定 → P0.3 で auto-solved
  として確認)
- **R4-L2-4**: generator match indent propagation (P0.3 で verify、real なら
  Phase 12 (旧 Phase 13) で fix)

#### L3 (未 verify)

- **R4-L3-1**: arrow type_params handling
- **R4-L3-2**: error message context 情報
- **R4-L3-3**: marker suffix loop の実装
- **R4-L3-4**: compile_test.rs coverage gap
- **R4-L3-5**: E2E test coverage gap

#### L4 (未 verify)

- **R4-L4-1**: nested wrap での string coerce depth
- **R4-L4-2**: delegate method の unreachable pattern 冗長性
- **R4-L4-3**: variant enum name fallback

### C. Pre-existing gap (I-392 より前から存在するが本 PRD 作業中に発見)

- **`arrow_fns.rs:28-32` (現 state)**: `convert_var_decl_arrow_fns` が non-arrow init
  を skip。R4-C3 の root cause。git log で pre-existing 確認済。I-392 では Phase 1.5
  で Lit init 対応 + Phase 2.4 で callable interface arrow init の型注釈 consume。
  Call/Ident 等の非 arrow init は本 PRD scope 外 (follow-up PRD, Revision 3.3 C2)
- **`resolve_fn_type_info` (helpers.rs:289-327) の arg_count=0 固定 bug**: pre-existing
  bug。本 PRD Phase 9.3 で修正
- **`select_overload` Stage 2 の void-only multi-overload bug**: pre-existing bug。
  本 PRD Phase 9.4 で修正
- **Transformer 直接構築サイト 12 production + 10+ test** (F4 修正済 — 10 → 12):
  pre-existing。I-392 で新 field 追加時に invariant leak の risk があるため
  Phase 0.4 (旧 P5.4/P5.5 から移動) で対応

## Test Plan

### Conversion fixture 一覧 (`tests/fixtures/*.input.ts`)

| Fixture | Phase | 目的 | 新規/既存 |
|---|---|---|---|
| `callable-interface.input.ts` | P4.1 | 既存 single + multi 包括 (現 snapshot 更新対象) | **既存** (snapshot 書換え) |
| `callable-interface-simple-trait.input.ts` | P4.1 | single overload の最小例 | 新規 |
| `callable-interface-inner.input.ts` | P5.4 (旧 P5.6) | inner fn signature が widest 型で emit されることの snapshot lock-in | 新規 (**C6 対応で table 追加**) |
| `callable-interface-divergent.input.ts` | P7.1 | 2 overload + divergent return | 新規 |
| `callable-interface-expr-body.input.ts` | P6.3 | expression body arrow | 新規 |
| `callable-interface-ternary-return.input.ts` | P6.3 | ternary at return position | 新規 |
| `callable-interface-typeof-narrowing.input.ts` | P0.1 / P6.4 | typeof narrowing in body | 新規 |
| `callable-interface-switch-return.input.ts` | P0.1 / P6.4 | switch in body | 新規 |
| `callable-interface-async.input.ts` | P4.2 / P7.3 | Promise<T> overloads | 新規 |
| `callable-interface-generic.input.ts` | P9.3 | `interface I<T, U>` + concrete args | 新規 |
| `callable-interface-generic-arity-mismatch.input.ts` | P9.1 | 意図的 arity mismatch (変換 error test) | 新規 |
| `callable-interface-polymorphic-none-ambiguous.input.ts` | P6.2 | ambiguous return null (hard error test) | 新規 |
| `callable-interface-overload-select-*.input.ts` | P10.2 | select by arity + type 3 ケース | 新規 |
| `callable-interface-pascal-collision.input.ts` | P5.1 | 同名 pascal case 2 const | 新規 |
| `callable-interface-any-narrowing.input.ts` | P5.4 | widest Any + body narrowing (R4-C6) | 新規 |
| `callable-interface-void-multi.input.ts` | P9.4 | void-only multi overload (Stage 2 bug 再現) | 新規 |
| `const-primitive.input.ts` | P1.5 | primitive const (R4-C3 transformer 側修正確認) | 新規 |
| `async-class-method.input.ts` | P1.4 | async class method の is_async propagation 確認 | 新規 (C21 対応 — `grep` で既存 fixture に無ければ作成) |

### E2E test script (`tests/e2e/scripts/*.ts`) — **C11 対応で fixture table から分離**

| E2E script | Phase | 目的 |
|---|---|---|
| `tests/e2e/scripts/callable_interface.ts` | P11.1 | TS 実行 (`tsx`) と変換 Rust 実行 (`cargo run`) の stdout 完全一致確認。divergent return, generic, async 等の callable interface 使用を全て含む |

### 各 conversion fixture の verification 手順

1. `ts_to_rs fixture.ts -o output.rs`
2. `rustc --edition 2021 --crate-type lib output.rs` → compile pass 確認 (error-case fixture
   は除く)
3. Insta snapshot で IR/generator 出力の shape lock-in
4. `tests/compile_test.rs` に登録 (INV-9)

### E2E test の verification 手順

1. `tests/e2e/scripts/callable_interface.ts` を作成 (`function main(): void` で
   `console.log` を使った observable output を含む)
2. `tests/e2e_test.rs` に `test_e2e_callable_interface_ts_rust_stdout_match`
   関数を追加、`run_e2e_test("callable_interface")` を呼ぶ
3. `cargo test --test e2e_test test_e2e_callable_interface_ts_rust_stdout_match`
   で TS 実行 stdout と Rust 実行 stdout の完全一致を確認

### Snapshot 更新方針

Phase 4.1 で `convert_interface_as_fn_type` を trait 化することで以下既存 snapshot が
書き換わる:

- `integration_test__callable_interface.snap` (trait + marker + impls + const に変化)
- その他 callable interface を含む snapshot (`type_alias_forms.rs` 関連等)

各 phase 完了時に `cargo insta review` で 1 件ずつ accept。snapshot diff 内容を
PR 説明に転記。

## Completion Criteria

1. Round 4 Critical 7 項目 (R4-C1〜C7) が解消し、対応する fixture で rustc compile pass
2. Round 1-3 の既存 fix (section A') の preserve / reversal 状態が各 phase で
   明示的に反映されていること:
   - R3-C1 (factory method): Phase 0.4 で preserve (INV-8)
   - R3-C2 (Polymorphic None hard error): Phase 6.2 で preserve (INV-3)
   - R1-L1-1 (generic type substitution): Phase 9.3 で type substitution helper 化
   - R2-L1-2 (TypeResolver fallthrough recovery): Phase 10.3 で **REVERSE** (fallthrough 全面禁止)
   - R2-L1-3 (wrap_expr_tail Match/IfLet): Phase 6.3 で Expr::If (ternary) preserve。
     Match/IfLet は P0.1 で empirical 確認後に P6.4 で決定
   - R3-L1-4 (L1-5 Pass clone 最適化): Phase 2.3 で **REVERSE** (Pass 2a → Pass 2b の 2 step 化)
   - R3-L1-5 (Error message SWC source span): Phase 6.1 で preserve
   - R2-L2-1 (ZST struct derive list): Phase 5.2 で preserve (**marker 専用** — Revision 3.3 C3)
   - R2-L2-2 (Expr::StructInit empty fields → Name 形式): Phase 5.3 で preserve
   - R2-L2-3 (wrap_leaf の ast_arg: &ast::Expr signature): Phase 6.1 で preserve
   - R2-L2-4 (unwrap_promise を RustType::unwrap_promise() method に集約): Phase 4.2 で preserve (INV-6)
   - **R2-L2-5 (doc/impl 整合 + visibility 適正化)**: 明示 phase なし、**Phase 13 quality gate の `cargo clippy` + `cargo fmt` で自動的に検証** (C10 解消)
3. Round 3 の test addition 全件が phase Exit で確認されている:
   - `marker_struct_name` / `build_return_wrap_context` / `polymorphic_none_*` /
     `Item::Const` generator unit tests
   - `tests/e2e/scripts/callable_interface.ts` + `test_e2e_callable_interface_ts_rust_stdout_match`
4. Pre-existing gap が解消済:
   - R4-C3 transformer 側 (arrow_fns.rs arrow-only filter): **Phase 1.5** で解消
     (**Expr::Lit のみ** — Revision 3.3 C2。Call/Ident は follow-up PRD)
   - arg_count=0 bug (resolve_fn_type_info): Phase 9.2 で解消
   - Stage 2 bug (select_overload): Phase 9.4 で解消
   - factory method 不在: Phase 0.4 で解消
5. Phase 0.3 で Round 4 L2/L3/L4 12 項目の verification 完了 (real 1 件: L2-4 cosmetic、
   auto-solved 6 件、false alarm 4 件)、real 項目は Phase 12 で fix
6. **P8.2 統合チェックポイント** (Revision 3.3 H1): **完了**。既存 callable-interface fixture の
   変換結果が rustc compile pass、compile_test.rs から callable-interface 系 6 fixture 復帰、
   `async-class-method` stale skip も解消
7. `cargo test` 全件 pass (Phase 13)
8. `cargo clippy --all-targets --all-features -- -D warnings` 0 warning (Phase 13)
9. `cargo fmt --all --check` 0 diff (Phase 13)
10. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` pass
   (threshold が 2+ 超過なら +1 ratchet — CLAUDE.md ルール) (Phase 13)
11. Hono bench regression 0 (P0.0 baseline との比較, Phase 11.2)
12. 全 callable-interface fixture が `tests/compile_test.rs` に登録されている (INV-9, P8.2 + Phase 11.1)
13. `scripts/check-classify-callable-usage.sh` — `helpers.rs` の violation が P9.2 で
    解消されていること確認。他 module の violation (ts_type_info, intersection,
    type_aliases, registry/mod.rs) は本 PRD scope 外 (Phase 13 では lint script の
    `exit 0` を維持、全 violation 解消は follow-up PRD)
14. `scripts/check-transformer-construction.sh` pass (INV-8 lint, Phase 13)
15. `scripts/check-promise-unwrap.sh` pass (INV-6 lint, Phase 13)
16. Invariant INV-1〜9 が型 or CI lint で enforce されている (上記 12-15 で確認)
17. **機能実装完了後、最適化は別 PRD に分離**。本 PRD に最適化を混入しない
    (前回失敗の lessons learned)

## Non-Goals

- 最適化 (Pass 2 の clone 削減、DRY 集約、inline optimization) は本 PRD に含めない
- I-181 (call signature generic type params) は本 PRD 完了後に再評価
- Overloaded method signatures (`interface I { method(x: string): void; method(x: number): void; }`)
  は別 code path (`convert_method_signature`) なので本 PRD 外
- Object literal / array literal / `as const` 初期化の module-level const は本 PRD 外
  (I-392 で必要にならないため follow-up PRD)
- **`Expr::Call` / `Expr::Ident` の non-arrow init の module-level const 変換**
  (Revision 3.3 C2 — `const` vs `static` vs `lazy_static` の設計判断が必要。follow-up PRD)
- **`Expr::Lit::Str` / `Expr::Lit::Regex` / `Expr::Lit::BigInt` の module-level const 変換**
  (Phase 1 deep review で発見): Rust の `const` 宣言では `to_string()` や `Regex::new()`
  が呼べない (const fn ではない)。現状 `convert_lit_var_decl` は `Num`/`Bool`/`Null` のみ
  const-safe として `Item::Const` に変換し、他のリテラル型は skip する。String const は
  以下の設計判断が必要で follow-up PRD に委ねる:
  - `const MSG: &str = "hello";` (const-safe だが `String` ではなく `&str` が必要)
  - `static MSG: String = String::from("hello");` (static なら non-const fn 呼び出し可能だが
    interior mutability の問題)
  - `lazy_static! { static ref MSG: String = "hello".to_string(); }` (外部依存追加)
  - Regex も同様: `lazy_static! { static ref RE: Regex = Regex::new("pattern").unwrap(); }`
- **Rest parameter を含む overloaded callable interface の widest 計算**
  (Phase 3 実装時に判明): `compute_widest_params` は位置ベースでパラメータを比較する。
  `interface I { (...args: number[]): void; (x: string, ...args: number[]): void; }` のように
  rest param が異なる位置にある overload の組み合わせでは、位置ベース比較が不正確になる。
  Hono の 4 callable interface には rest param overload がないため I-392 scope 外。
  follow-up PRD で対応（rest param の semantics を考慮した widest 計算が必要）
- **`RustType::Any` 型注釈の module-level const 変換**
  (Phase 3 /check_problem で発見): `const anyVal: any = 42` は `serde_json::Value` 型に
  変換されるが、`42.0` は `f64` であり `const anyVal: serde_json::Value = 42.0;` は
  型不一致で compile 不可。現状 `convert_lit_var_decl` で `RustType::Any` の場合 skip。
  正しくは `serde_json::json!(42.0)` 等に変換すべきだが、設計判断が必要
- **Type alias 由来の callable type の trait 化**
  (Phase 4 /check_problem で発見): `type Handler = { (x: string): string }` のような
  type alias 由来の callable type は `is_interface: false` のため `classify_callable_interface`
  で `NonCallable` と判定され、`type_aliases.rs` の `Box<dyn Fn>` path が適用される。
  trait 化は `interface` 宣言のみが対象。type alias callable type の trait 化は
  `type_aliases.rs` の `TsTypeLiteralInfo` → `Item::Trait` 変換パスが必要で、
  `interfaces.rs` とは別の実装が必要。follow-up PRD で対応。
  `classify_callable_interface` に `is_interface` guard を追加して後続 phase の
  不整合 (trait 未定義で trait impl 生成) を防止済
- **Builtin types と user-defined types の名前衝突時のマージ戦略改善**
  (Phase 6 /check_problem で発見): CLI はデフォルトで builtin types を読み込む
  (`--no-builtin-types` 未指定)。Web Streams API の `Transformer` interface 等が
  ユーザー定義と名前衝突すると `TypeRegistry::merge` でマージされ、
  `classify_callable_interface` が正しく判定できなくなる。
  現状の対策: fixture の interface 名を builtin と衝突しない名前に変更。
  根本対策: ユーザー定義がビルトインを上書きするマージ戦略 (follow-up PRD)
- `interface Factory { new (config): Factory; name: string; }` 等 construct signature
  の emission 改善 (現在も emit されていない、変更なし)
- **for-of ループ変数の TypeResolver 型解決不足**
  (Phase 7 スコープ外で発見): `for (const item of items)` の `item` の型が
  TypeResolver で `Unknown` になるケースがある (配列要素型の推論が未対応)。
  callable interface arrow body で for-of ループ変数を return する場合、wrap_leaf の
  priority 3 (TypeResolver 型) がスキップされ priority 4 (single non-Option fallback)
  に fall through する。根本修正は TypeResolver の for-of 要素型推論 (別イシュー)
- **Promise unwrap + Unit 除去パターンの DRY 化**
  (Phase 7 スコープ外で発見): `.map(|ty| ty.unwrap_promise()).and_then(...)` パターンが
  3 箇所に存在。各箇所でコンテキストが微妙に異なり、共有すると結合度が上がるため
  現時点では許容。`RustType::unwrap_promise_to_return_type()` convenience method として
  統合を検討 (follow-up refactoring)
- **non-async arrow with `Promise<T>` return type**
  (Phase 7 スコープ外で発見): TypeScript では `async` キーワードなしで `Promise<T>` を
  返す関数が書ける。現在の trait 生成は `Promise<T>` → `async fn -> T` と一律変換するため、
  non-async arrow の場合に trait impl が `async fn` を要求する不整合が生じる。
  callable interface 固有ではなく trait 生成の一般的な設計課題 (別イシュー)
- **callable-interface fixture body での Option narrowing テスト**
  (Phase 8 /check_problem で確認): `callable-interface-inner` / `callable-interface-async`
  の fixture body に `if (key)` / `if (flag)` パターンがあり、`key: Option<String>` /
  `flag: Option<bool>` の truthiness check が Rust で compile 不可 (I-360: Option narrowing
  + 暗黙 None)。PRD H3 (Phase 5-8 fixture body 制限) に従い body を単純化して compile_test
  を復帰。divergent return の multi-path body テストは Phase 11 E2E で実施予定。
  Option narrowing の根本修正は I-360 (別 PRD) の scope

## References

- `report/i392-round4-verification.md` — Critical 7 項目の empirical verification
  と session-level lessons learned (次 session 引継ぎ必須)
- `.claude/rules/ideal-implementation-primacy.md` — 最上位原則
- `.claude/rules/todo-prioritization.md` — Investigation Debt Step 0
- `.claude/rules/conversion-correctness-priority.md` — silent semantic change 禁止
- `.claude/rules/prd-completion.md` — scope 縮小禁止
- `.claude/rules/incremental-commit.md` — phase 境界での commit 方針
- `.claude/rules/pre-commit-doc-sync.md` — plan.md/TODO 同期
- `CLAUDE.md` — coverage threshold ratchet ルール
