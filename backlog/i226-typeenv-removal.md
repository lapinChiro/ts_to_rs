# I-226: TypeEnv の完全除去

## 背景・動機

Transformer は `TypeEnv`（可変スコープチェーン）と TypeResolver の `FileTypeResolution`（不変 pre-pass 結果）の 2 系統で変数の型情報を並行管理している。これは DRY 違反であり、新しい変換構文を追加するたびに両方を更新する必要がある。片方の更新漏れがサイレントな型不整合を生むリスクがある。

現在 `type_env.get()` 8箇所、`insert()` 16箇所、`push_scope()`/`pop_scope()` 各2箇所。新機能追加のたびにこれらが増えるため、除去コストは時間とともに増大する。

## ゴール

1. `TypeEnv` 型と `type_env` フィールドが Transformer struct から完全に除去されている
2. `src/transformer/type_env.rs` から TypeEnv 構造体が削除されている（`wrap_trait_for_position` 等の独立ユーティリティは残存可）
3. 全ての型情報の読み取りが `FileTypeResolution`（`get_expr_type`, `narrowed_type`, `expr_type`）経由に一本化されている
4. テストコードから TypeEnv の構築・操作が完全に除去されている
5. 全テスト GREEN、clippy 0 警告、ベンチマーク結果が維持または改善

## スコープ

### 対象

- Transformer struct からの `type_env` フィールド除去
- 全 production code の `type_env.get/insert/push_scope/pop_scope` 呼び出しの置換
- TypeResolver / FileTypeResolution の拡張（不足する情報の補完）
- テストコードの TypeEnv 依存の除去
- `TypeEnv` 構造体自体の削除

### 対象外

- TypeResolver の型解決精度の改善（I-112c のスコープ）
- ジェネリクス基盤の修正（I-218 のスコープ）
- `wrap_trait_for_position`（TypeEnv と独立したユーティリティ関数。残存可）

## 設計

### 技術的アプローチ

TypeEnv の全用途を 5 分類し、それぞれを FileTypeResolution 経由に置換する。

#### 分類 1: 変数宣言の型登録（insert 6箇所 + get 3箇所）

**現状**: 変数宣言の変換後、IR の `Stmt::Let` から型を抽出して `type_env.insert(name, ty)` で登録。後続の式変換で `type_env.get(name)` により参照。

- `statements/mod.rs:363,392,455` — nullish coalescing / logical OR パターン（`insert`）
- `statements/mod.rs:1195,1204,1208` — 通常の変数宣言（`insert`）
- `statements/mod.rs:119` — Any 型注釈のフォールバック（`get`）
- `statements/mod.rs:1812` — typeof switch の enum 型取得（`get`）
- `expressions/calls.rs:44` — 関数型変数の参照（`get`）

**置換方針**: `get_expr_type()` は既に FileTypeResolution の `expr_types` + `narrowed_type` を参照している。TypeResolver は変数宣言時に `scope_stack` に型を登録し、後続の `Ident` 式の型を `expr_types` に記録済み。

- `statements/mod.rs:119` の Any フォールバック: `get_expr_type` で initializer の型を取得（TypeResolver が既に解決済み）
- `expressions/calls.rs:44` の Fn 型参照: `get_expr_type` で callee 式の型を取得
- `statements/mod.rs:363,392,455,1195,1204,1208`: insert を削除。後続の参照は全て `get_expr_type` に統一
- `statements/mod.rs:2065`: ネスト関数宣言 → TypeResolver が関数宣言を `scope_stack` に登録済み

**TypeResolver の拡張が必要なケース**:
- `statements/mod.rs:1204` の `infer_fn_type_from_closure`: クロージャの IR (`Expr::Closure`) から Fn 型を推論する。TypeResolver は AST レベルでアロー関数/関数式の型を解決するが、Transformer が生成した IR からの逆推論はしない。**対策**: TypeResolver の `visit_var_decl` でアロー関数/関数式の initializer に対して Fn 型を `expr_types` に登録する。現在 `visit_arrow_expr` / `visit_fn_expr` で `resolve_arrow_expr` / `resolve_fn_expr` を呼んでいるが、その結果を変数宣言の initializer スパンに紐づけて `expr_types` に記録する。

