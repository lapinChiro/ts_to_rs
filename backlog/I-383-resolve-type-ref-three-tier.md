# I-383: `resolve_type_ref` の 3 階層判定化と型パラメータ scope 完備化

> **重要 (2026-04-07)**: 本 PRD の T6 (Step 3 = unknown error 化) は実装中に **73 件の既存テスト失敗** を引き起こすことが判明したため、別 PRD `I-386` (PRD-A-2, `backlog/I-386-resolve-type-ref-step3-and-test-fixture-cleanup.md`) に分離した。本 PRD のスコープは「Step 1 (型パラメータ scope) + Step 2 (user 定義型) + 型パラメータ scope 補完」に縮小される。Step 3 (unknown → error) は I-386 で実装する。
>
> 実質スコープ: T1-T5 (済) + T7-T11 + T12 (quality-check)。T6 は I-386 に移管。
> Cluster 1a (11 件) は本 PRD で解消、Cluster 1b/1c は I-386 で解消。


## Background

`src/ts_type_info/resolve/mod.rs::resolve_type_ref` (L370-444) の default branch は、引数 `name` に対し以下を **一切区別せず** すべて `RustType::Named { name, type_args }` として返している:

1. 関数 / メソッド / クラスの **型パラメータ** (例: `<M extends string>` 内の `M`)
2. **user 定義型** (例: `HTTPException`)
3. TypeScript の **lib.dom / lib.es5 ambient builtin** (例: `BufferSource`, `Window`)
4. TypeScript compiler internal marker の leak (例: `__type`)
5. 真の **未知識別子**

この区別欠如により、TypeResolver が anonymous union/struct を生成するときにこれらが等しく field/variant 型に焼き込まれる。下流の `external_struct_generator::generate_stub_structs` は band-aid (`defined_elsewhere_names` exclusion + 空 stub フォールバック) で隠蔽していたが、これは silent semantic change (Tier 1) のリスクを温存する設計である。

事前検証 (`report/i382/probe-raw.log`) で Hono 158 fixture に対し、上記のうち type_param leak + lib builtin leak + `__type` leak の合計 **32 件** が `generate_stub_structs` の空 stub フォールバックで握り潰されていることが実測された。

加えて、`SyntheticTypeRegistry::push_type_param_scope` は **わずか 7 箇所** でしか呼ばれておらず、関数 / メソッド / クラス member の converter では呼ばれていない。これにより `register_union` 内の type_param 抽出ロジック (synthetic_registry/mod.rs:139-148) が空 scope のため機能せず、結果として `enum MOrVecM` が `type_params: vec![]` で生成されてしまう。

更に `register_struct_dedup` (`_TypeLit*` 用) と `register_intersection_enum` は **`type_param_scope` 伝播ロジック自体を持たず** `type_params: vec![]` を hardcode している。これは `register_union` との非対称な実装で、cluster 1a の `Status` (in `_TypeLit4`) leak の直接原因。

最後に、`external_struct_generator::collect_undefined_refs_inner` (L111-124) の type_param 除外フィルタは `Item::Enum` を含んでおらず、生成された generic synthetic enum の type_params が collector に認識されない。これも `M` / `S` 等の leak の必要条件となっている。

## Goal

`resolve_type_ref` の default branch を 3 階層判定 (型パラメータ / user 定義型 / 未知=明示エラー) に再設計し、`push_type_param_scope` 呼び出しを関数 / メソッド / クラス member converter に補完し、`register_*` 系 3 関数の type_param 伝播を共通ヘルパーで統一することにより、Hono 158 fixture で `generate_stub_structs` の空 stub フォールバックが **Cluster 1a (11) + 1b (20) + 1c (1) = 32 件 0 件** になる。

具体的測定基準:

1. probe instrumentation (= report/i382/phase0-synthesis.md の probe コード) を再投入した結果、`dangling iter=0 name=<X>` 行のうち以下が **0 件**:
   - 型パラメータ系: `M`, `S`, `U`, `E`, `P`, `TNext`, `TResult`, `TResult1`, `TResult2`, `Status`, `OutputType` (11 件)
   - lib.dom / lib.es5 builtin 系: `HTMLCanvasElement`, `HTMLImageElement`, `HTMLVideoElement`, `SVGImageElement`, `ImageBitmap`, `VideoFrame`, `AudioData`, `BufferSource`, `CanvasGradient`, `CanvasPattern`, `MediaSourceHandle`, `RTCDataChannel`, `ImageBitmapRenderingContext`, `WebGL2RenderingContext`, `WebGLRenderingContext`, `ServiceWorker`, `Window`, `HeadersInit`, `RequestInfo`, `TemplateStringsArray`, `symbol` (21 件、内部 1 件は重複の可能性あり、最終件数は実装後再計測)
   - compiler internal: `__type` (1 件)