#### 分類 2: Narrowing ガード解決（get 3箇所 + push_scope/pop_scope 1ペア + insert 1箇所）

**現状**: 三項演算子の narrowing で:
1. `type_env.get(var_name)` で元の型を取得
2. `push_scope()` で新スコープ
3. narrowed type を `insert()`
4. body を変換（内部の `type_env.get()` が narrowed type を返す）
5. `pop_scope()` で復帰

- `expressions/mod.rs:175-188` — 三項演算子の narrowing スコープ
- `expressions/patterns.rs:276` — instanceof の LHS 型取得
- `expressions/patterns.rs:403` — `resolve_if_let_pattern` の変数型取得

**置換方針**: `get_expr_type()` は既に `narrowed_type(name, position)` を優先参照する。TypeResolver が narrowing_events を正しく生成していれば、`type_env` のスコープ操作は不要。

- `expressions/patterns.rs:403`: `self.type_env.get(guard.var_name())` → `get_expr_type` で guard の変数の AST Ident 式を使って型を取得。ただし現在 `guard.var_name()` は文字列で、AST ノードへの参照がない。**対策**: `NarrowingGuard` にソース Span を保持させ、`FileTypeResolution::expr_type(span)` で型を取得する。
- `expressions/patterns.rs:276`: 同上。`bin.left` の AST ノードから `get_expr_type` を使用。
- `expressions/mod.rs:175-188`: push_scope/pop_scope と insert を全て削除。`get_expr_type` が narrowed_type を position-based で返すため、body 変換中は自動的に narrowed type が使われる。

#### 分類 3: DU match arm フィールドバインディング（push_scope/pop_scope 1ペア + insert + get 1箇所）

**現状**: discriminated union の switch → match 変換時:
1. `push_scope()` で match arm スコープ作成
2. destructured フィールド変数を `insert(fname, ftype)` で登録
3. body 変換中、`member_access.rs:256` で `type_env.get(&field).is_some()` をチェック → match arm 内でフィールド名がローカル変数として存在するか判定 → 存在すれば `.clone()` を付加

- `statements/mod.rs:2063-2078` — DU switch case body のスコープ
- `expressions/member_access.rs:256` — フィールド名の shadow 判定

**置換方針**: TypeResolver に DU switch の各 case で destructure されるフィールドを事前解析させる。

**対策**: TypeResolver の `visit_switch_stmt` で、discriminated union の switch を検出した場合、各 case body のスコープ内でフィールド変数を `scope_stack` に登録する。これにより:
- Transformer は `get_expr_type` でフィールド変数の型を取得可能
- `member_access.rs:256` の shadow 判定は `get_expr_type(ident_expr).is_some()` で置換可能（TypeResolver がフィールド変数を scope に登録していれば、`expr_types` に Ident の型が記録される）

ただし `member_access.rs:256` は AST ノード（`MemberExpr`）のフィールド名（文字列）で判定しており、対応する `Ident` AST ノードがない。**対策**: FileTypeResolution に `is_du_field_binding(var_name: &str, position: u32) -> bool` メソッドを追加。TypeResolver が DU switch 解析時にフィールドバインディング情報（変数名 + スコープ範囲）を記録する。

#### 分類 4: 関数パラメータの型登録（insert 3箇所）

**現状**: 関数 body 変換前にパラメータ型を `type_env.insert(param.name, ty)` で登録。

- `functions/mod.rs:165` — 関数パラメータ
- `classes.rs:759` — メソッドパラメータ
- `functions/mod.rs:1176` — アロー関数の any-narrowing enum パラメータ

**置換方針**: TypeResolver の `visit_fn_decl` / `visit_arrow_expr` が既にパラメータを `scope_stack` に登録し、body 内の `Ident` 式の型を `expr_types` に記録済み。`insert` を削除するだけで、`get_expr_type` が正しい型を返す。

any-narrowing enum パラメータ（`functions/mod.rs:1176`）は特殊: AnyTypeAnalyzer の結果（`SyntheticTypeRegistry` の enum 型）で Any 型を上書きする。**対策**: TypeResolver に AnyTypeAnalyzer の結果を渡し、Any 型のパラメータに対して enum 型を `expr_types` に記録する。現在 TypeResolver は SyntheticTypeRegistry を `&mut` で受け取っているが、AnyTypeAnalyzer の結果（enum 名 → 変数名のマッピング）を追加で参照する仕組みが必要。

#### 分類 5: Any-narrowing enum オーバーライド（insert 2箇所 + get 1箇所）

**現状**: `functions/mod.rs:172`, `statements/mod.rs:119`
- AnyTypeAnalyzer が生成した enum 型で、`any` 型の変数を上書き

**置換方針**: 分類 4 と同じ。TypeResolver に AnyTypeAnalyzer の結果を統合する。

**具体的な設計**: `transpile_pipeline` の実行順序を変更:
1. Pass 2: Type Collection
2. Pass 3: Type Resolution（TypeResolver）← ここで AnyTypeAnalyzer の結果も使う
3. ~~AnyTypeAnalyzer は別 Pass~~ → TypeResolver 内で Any 型の変数に対する enum 型解決を行う

ただし現在の AnyTypeAnalyzer は Transformer の変換結果（どの変数が Any 型で typeof/instanceof で narrowing されるか）に依存しており、TypeResolver とは独立して動作する。TypeResolver のスコープ内で同等の解析を行う必要がある。

**簡易対策（段階的）**: AnyTypeAnalyzer の結果を `FileTypeResolution` に格納するフィールドを追加し、Transformer が参照する。TypeResolver の中に統合するのは将来の改善。

### 設計整合性レビュー

- **高次の整合性**: 変換パイプライン（parser → transformer → generator）の設計方針「TypeResolver が全ての型情報を事前解決し、Transformer は読み取るだけ」に完全に合致する。TypeEnv 除去はこの方針の完遂
- **DRY**: TypeEnv と FileTypeResolution の重複が完全に解消される
- **直交性**: Transformer の責務が「AST → IR 変換」に集中し、型追跡の責務が TypeResolver に完全移管される
- **結合度**: Transformer → FileTypeResolution の読み取り依存のみ。双方向依存なし
- **割れ窓**: なし

### 影響範囲

| ファイル | 変更内容 |
|---|---|
| `src/transformer/type_env.rs` | TypeEnv 構造体削除。`wrap_trait_for_position` / `TypePosition` は残存 |
| `src/transformer/mod.rs` | `type_env` フィールド除去、`for_module` から TypeEnv 初期化除去 |
| `src/transformer/statements/mod.rs` | insert 8箇所 + get 1箇所 + push_scope/pop_scope 1ペア 削除 |
| `src/transformer/expressions/mod.rs` | push_scope/pop_scope 1ペア + insert 1箇所 + get 1箇所 削除 |
| `src/transformer/expressions/patterns.rs` | get 2箇所 → get_expr_type に置換。NarrowingGuard に Span 追加 |
| `src/transformer/expressions/member_access.rs` | get 1箇所 → FileTypeResolution の DU binding 判定に置換 |
| `src/transformer/expressions/calls.rs` | get 1箇所 → get_expr_type に置換 |
| `src/transformer/expressions/functions.rs` | type_env.clone() 3箇所 削除 |
| `src/transformer/functions/mod.rs` | insert 3箇所削除、sub-transformer の type_env パラメータ除去 |
| `src/transformer/classes.rs` | insert 1箇所削除、MethodContext の type_env 除去 |
| `src/pipeline/type_resolution.rs` | `DuFieldBinding` 構造体追加、`is_du_field_binding` メソッド追加 |
| `src/pipeline/type_resolver.rs` | DU switch フィールドバインディング解析追加、Fn 型の expr_types 登録改善 |
| `src/transformer/statements/tests.rs` | TypeEnv 構築コード除去 |
| `src/transformer/expressions/tests.rs` | TypeEnv 構築コード除去 |
| `src/transformer/tests.rs` | TypeEnv 構築コード除去 |
| `src/transformer/test_fixtures.rs` | TypeEnv 関連ヘルパー除去 |

## タスク一覧

### T1: NarrowingGuard に Span 情報を追加 ✅