2. Hono 158 fixture で `--report-unsupported` の error 計上に「unknown type ref: <name>」が cluster 1b/1c 該当型について明示計上される (silent ではない)
3. 既存の `generate_stub_structs` 関数は **本 PRD では削除しない**。削除は I-382 本体 (PRD-B) で行う。本 PRD 完了時点では `generate_stub_structs` の空 stub フォールバックの 32 件が、上記 0 件達成により実行されなくなった状態にする
4. /quality-check 通過 (clippy / fmt / test 全 pass)
5. `cargo test` 全 pass

## Scope

### In Scope

1. **`resolve_type_ref` の 3 階層判定実装** (`src/ts_type_info/resolve/mod.rs:370-444`):
   - ① 型パラメータ scope に存在 → そのまま `RustType::Named { name, type_args: vec![] }` (型引数なし、type_param 参照と同じ表現)
   - ② `TypeRegistry::get(name).is_some()` → user 定義型として `RustType::Named { name, type_args: <resolved_args> }` (現状動作)
   - ③ 未知 → `Err(anyhow::Error::msg(format!("unknown type ref: {name}")))` (明示エラー、silent ではない)
2. **`SyntheticTypeRegistry::extract_used_type_params` 共通ヘルパー追加**:
   - `register_union` (synthetic_registry/mod.rs:139-148) の既存ロジックを抽出
   - `register_struct_dedup` (L240-271) で同ヘルパーを使い `type_params` を生成
   - `register_intersection_enum` (L280-311) で同ヘルパーを使い `type_params` を生成
3. **`push_type_param_scope` の補完** — 以下の converter で generic 型パラメータを scope に push する:
   - `src/transformer/functions/mod.rs::convert_fn_decl` (関数宣言)
   - `src/transformer/expressions/functions.rs::convert_arrow_expr` / `convert_arrow_expr_with_return_type` (arrow 関数)
   - `src/transformer/classes/members.rs::convert_class_method` (クラスメソッド)
   - `src/transformer/classes/members.rs::convert_constructor` (コンストラクタ)
   - `src/transformer/classes/members.rs::convert_class_prop` (クラスプロパティ — クラスの type_params がプロパティ型に登場する場合)
   - `src/pipeline/type_converter/interfaces.rs::convert_method_signature` (interface method signature) — 既存の interface 単位 push に加えてメソッド単位 push が必要か要確認
4. **`external_struct_generator::collect_undefined_refs_inner` の type_param 除外フィルタ修正** (L111-124):
   - 抽出対象に `Item::Enum { type_params, .. }` を追加
5. **`resolve_type_ref` が `type_param_scope` を参照できるようにシグネチャ変更**:
   - 現状: `resolve_type_ref(name, type_args, reg, synthetic)` の `synthetic: &mut SyntheticTypeRegistry` は scope を保持しているので、新規引数追加は不要
   - `synthetic.is_in_type_param_scope(name)` 相当のメソッドを `SyntheticTypeRegistry` に追加 (公開 API 拡張)
6. **新規テスト**:
   - `test_register_struct_dedup_detects_type_params_from_scope` (`synthetic_registry/tests.rs`)
   - `test_register_intersection_enum_detects_type_params_from_scope` (同)
   - `test_resolve_type_ref_returns_error_for_unknown_name` (`ts_type_info/resolve/mod_tests.rs`)
   - `test_resolve_type_ref_recognizes_type_param_in_scope` (同)
   - `test_collect_undefined_refs_excludes_enum_type_params` (`external_struct_generator/tests/undefined_refs_tests.rs`)
   - 関数 / メソッド / クラス generic を含む整合性テスト (各 converter テストファイルに 1 件ずつ)
   - lib.dom 型を参照する Hono 風 fixture が変換 error として正しく扱われる integration test
7. **`__type` の発生経路特定と修正** — `resolve_type_ref` の 3 階層化で ③ (unknown) 経路に流れる想定だが、別経路から `__type` が `RustType::Named` として construct されている場合は当該箇所を grep で特定して修正