- **作業内容**: `expressions/patterns.rs` の `NarrowingGuard` enum の各バリアントに `var_span: swc_common::Span` フィールドを追加。`extract_narrowing_guard` で AST ノードから Span を取得して格納。`resolve_if_let_pattern` で type_env → FileTypeResolution フォールバック構成に変更。TypeResolver の CondExpr ハンドラに `cond.test` の解決を追加
- **完了条件**: ~~`resolve_if_let_pattern` が `type_env` を参照せず~~ → `resolve_if_let_pattern` が FileTypeResolution をフォールバックとして使用する（type_env 優先は T7 の any-narrowing 移行まで維持）。既存テスト全 GREEN
- **実績**: テスト 6 件追加。`get_type_for_var` ヘルパーを `type_resolution.rs` に追加。type_env の完全除去は AnyTypeAnalyzer の enum オーバーライドが FileTypeResolution に移行する T7 で実施
- **依存**: なし

### T2: FileTypeResolution に DU フィールドバインディング情報を追加 ✅

- **作業内容**: `pipeline/type_resolution.rs` に `DuFieldBinding { var_name: String, scope_start: u32, scope_end: u32 }` 構造体と `du_field_bindings: Vec<DuFieldBinding>` フィールドを追加。`is_du_field_binding(var_name, position)` メソッドを追加。`pipeline/type_resolver.rs` の `visit_switch_stmt` で DU switch を検出し、各 case のフィールドバインディングを記録
- **実績**: テスト 4 件追加（type_resolution 2 件 + type_resolver 2 件）。`detect_du_switch_bindings` メソッドと AST field access 収集ヘルパーを type_resolver.rs に追加
- **完了条件**: ✅
- **依存**: なし

### T3: TypeResolver の Fn 型 expr_types 登録を改善 ✅

- **作業内容**: `pipeline/type_resolver.rs` の `visit_var_decl` で、initializer がアロー関数式 / 関数式の場合、infer した `RustType::Fn` を変数宣言の initializer スパンだけでなく、変数自体の `Ident` スパンにも `expr_types` に記録する。これにより `get_expr_type(ident_expr)` が Fn 型を返すようになる
- **実績**: テスト 2 件追加。`visit_var_decl` の `declare_var` 呼び出し前に Fn 型の場合のみ `expr_types.insert(span, var_type)` を追加
- **完了条件**: ✅
- **依存**: なし

### T4: 分類 1（変数宣言の型登録）の TypeEnv insert/get 除去

- **作業内容**: `statements/mod.rs` の 6 箇所の `type_env.insert`（363,392,455,1195,1204,1208）を削除。`calls.rs:44` の `type_env.get` を `get_expr_type` に置換
- **T7 で先行除去済みの箇所**: `statements/mod.rs:119`（Any フォールバック → `any_enum_override` に置換）、`statements/mod.rs:1812`（typeof switch → `get_expr_type` に置換。現在は `try_convert_typeof_switch` 内で `get_expr_type` を使用）
- **レガシーコメント修正**: 変更対象ファイル内に `P-N` / `I-NNN` / `F-Nb` 等のイシュー番号やフェーズ番号のみのコメントがあれば、内容が伝わる説明コメントに書き換える
- **完了条件**: 分類 1 の全 insert/get が除去。全テスト GREEN
- **依存**: T3 ✅（Fn 型の expr_types 登録が必要）

### T5: 分類 2（Narrowing ガード）の TypeEnv 操作除去

- **作業内容**: `expressions/mod.rs:175-188` の `push_scope` / `insert` / `pop_scope` を削除。`get_expr_type` が narrowed_type を position-based で返すため、スコープ操作は不要。`resolve_if_let_pattern`（`patterns.rs:404`）の `type_env.get` フォールバックを除去し、`get_type_for_var` のみに一本化する
- **T7 で先行除去済みの箇所**: `expressions/patterns.rs:276`（`convert_instanceof` の LHS 型取得 → `get_expr_type(&bin.left).cloned()` に置換済み）
- **注意: TypeResolver `LogicalAnd/Or` の修正（T7 で実施済み）**: 以前は `LogicalAnd` が右辺のみ resolve し、左辺が Known なら左辺をスキップしていた。これにより `typeof x === "string" && typeof y === "number"` の `x` が `expr_types` に未登録だった。T7 で両辺を必ず resolve するよう修正済み。T5 の compound guard テストはこの修正を前提とする
- **レガシーコメント修正**: 変更対象ファイル内に `P-N` / `I-NNN` / `F-Nb` 等のイシュー番号やフェーズ番号のみのコメントがあれば、内容が伝わる説明コメントに書き換える
- **完了条件**: `expressions/mod.rs` と `resolve_if_let_pattern` から TypeEnv 参照が完全除去。全テスト GREEN
- **依存**: T1 ✅, T7 ✅