### Out of Scope

- `generate_stub_structs` 関数自体の削除 (= I-382 本体 = PRD-B のスコープ)
- synthetic → user 型 import 生成 (= PRD-B のスコープ)
- lib.dom 型を `web_sys` 連携で実装可能な型に変換するロジック
- TypeScript lib.\*.d.ts のパース・自動取り込み
- `Item::Unsupported` variant の新設
- `RustType::ExternalBuiltin` 等の新 RustType variant 追加 (3 階層の中に lib builtin を独立 case として持たないため)

## Design

### Technical Approach

#### 1. `resolve_type_ref` の 3 階層化

`src/ts_type_info/resolve/mod.rs:370-444` の default branch (L429-442) を以下に置換:

```rust
_ => {
    // Step 1: 型パラメータ scope 判定
    if synthetic.is_in_type_param_scope(name) {
        // 型パラメータは型引数を持たないため args は空。
        // 既存 type_args (TS 側で `M<X>` のように書かれていたとしても、
        // 型パラメータに対する高階型は Rust では表現できないため drop)
        return Ok(RustType::Named {
            name: sanitize_rust_type_name(name),
            type_args: vec![],
        });
    }

    // Step 2: user 定義型判定 (現状の expected_count truncate ロジックを保持)
    let resolved_args = type_args
        .iter()
        .map(|a| resolve_ts_type(a, reg, synthetic))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let expected_count = reg.get(name).map(|td| td.type_params().len());
    if let Some(expected) = expected_count {
        let mut args = resolved_args;
        if args.len() > expected {
            args.truncate(expected);
        }
        return Ok(RustType::Named {
            name: sanitize_rust_type_name(name),
            type_args: args,
        });
    }

    // Step 3: 未知識別子 → 明示エラー
    Err(anyhow::anyhow!("unknown type ref: {name}"))
}
```

注意: Step 2 の `resolved_args` 計算は `?` で他の resolve エラーを伝播するが、現状コード (L394-397) と挙動が変わらない。Step 1 で型パラメータと判定した時点では `type_args` を resolve しない (型パラメータに対する型引数は意味を持たないため)。

#### 2. `SyntheticTypeRegistry` API 拡張

```rust
impl SyntheticTypeRegistry {
    /// 現在の scope に指定の型パラメータ名が含まれるか判定する。
    pub fn is_in_type_param_scope(&self, name: &str) -> bool {
        self.type_param_scope.iter().any(|tp| tp == name)
    }
}
```

#### 3. 共通ヘルパー `extract_used_type_params`

`synthetic_registry/mod.rs` 内に private fn として追加:

```rust
/// member 型集合と現在の type_param_scope から、実際に使われている型パラメータ
/// のみを `Vec<TypeParam>` として抽出する。
///
/// `register_union` / `register_struct_dedup` / `register_intersection_enum`
/// の 3 箇所で同じロジックを共有するため抽出。
fn extract_used_type_params(
    member_types: &[RustType],
    scope: &[String],
) -> Vec<TypeParam> {
    scope
        .iter()
        .filter(|tp_name| member_types.iter().any(|ty| ty.uses_param(tp_name)))
        .map(|tp_name| TypeParam {
            name: tp_name.clone(),
            constraint: None,
        })
        .collect()
}
```

`register_union` (L139-148) は呼び出しに置換:
```rust
let type_params = extract_used_type_params(member_types, &self.type_param_scope);
```

`register_struct_dedup` (L240-271) は `fields` から member 型集合を構築:
```rust
let member_types: Vec<RustType> = fields.iter().map(|f| f.ty.clone()).collect();
let type_params = extract_used_type_params(&member_types, &self.type_param_scope);
let item = Item::Struct {
    vis: Visibility::Public,
    name: name.clone(),
    type_params,  // 旧: vec![]
    fields: fields.to_vec(),
};
```

`register_intersection_enum` (L280-311) は `variants` から各 variant の data 型を抽出:
```rust
let member_types: Vec<RustType> = variants
    .iter()
    .filter_map(|v| v.data.clone())
    .collect();
let type_params = extract_used_type_params(&member_types, &self.type_param_scope);
```

#### 4. `collect_undefined_refs_inner` の Enum 漏れ修正

`external_struct_generator/mod.rs:111-124`:

```rust
let type_param_names: HashSet<String> = items
    .iter()
    .flat_map(|item| match item {
        Item::Struct { type_params, .. }
        | Item::Enum { type_params, .. }  // ← 追加
        | Item::Trait { type_params, .. }
        | Item::Fn { type_params, .. }
        | Item::Impl { type_params, .. }
        | Item::TypeAlias { type_params, .. } => type_params
            .iter()
            .map(|tp| tp.name.clone())
            .collect::<Vec<_>>(),
        _ => vec![],
    })
    .collect();
```

#### 5. `push_type_param_scope` の補完

各 converter で関数/メソッド/クラスの generic 型パラメータを scope に push し、本体処理後に restore する。`type_aliases.rs:21,61` のパターンを踏襲:

```rust
// (例) convert_fn_decl 内
let tp_names: Vec<String> = generic_params.iter().map(|tp| tp.name.clone()).collect();
let prev_scope = self.synthetic.push_type_param_scope(tp_names);

// ... 関数本体の変換 ...

self.synthetic.restore_type_param_scope(prev_scope);
```

scope のネスト (例: クラスの generic 内のメソッドの generic) は既存の `push_type_param_scope` がメンバー入れ替え方式なので、ネスト時は **outer scope を inner で「上書き」する** ことになる。これは TS の semantics と一致する (内部 scope の同名が外部を shadow) が、union 型の判定はメンバー型の要素を見るだけで scope メンバーの「すべて」をフィルタするため、外部 scope の型パラメータが member type に出現していれば検出されるべき。

**設計判断**: ネスト時は外部 scope の型パラメータを失わないため、`push` で **既存に append、restore で truncate** するように API 改修すべきか。これは `prd-design-review` 観点で要検討。

→ **本 PRD では `push_type_param_scope` の意味論を `replace` から `append-or-merge` に変更する**。具体的には:

```rust
pub fn push_type_param_scope(&mut self, names: Vec<String>) -> Vec<String> {
    let prev = self.type_param_scope.clone();
    for name in names {
        if !self.type_param_scope.contains(&name) {
            self.type_param_scope.push(name);
        }
    }
    prev
}

pub fn restore_type_param_scope(&mut self, prev: Vec<String>) {
    self.type_param_scope = prev;
}
```

これにより外部 scope と内部 scope が両方アクティブになり、ネスト generic の正確性を保てる。既存呼び出し側は変更不要 (replace → append-merge は subset 関係)。

#### 6. `__type` の発生経路特定

3 階層化後、`__type` が `resolve_type_ref` 経由なら ③ branch で error 化される。それ以外の経路 (例: TypeCollector で TS compiler の anonymous symbol を拾う直接構築) は別途 grep で特定。Phase 0 では深掘りしていないため、PRD-A 実装中の RED テスト失敗時に追跡する。

### Design Integrity Review

`prd-design-review.md` 3 観点での自己レビュー:

#### 凝集度

- `resolve_type_ref` の 3 階層判定: 単一責務 (TS TypeRef → RustType の resolution) で、3 階層は同一抽象レベル
- `extract_used_type_params`: 単一責務 (member 型 + scope → 使用型パラメータ抽出)
- `push_type_param_scope` の append-merge 改修: scope 管理という単一責務に閉じる
- ✅ 凝集度問題なし

#### 責務分離

- 「未知判定」(`resolve_type_ref`) と「stub 生成」(`external_struct_generator`) が分離される。現状は両者が `defined_elsewhere_names` を介して暗黙的に結合していたが、新設計では `resolve_type_ref` で error 終端し、stub 生成は user 定義型のみ扱う
- 「scope 管理」(`SyntheticTypeRegistry::type_param_scope`) と「scope 利用」(`resolve_type_ref` / `register_*`) が API 経由で疎結合
- ✅ 責務分離問題なし

#### DRY

- `extract_used_type_params` 抽出により `register_*` 3 関数の重複ロジックが解消
- `push_type_param_scope` 補完の 5 箇所は形式上の繰り返しになるが、各々異なる context (関数 / arrow / メソッド / コンストラクタ / プロパティ) でパターンが異なる可能性あり。共通マクロ化は YAGNI で、各々で生コードを書く
- `external_struct_generator` の Enum 漏れ修正: 単独 1 行追加で重複なし
- ✅ DRY 問題なし

#### 結合度