### T6: 分類 3（DU match arm）の TypeEnv 操作除去

- **作業内容**: `statements/mod.rs:2063-2078` の `push_scope` / `insert` / `pop_scope` を削除。`member_access.rs:256` の `type_env.get(&field).is_some()` を `self.tctx.type_resolution.is_du_field_binding(&field, span_position)` に置換
- **span_position の取得**: `member_access.rs` の `convert_member_expr` は `member: &ast::MemberExpr` を受け取る。`member.span.lo.0` を position として使用する
- **レガシーコメント修正**: 変更対象ファイル内に `P-N` / `I-NNN` / `F-Nb` 等のイシュー番号やフェーズ番号のみのコメントがあれば、内容が伝わる説明コメントに書き換える
- **完了条件**: DU switch 関連の TypeEnv 操作が完全除去。既存の DU テスト全 GREEN
- **依存**: T2 ✅

### T7: 分類 4-5（関数パラメータ + Any enum オーバーライド）の除去 ✅

- **作業内容**: パラメータ insert 全削除。AnyTypeAnalyzer を `pipeline/any_enum_analyzer.rs` に移動し、TypeResolver の前に実行。TypeResolver の `declare_var` で Any → enum 型を自動置換。`get_expr_type` / `get_type_for_var` にフォールバック不要（単一ソース）
- **実績**:
  - `pipeline/any_enum_analyzer.rs` 新規作成（テスト 5 件）
  - `functions/mod.rs:165,172` / `classes.rs:759` / `statements/mod.rs:2720` の insert 削除
  - `statements/mod.rs:119` の Any フォールバックを `any_enum_override` に置換
  - `convert_instanceof`（`patterns.rs:276`）の `type_env.get` → `get_expr_type` に先行置換
  - `try_convert_typeof_switch`（`statements/mod.rs:1812`）の `type_env.get` → `get_expr_type` に先行置換
  - `convert_constructor_body` から未使用の `params` パラメータを除去
  - TypeResolver の `LogicalAnd/Or` が左辺を resolve しないバグを修正
  - レガシーコメント（`F-3b`、`Pass 3.5`、`P4`、`P8`）を修正
- **完了条件**: ✅ 全テスト GREEN、clippy 0 警告、fmt 通過
- **依存**: なし

### T8: Transformer struct から type_env フィールド除去

- **作業内容**: `Transformer` struct から `type_env: TypeEnv` フィールドを削除。`for_module` から TypeEnv 初期化を削除。sub-transformer 作成（`expressions/functions.rs` の clone 3箇所、`functions/mod.rs` の構築箇所）から type_env を除去。`classes.rs` の `MethodContext` から type_env を除去
- **T7 で先行除去済みの箇所**: `functions/mod.rs` の `convert_fn_decl` と `convert_var_decl_to_fn` 内の sub-transformer 構築は既に `TypeEnv::new()` を使用。`classes.rs` の `convert_constructor_body` の sub-transformer 構築も同様。`statements/mod.rs` のネスト関数 sub-transformer も同様
- **レガシーコメント修正**: 変更対象ファイル内に `P-N` / `I-NNN` / `F-Nb` 等のイシュー番号やフェーズ番号のみのコメントがあれば、内容が伝わる説明コメントに書き換える。特に `type_resolver.rs` の `propagate_expected` 内の `P-1` 〜 `P-6` ラベルを確認する
- **完了条件**: `type_env` が Transformer struct と全 sub-transformer から完全除去。全テスト GREEN
- **依存**: T4, T5, T6, T7 ✅（全 TypeEnv 使用箇所の除去が完了）

### T9: TypeEnv 構造体の削除