- `SyntheticTypeRegistry::is_in_type_param_scope` 公開メソッドにより `resolve_type_ref` から scope を読める。これは新規結合だが、scope の所在を `SyntheticTypeRegistry` に集約する既存設計と整合
- ✅ 結合度問題なし

### Impact Area

- `src/ts_type_info/resolve/mod.rs` (resolve_type_ref 3 階層化)
- `src/pipeline/synthetic_registry/mod.rs` (extract_used_type_params 抽出 + push_type_param_scope 改修 + is_in_type_param_scope 追加)
- `src/pipeline/external_struct_generator/mod.rs` (Item::Enum 漏れ修正)
- `src/transformer/functions/mod.rs` (convert_fn_decl の scope push)
- `src/transformer/expressions/functions.rs` (convert_arrow_expr* の scope push)
- `src/transformer/classes/members.rs` (convert_class_method / convert_constructor / convert_class_prop の scope push)
- `src/pipeline/type_converter/interfaces.rs::convert_method_signature` (要確認)
- `src/pipeline/synthetic_registry/tests.rs` (struct/intersection の scope テスト追加)
- `src/ts_type_info/resolve/mod_tests.rs` (resolve_type_ref の 3 階層テスト追加)
- `src/pipeline/external_struct_generator/tests/undefined_refs_tests.rs` (Enum type_param exclusion テスト追加)
- `tests/` (lib.dom 型 error の integration test)

### Semantic Safety Analysis

本 PRD は `resolve_type_ref` の type fallback を **狭める** (現状の Named 化 fallback を error 化) ため、`type-fallback-safety.md` の 3 ステップ分析を適用:

**Step 1: 型 fallback パターン**

| パターン | 現状 | 新設計 |
|---|---|---|
| 型パラメータ参照 (`M`) | `RustType::Named { name: "M", type_args: vec![] }` | 同 (Step 1 branch) |
| user 定義型参照 (`HTTPException`) | `RustType::Named { name: "HTTPException", type_args }` | 同 (Step 2 branch) |
| lib.dom 型 (`BufferSource`) | `RustType::Named { name: "BufferSource", type_args }` (silent fall-through) | `Err("unknown type ref: BufferSource")` (Step 3) |
| `__type` | 同 (silent fall-through) | `Err("unknown type ref: __type")` (Step 3) |
| 真の未知 | 同 (silent fall-through) | `Err("unknown type ref: ...")` (Step 3) |

**Step 2: 各 usage site の分類**

- 型パラメータ branch: 動作変更なし (現状と同じ Named 化)
- user 定義型 branch: 動作変更なし (現状と同じ Named 化、expected_count truncate も保持)
- 未知 branch: 現状の silent Named 化 → error 化。**Safe 改善**: silent semantic change が消え、conversion-correctness-priority Tier 3 の正規化となる。下流の anonymous union 化は error 伝播で停止し、`Item` レベルで unsupported 計上される

**Step 3: Verdict**

- 型パラメータ / user 定義型 branch: **Safe** (動作不変)
- 未知 branch: **Safe** (silent → error は Tier 1 リスクの解消)

UNSAFE パターンなし。本 PRD は type-fallback-safety に完全準拠。

## Task List

### T1: 共通ヘルパー `extract_used_type_params` の抽出 + RED テスト

- **Work**:
  - `src/pipeline/synthetic_registry/mod.rs` に private fn `extract_used_type_params(member_types: &[RustType], scope: &[String]) -> Vec<TypeParam>` を追加
  - `register_union` (L139-148) を `extract_used_type_params` 呼び出しに置換
  - 既存 `test_register_union_detects_type_params_from_scope` (L754) が継続 pass することを確認
  - **新規 RED テスト**: `test_register_struct_dedup_detects_type_params_from_scope` を追加 — `push_type_param_scope(["T"])` 後に `register_inline_struct(&[("x", Named("T"))])` を呼び、生成された `Item::Struct.type_params` が `[T]` であることを assert (現状は空 vec のため fail)
  - **新規 RED テスト**: `test_register_intersection_enum_detects_type_params_from_scope` を追加 — 同様の構造で intersection enum のテスト
- **Completion criteria**:
  - `extract_used_type_params` 関数が追加されている
  - `register_union` が新ヘルパーを使用している
  - 既存 union test が pass
  - 新規 struct/intersection test が **fail** (RED 状態を確認)
- **Depends on**: なし