- **作業内容**: `src/transformer/type_env.rs` から `TypeEnv` struct とその impl を削除。`pub use type_env::TypeEnv` を `mod.rs` から削除。`wrap_trait_for_position` / `TypePosition` は独立ユーティリティとして残存させる（使用箇所がある限り）
- **レガシーコメント修正**: 変更対象ファイル内に `P-N` / `I-NNN` / `F-Nb` 等のイシュー番号やフェーズ番号のみのコメントがあれば、内容が伝わる説明コメントに書き換える
- **完了条件**: `TypeEnv` 型がコードベースに存在しない。コンパイル通過
- **依存**: T8

### T10: テストコードの TypeEnv 依存除去

- **作業内容**: `transformer/tests.rs` / `statements/tests.rs` / `expressions/tests.rs` から TypeEnv の構築・操作を全て除去。テストヘルパー `convert_stmts_with_env` を `TctxFixture::from_source` ベースに置換。TypeEnv に依存していたテストは、TypeResolver 経由で同じ型情報が提供されることを検証するテストに書き換える
- **T7 で先行修正済みのテスト**: `test_instanceof_known_type_match_resolves_true`、`test_instanceof_known_type_mismatch_resolves_false`、`test_convert_instanceof_known_matching_type_returns_true`、`test_convert_instanceof_option_type_returns_is_some` — TypeEnv 構築を `TctxFixture::from_source` に置換済み
- **レガシーコメント修正**: テストコード内に `P-N` / `I-NNN` / `F-Nb` 等のイシュー番号やフェーズ番号のみのコメントがあれば、内容が伝わる説明コメントに書き換える
- **完了条件**: テストコードに `TypeEnv` への参照がゼロ。全テスト GREEN
- **依存**: T8 と並行実施可能（T4-T7 ✅ 完了後）

### T11: 品質チェック + ベンチマーク

- **作業内容**: `cargo test` 全 GREEN、`cargo clippy --all-targets --all-features -- -D warnings` 0 警告、`cargo fmt --all --check` 通過、Hono ベンチマーク実行して結果が維持または改善
- **最終レガシーコメント確認**: コードベース全体を `grep -rn 'P-[0-9]\|F-[0-9]\|#[0-9]\+:' src/` でスキャンし、残存するイシュー番号やフェーズ番号のみのコメントがないことを確認する。`I-NNN` は具体的なケースの来歴として許容するが、単独のラベル（`P-1`、`F-3b #1`）は不可
- **完了条件**: 全品質チェック通過。ベンチマーク結果が前回（86 clean / 132 errors）以上。レガシーコメント残存ゼロ
- **依存**: T9, T10

## テスト計画

| テスト | 検証内容 | 期待結果 |
|---|---|---|
| `test_narrowing_guard_has_span` | NarrowingGuard の Span フィールド | Span が正しく設定される |
| `test_du_field_binding_detection` | TypeResolver の DU switch フィールド解析 | case body 内のフィールド変数が binding として検出される |
| `test_du_field_binding_outside_scope` | スコープ外の判定 | case body 外では binding と判定されない |
| `test_fn_type_registered_in_expr_types` | 変数宣言の Fn 型推論 | `const fn = (x) => x` で Fn 型が expr_types に登録される |
| `test_get_expr_type_returns_fn_type` | calls.rs の Fn 型参照 | `get_expr_type` が TypeEnv なしで Fn 型を返す |
| `test_narrowing_without_type_env` | 三項演算子の narrowing | TypeEnv push_scope/pop_scope なしで narrowed type が使われる |
| `test_any_enum_override_via_resolution` | Any enum 型のオーバーライド | FileTypeResolution 経由で enum 型が取得される |
| 既存テスト全体 | 後方互換性 | 全テスト GREEN（挙動変更なし） |
| Hono ベンチマーク | 変換品質の維持 | 86 clean / 132 errors 以上 |

## 完了条件

- [ ] `TypeEnv` 構造体がコードベースから削除されている
- [ ] `type_env` フィールドが Transformer struct から削除されている
- [ ] 全ての型情報の読み取りが FileTypeResolution 経由に一本化されている
- [ ] テストコードに TypeEnv への参照がゼロ
- [ ] `cargo test` 全 GREEN
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] `cargo fmt --all --check` 通過
- [ ] Hono ベンチマーク結果が 86 clean / 132 errors 以上