### T2: `register_struct_dedup` / `register_intersection_enum` の type_param 伝播 (T1 の GREEN)

- **Work**:
  - `register_struct_dedup` (L240-271) で `extract_used_type_params` を呼び、生成 `Item::Struct.type_params` に渡す
  - `register_intersection_enum` (L280-311) で同様に `Item::Enum.type_params` に渡す
- **Completion criteria**:
  - T1 で追加した struct/intersection RED テストが pass
  - 既存テスト全 pass
- **Depends on**: T1

### T3: `push_type_param_scope` の append-merge 意味論変更

- **Work**:
  - `synthetic_registry/mod.rs:92-99` の `push_type_param_scope` / `restore_type_param_scope` を append-merge 方式に変更
  - 既存 7 箇所の caller が継続正常動作することを確認 (replace → append-merge は subset 関係なので影響なしの想定だが、`type_aliases.rs::convert_type_alias` のネストケース等を grep で確認)
  - 新規テスト: `test_push_type_param_scope_nests_scopes` を追加し、外部 scope が内部 scope の push 後も維持されることを assert
- **Completion criteria**:
  - 新規テスト pass
  - 既存テスト全 pass
- **Depends on**: なし (T1/T2 と並列可能)

### T4: `is_in_type_param_scope` 公開メソッド追加

- **Work**:
  - `SyntheticTypeRegistry::is_in_type_param_scope(&self, name: &str) -> bool` を追加
  - 単体テスト追加 (`tests.rs` に空 scope / 単一型パラメータ / 複数型パラメータ / マッチしないケース)
- **Completion criteria**:
  - 新規テスト pass
- **Depends on**: なし (T1/T2/T3 と並列可能)

### T5: `external_struct_generator::collect_undefined_refs_inner` の Enum 漏れ修正

- **Work**:
  - L111-124 の `type_param_names` 抽出 match arm に `Item::Enum { type_params, .. }` を追加
  - **新規 RED テスト** (`undefined_refs_tests.rs`): `test_collect_undefined_refs_excludes_enum_type_params` を追加 — `Item::Enum { type_params: [TypeParam { name: "M" }], variants: [variant with field of type Named("M")] }` を渡し、`M` が undefined refs に含まれないことを assert (現状は含まれる → fail)
  - 修正後 GREEN
- **Completion criteria**:
  - 新規テスト pass
  - 既存テスト全 pass
- **Depends on**: なし (T1-T4 と並列可能)

### T6: `resolve_type_ref` の 3 階層化 (RED → GREEN)

- **Work**:
  - **新規 RED テスト** (`mod_tests.rs`):
    - `test_resolve_type_ref_returns_error_for_unknown_name`: `resolve_type_ref("CompletelyUnknown", &[], reg, synthetic)` が `Err` を返すことを assert
    - `test_resolve_type_ref_recognizes_type_param_in_scope`: `synthetic.push_type_param_scope(vec!["T"])` 後に `resolve_type_ref("T", &[], reg, synthetic)` が `Ok(RustType::Named { name: "T", type_args: vec![] })` を返すことを assert
    - `test_resolve_type_ref_resolves_user_type_with_args`: TypeRegistry に登録された型が現状通り `Named` で返されることを assert (回帰防止)
  - `resolve_type_ref` (L370-444) の default branch を 3 階層判定に置換 (Design 1 を実装)
  - 既存テスト (`mod_tests.rs::resolve_type_ref_*`) が継続 pass することを確認
- **Completion criteria**:
  - 新規 3 テスト pass
  - 既存 `mod_tests.rs::resolve_type_ref_*` 全 pass
- **Depends on**: T4 (`is_in_type_param_scope` メソッドが必要)

### T7: 関数 / arrow 関数 converter の scope push 補完

- **Work**:
  - `src/transformer/functions/mod.rs::convert_fn_decl` (L43): 関数の generic type_params を `push_type_param_scope` し、本体変換後に restore
  - `src/transformer/expressions/functions.rs::convert_arrow_expr` (L155) / `convert_arrow_expr_with_return_type` (L170): 同様
  - **新規テスト** (各 converter のテストファイル): `test_convert_fn_decl_with_generic_propagates_scope` 等
  - **integration test**: TS source `function foo<M>(x: M | M[]): M { return x[0] }` を変換し、生成 Rust に `enum MOrVecM<M>` が `<M>` 付きで生成されることを assert
- **Completion criteria**:
  - 新規テスト pass
  - probe で `M` が dangling から消える (手動確認)
- **Depends on**: T1-T6 (synthetic_registry 改修と resolve_type_ref 3 階層化が前提)

### T8: クラスメソッド / コンストラクタ / プロパティ converter の scope push 補完

- **Work**:
  - `src/transformer/classes/members.rs::convert_class_method` (L232): メソッドの generic + クラスの generic を merge して push
  - `convert_constructor` (L78): クラスの generic を push (constructor 自体は通常 generic を持たないが、クラスの type_params が引数型に登場する)
  - `convert_class_prop` (L53): クラスの generic を push
  - **新規 integration test**: `class C<S> { foo(): S | S[] { ... } }` を変換し、メソッド本体内で `S` 系 union が generic 化されることを assert
- **Completion criteria**:
  - 新規テスト pass
  - probe で `S` が dangling から消える (手動確認)
- **Depends on**: T7 (パターン確立のため順序付け)

### T9: interface method signature の scope push 確認 (補完が必要なら追加)

- **Work**:
  - `src/pipeline/type_converter/interfaces.rs::convert_method_signature` (L392) を読み、generic method がある場合に scope が push されているか確認
  - 不足していれば push を追加
  - 関連テスト追加
- **Completion criteria**:
  - probe で interface generic 由来の dangling が 0
- **Depends on**: T8

### T10: `__type` の発生経路調査と対応

- **Work**:
  - T6 の 3 階層化適用後、probe を再投入して `__type` が ③ unknown branch で error 化されているか確認
  - error 化されていれば本タスクは PASS で終了
  - error 化されていない (= 別経路から construct されている) 場合、`grep -rn '"__type"\|format.*__type' src/` で発生源を特定し、適切な型 (`RustType::Fn` 等) に置換
  - **新規 integration test**: `RegExpMatchArray` 風 fixture の変換で `__type` が出力に出ないことを assert
- **Completion criteria**:
  - probe で `__type` 0 件
- **Depends on**: T6

### T11: probe 再投入 + Hono 全件検証

- **Work**:
  - probe instrumentation を `external_struct_generator::generate_stub_structs` に再投入 (Phase 0 と同じコード)
  - Hono 158 fixture 変換実行
  - probe ログから dangling refs を集計し、Cluster 1a (11) + 1b (20) + 1c (1) = 32 件が **0 件** であることを確認
  - 0 件達成後 probe instrumentation を撤去
- **Completion criteria**:
  - probe ログで Cluster 1a/1b/1c の dangling が 0 件
  - probe instrumentation がコードから撤去されている
- **Depends on**: T7, T8, T9, T10

### T12: /quality-check + 既存テスト全 pass + bench-history 記録

- **Work**:
  - `cargo fix --allow-dirty --allow-staged`
  - `cargo fmt --all --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
  - `./scripts/hono-bench.sh` を実行し `bench-history.jsonl` に新 entry
- **Completion criteria**:
  - 0 errors / 0 warnings / 全 test pass
  - bench-history に新 entry 追加
- **Depends on**: T11

## Test Plan

### 新規テスト一覧

1. `test_register_struct_dedup_detects_type_params_from_scope` — Bug 2 の RED→GREEN
2. `test_register_intersection_enum_detects_type_params_from_scope` — 同
3. `test_push_type_param_scope_nests_scopes` — append-merge 意味論
4. `test_is_in_type_param_scope_*` — 公開 API 単体 (空 / 単一 / 複数 / マッチしない)
5. `test_collect_undefined_refs_excludes_enum_type_params` — Enum 漏れ修正
6. `test_resolve_type_ref_returns_error_for_unknown_name` — 3 階層 ③ branch
7. `test_resolve_type_ref_recognizes_type_param_in_scope` — 3 階層 ① branch
8. `test_resolve_type_ref_resolves_user_type_with_args` — 3 階層 ② branch (回帰防止)
9. `test_convert_fn_decl_with_generic_propagates_scope` — 関数 generic
10. `test_convert_arrow_with_generic_propagates_scope` — arrow 関数 generic
11. `test_convert_class_method_with_generic_propagates_scope` — メソッド generic
12. `test_convert_constructor_with_class_generic_propagates_scope` — コンストラクタ + クラス generic
13. integration test: `function foo<M>(x: M | M[]): M` 変換結果に `enum MOrVecM<M>` が生成
14. integration test: `class C<S> { foo(): S | S[] { } }` メソッド内 union が generic 化
15. integration test: `RegExpMatchArray` 風参照が変換 error として error 計上
16. integration test: `BufferSource` 等 lib.dom 型参照が変換 error として error 計上

### 既存テスト保全

- `synthetic_registry/tests.rs::test_register_union_detects_type_params_from_scope` (L754)
- `ts_type_info/resolve/mod_tests.rs::resolve_type_ref_*` (L59, L423)
- `external_struct_generator/tests/undefined_refs_tests.rs` 全件
- 全てが PRD 完了時点で継続 pass であること

### Test Coverage Review (Impact Area)

#### Production Code Quality Issues (T0.1-T0.3 で発見)

| # | Location | Category | Severity | Action |
|---|---|---|---|---|
| P1 | `external_struct_generator/mod.rs:111-124` | C1 branch coverage gap (Item::Enum 漏れ) | High | T5 で修正 + テスト |
| P2 | `synthetic_registry/mod.rs:240-311` | DRY (3 関数で type_param 伝播ロジック非統一) | High | T1-T2 で共通ヘルパー化 |
| P3 | `ts_type_info/resolve/mod.rs:429-442` | 凝集度 (型パラメータ / user / lib / unknown を 1 branch で処理) | High | T6 で 3 階層化 |
| P4 | 各 converter (functions/classes/arrow) | scope 管理の責務漏れ (`push_type_param_scope` 呼ばれてない) | High | T7-T9 で補完 |
| P5 | `synthetic_registry/mod.rs:92-99` | API 設計 (push が replace でネスト不可) | Medium | T3 で append-merge 化 |

#### Test Coverage Gaps

| # | Missing Pattern | Technique | Severity | Action |
|---|---|---|---|---|
| G1 | `register_struct_dedup` の type_param 伝播 | C1 branch + 等価分割 | High | T1 で追加 |
| G2 | `register_intersection_enum` の type_param 伝播 | 同 | High | T1 で追加 |
| G3 | `collect_undefined_refs_inner` の Item::Enum branch | C1 branch (AST variant exhaustiveness) | High | T5 で追加 |
| G4 | `resolve_type_ref` の unknown identifier branch | 等価分割 (type_param / user / unknown) | High | T6 で追加 |
| G5 | 関数/メソッド/クラス generic の scope propagation E2E | integration | High | T7-T8 で追加 |
| G6 | `push_type_param_scope` のネスト動作 | 境界値 (空→単一→単一+単一) | Medium | T3 で追加 |

全ギャップを task list に組み込み済み (T1-T12)。

## Completion Criteria

1. 全 task (T1-T12) の completion criteria を満たす
2. probe で Cluster 1a (11) + 1b (20) + 1c (1) = **32 件 0 件**
3. `cargo test` 全 pass
4. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
5. `cargo fmt --all --check` 0 diff
6. `bench-history.jsonl` に新 entry 追加 (Hono 変換結果)
7. `report/i382/master-plan.md` の進捗表で T1.A / T2.A が `done`
8. 本 PRD 完了時点で `generate_stub_structs` 関数は **存続** している (削除は PRD-B = I-382 本体のスコープ)。ただし空 stub フォールバックの 32 件が実行されない状態 (= probe 0 件) を達成

### Impact 推定の検証 (3 件 trace)

PRD-A の影響推定値「Cluster 1a/1b/1c 計 32 件解消」は、Phase 0 の probe 実測 (`report/i382/probe-raw.log`) に基づく。3 件の代表的 trace:

1. **`M` (Cluster 1a)**: TS source `types.ts:2176` の `methods: M | M[]` → `convert_fn_decl` で scope push 補完 (T7) → `resolve_type_ref` Step 1 で型パラメータ判定 (T6) → `register_union` が `enum MOrVecM<M>` 生成 (T1) → walker が `M` を type_param として除外 (T5) → probe 0
2. **`BufferSource` (Cluster 1b)**: TypeRegistry に存在しない → `resolve_type_ref` Step 3 で error → anonymous union 化されず → walker に到達せず → probe 0
3. **`__type` (Cluster 1c)**: 同じく Step 3 で error 化 (T10 で経路確認、必要なら追加修正)

各 trace で「probe 0 件達成」までの execution path が確認可能。
