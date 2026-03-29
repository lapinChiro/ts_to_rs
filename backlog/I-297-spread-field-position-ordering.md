# I-297: spread とフィールドの位置順序によるサイレント意味変更の修正

## Background

`convert_object_lit`（`src/transformer/expressions/data_literals.rs:196-228`）がオブジェクトリテラルのプロパティを処理する際、`Prop`（明示フィールド）と `Spread` を別々のベクタ（`fields` と `spreads`）に分離して収集している。この設計によりソースコード上の位置順序が失われ、明示フィールドが常にスプレッドより優先される。

TypeScript では「後に書かれた方が勝つ」（rightmost wins）のルールで、`{ x: 1, ...a }` は `a.x` が `x: 1` を上書きする。しかし現在の変換は常に `x: 1` を保持し、`a.x` を無視する。これはサイレント意味変更であり、コンパイラが検出できない。

### 不正な変換例

| TS ソース | 現在の出力 | 正しい出力 |
|-----------|-----------|-----------|
| `{ x: 42, ...p }` | `Point { x: 42.0, y: p.y }` | `Point { x: p.x, y: p.y }` |
| `{ a: 1, ...cfg, c: 3 }` | `Config { a: 1.0, c: 3.0, b: cfg.b }` | `Config { a: cfg.a, b: cfg.b, c: 3.0 }` |

### 実プロジェクトでの発生

Hono ソースの `helper/cookie/index.ts` に `{ path: '/', ...opt, secure: true }` パターンが 3 箇所存在。`path` のデフォルト値が `opt.path` で上書きされるべきところ、現在の変換では常にデフォルト値 `'/'` が残る。

### 既存テストの問題

以下の 2 テストが不正な期待値を持っている（バグをテストが肯定している状態）:

- `test_convert_expr_object_spread_last_position_expands_remaining_fields`（objects.rs:95）: `{ x: 10, ...rest }` で `x: 10.0` を期待 → 正しくは `x: rest.x`
- `test_convert_expr_object_spread_middle_position_expands_remaining_fields`（objects.rs:136）: `{ a: 1, ...rest, c: 3 }` で `a: 1.0` を期待 → 正しくは `a: rest.a`

## Goal

1. オブジェクトリテラルの spread/フィールド位置順序が TypeScript のセマンティクスに忠実に変換される
2. snapshot テスト `object-spread` の `spreadAtEnd`/`spreadInMiddle` が正しい出力に更新される
3. E2E テスト `object_spread` で位置順序依存の値が TS/Rust 間で一致する
4. 既存のユニットテストが正しい期待値に修正され、新規テストが全パターンを網羅する

## Scope

### In Scope

- `convert_object_lit` のプロパティ処理ロジック再設計（位置順序保持）
- 登録済み型: 全フィールドの rightmost-wins 解決
- 未登録型: struct update syntax との整合（スプレッド後の明示フィールドのみリスト）
- 複数スプレッド: 登録済み型での位置順序考慮
- 既存ユニットテストの期待値修正（2 件）
- 新規ユニットテストの追加（位置順序の全パターン）
- snapshot テスト更新
- E2E テスト拡張（位置順序依存パターン追加）

### Out of Scope

- スプレッドソースの型がターゲット型と異なるケース（Partial<T> 等）— 現在の「スプレッドはターゲット型の全フィールドを提供」前提を維持
- I-274（フィールド名サニタイズ不一致）— 独立した問題
- I-235（未登録型の複数スプレッドフィールド展開）— 独立した問題

## Design

### Technical Approach

#### 核心: イベント列による位置順序保持

現在の「`fields` + `spreads` 分離」設計を廃止し、AST の出現順序を保持する単一のイベント列で処理する。

```rust
// 位置順序付きイベント
enum PropEvent {
    Explicit { key: String, value: Expr },
    Spread { expr: Expr },
}
```

#### ステップ 1: イベント列の構築

`obj_lit.props` をイテレートし、`Prop` → `PropEvent::Explicit`、`Spread` → `PropEvent::Spread` として順序を保持したまま `Vec<PropEvent>` に変換する。

#### ステップ 2a: 登録済み型のフィールド解決

`struct_fields` が取得できる場合、各フィールドに対してイベント列を**右から左**にスキャンし、最初にそのフィールドを提供するイベントを採用する:

- `PropEvent::Explicit { key, value }` で `key == field_name` → `value` を使用
- `PropEvent::Spread { expr }` → `Expr::FieldAccess { object: expr, field: field_name }` を使用

これにより rightmost-wins セマンティクスが正確に実現される。

```
TS: { x: 1, ...base, y: 2 }
イベント列: [Explicit(x,1), Spread(base), Explicit(y,2)]

field x: 右→左スキャン → Spread(base) が先にヒット → base.x
field y: 右→左スキャン → Explicit(y,2) が先にヒット → 2
field z: 右→左スキャン → Spread(base) がヒット → base.z
結果: S { x: base.x, y: 2, z: base.z }
```

struct update base は使用しない（全フィールドが明示的に列挙される）。

#### ステップ 2b: 未登録型の struct update syntax

`struct_fields` が取得できない場合、Rust の struct update syntax `S { fields..., ..base }` を使う。この syntax では `..base` が最低優先なので:

- スプレッドが 1 つの場合: スプレッドを struct update base に、スプレッド**より後**の明示フィールドのみをフィールドリストに含める。スプレッドより前の明示フィールドはスプレッドに上書きされるため省略する
- スプレッドが複数の場合: 現在と同様にエラー（型情報なしで複数スプレッドの解決は不可能）

```
TS: { x: 1, ...base, y: 2 }  (未登録型)
イベント列: [Explicit(x,1), Spread(base), Explicit(y,2)]

スプレッドのインデックス: 1
スプレッドより後の明示フィールド: [y: 2]
結果: S { y: 2, ..base }
```

```
TS: { ...base, x: 1 }  (未登録型)
結果: S { x: 1, ..base }  (現在と同じ — 変更なし)
```

```
TS: { x: 1, ...base }  (未登録型)
スプレッドより後の明示フィールド: []
結果: S { ..base }
```

#### ステップ 3: Option None-fill

登録済み型でスプレッドがない場合のみ、イベント列に含まれないフィールドのうち `Option<T>` 型のものを `None` で埋める。スプレッドがある場合はスプレッドが残フィールドを提供するため None-fill は不要（現在の動作と同じ）。

#### 複数スプレッド（登録済み型）の処理

現在の設計（B-fix で修正済み）を位置順序対応に拡張する。イベント列の右→左スキャンにより、複数スプレッドの優先順位が自然に解決される:

```
TS: { ...a, x: 1, ...b }
イベント列: [Spread(a), Explicit(x,1), Spread(b)]

field x: 右→左 → Spread(b) がヒット → b.x
field y: 右→左 → Spread(b) がヒット → b.y
```

これは B-fix の「後のスプレッドが優先」ルールと一致する。特別なケース処理は不要。

### PropEvent の配置

`PropEvent` は `convert_object_lit` のローカルな実装詳細であり、関数内のローカル enum として定義する。モジュールレベルや IR への追加は不要。

### Design Integrity Review

- **Higher-level consistency**: `convert_object_lit` の変更は IR（`Expr::StructInit`）のインターフェースを変更しない。`fields` と `base` の意味は同じまま。Generator への影響なし
- **DRY / Orthogonality**: 現在の single-spread パス（line 233-249）と multiple-spread パス（line 253-280）が統合され、単一のイベント列スキャンロジックに集約される。DRY 改善
- **Coupling**: `TypeRegistry` への依存は変化なし。`PropEvent` はローカル enum のため結合度の増加なし
- **Broken windows**: `test_convert_expr_object_spread_last_position_expands_remaining_fields` と `test_convert_expr_object_spread_middle_position_expands_remaining_fields` が不正な期待値を持つ broken window。本 PRD で修正する

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/expressions/data_literals.rs` | `convert_object_lit` の L196-297 を再設計 |
| `src/transformer/expressions/tests/objects.rs` | 既存テスト 2 件の期待値修正 + 新規テスト追加 |
| `tests/fixtures/object-spread.input.ts` | 変更なし（既にバグパターンを含む） |
| `tests/snapshots/integration_test__object_spread.snap` | snapshot 更新 |
| `tests/e2e/scripts/object_spread.ts` | 位置順序依存パターン追加 |

## Task List

### T1: ユニットテストの期待値修正 + 新規テスト追加（RED）

- **Work**: `src/transformer/expressions/tests/objects.rs` に対して以下を実施:
  1. **既存テスト修正**（不正な期待値 → 正しい期待値に変更）:
     - `test_convert_expr_object_spread_last_position_expands_remaining_fields`（L95）: `{ x: 10, ...rest }` の期待値を `x: rest.x, y: rest.y` に修正
     - `test_convert_expr_object_spread_middle_position_expands_remaining_fields`（L136）: `{ a: 1, ...rest, c: 3 }` の期待値を `a: rest.a, b: rest.b, c: 3.0` に修正
  2. **新規テスト追加 — 位置順序パターン**:
     - `test_spread_before_all_explicits_registered`: `{ ...base, x: 1, y: 2 }` 登録済み型 → 明示フィールドが勝つ（既存の `test_convert_expr_object_spread_with_override` と同等だが、全フィールド明示のケース）
     - `test_spread_after_all_explicits_registered`: `{ x: 1, y: 2, ...base }` 登録済み型 → スプレッドが全フィールドを上書き → `x: base.x, y: base.y`
     - `test_spread_between_explicits_registered`: `{ x: 1, ...base, y: 2 }` 登録済み型 3 フィールド → `x: base.x, y: 2, z: base.z`
     - `test_spread_after_explicit_unregistered`: `{ x: 1, ...base }` 未登録型 → `S { ..base }`（base のみ、明示フィールド省略）
     - `test_spread_between_explicits_unregistered`: `{ x: 1, ...base, y: 2 }` 未登録型 → `S { y: 2, ..base }`
     - `test_multiple_spreads_with_explicits_between`: `{ ...a, x: 1, ...b }` 登録済み型 → `x: b.x, y: b.y`（b が全勝）
     - `test_multiple_spreads_with_explicit_after_last`: `{ ...a, ...b, x: 1 }` 登録済み型 → `x: 1, y: b.y`（x は明示が勝つ）
     - `test_spread_only_registered`: `{ ...base }` 登録済み型 → `x: base.x, y: base.y`（スプレッドのみ、明示フィールドなし）
  3. **新規テスト追加 — テスト技法レビューで検出された欠落パターン（C1 分岐網羅 + 同値分割 + デシジョンテーブル）**:
     - `test_option_field_none_fill_when_omitted`: 登録済み型で `Option<T>` フィールドを省略 → `None` で自動補完される（C1: D22,D23 — None-fill パスが完全未テスト）
     - `test_multiple_spreads_unregistered_type_errors`: `{ ...a, ...b }` 未登録型 → エラー（C1: D21 — エラーパス未検証）
     - `test_string_key_property`: `{ "key": value }` → 文字列キーで正常動作（C1: D12 — `PropName::Str` 分岐）
     - `test_unsupported_property_kind_errors`: `{ get x() { return 1; } }` 等 → エラー（C1: D15 — 未対応 Prop 種別のエラーパス）
     - `test_spread_only_unregistered`: `{ ...base }` 未登録型 → `S { ..base }`（デシジョンテーブル: spread のみ + 未登録型の組み合わせ）
     - `test_unsupported_key_kind_errors`: `{ 42: value }` → 数値キーでエラー（C1: D13 — `PropName` の未対応バリアント）
     - `test_computed_and_normal_keys_mixed`: `{ [key]: "v", x: 1 }` → HashMap にならず struct パスに進む（同値分割: 計算キー混合の境界）
  4. **既存テストの網羅性確認**: 以下のテストは変更不要（正しい期待値）:
     - `test_convert_expr_object_spread_with_override`: `{ ...other, x: 10 }` → `x: 10, y: other.y` ✓
     - `test_convert_object_spread_unregistered_type_generates_struct_update`: `{ ...other, x: 10 }` → `S { x: 10, ..other }` ✓
     - `test_spread_multiple_overlapping_fields_first_spread_is_base`: `{ ...a, ...b }` ✓
- **Completion criteria**: テストが存在し、`cargo test` で **失敗する**（RED 状態）。修正済み 2 件 + 新規 15 件 = 計 17 件（Option None-fill やエラーパステストは現在の実装で pass する可能性があるため、fail するのは位置順序テストのみ）
- **Depends on**: なし

### T2: `convert_object_lit` の再設計（GREEN）

- **Work**: `src/transformer/expressions/data_literals.rs` の `convert_object_lit` メソッド（L143-304）を再設計:
  1. L196-228 の `fields`/`spreads` 分離ループを、`Vec<PropEvent>` 構築ループに置換
  2. L231-282 のスプレッド解決ロジックを以下に置換:
     - **登録済み型**: `struct_fields` の各フィールドに対してイベント列を右→左スキャンし、rightmost-wins で値を決定。struct update base は使用しない
     - **未登録型・単一スプレッド**: スプレッドのインデックスを特定し、スプレッド後の明示フィールドのみをリスト。スプレッドを struct update base に
     - **未登録型・複数スプレッド**: 現在と同様にエラー
  3. L284-297 の Option None-fill ロジックは、スプレッドなし＋登録済み型の場合のみ適用（現在と同じ条件）
  4. `PropEvent` は関数内ローカル enum として定義
- **Completion criteria**: T1 の全テスト（修正 2 件 + 新規 8 件）が PASS。既存の他のオブジェクトリテラル関連テストも全て PASS
- **Depends on**: T1

### T3: REFACTOR + コード品質

- **Work**:
  1. T2 で書いたコードのリファクタリング。旧コードのコメントが残っていれば削除。不要な分岐があれば統合
  2. `cargo clippy --all-targets --all-features -- -D warnings` で 0 warnings
  3. `cargo fmt --all --check` で 0 errors
  4. doc comment の更新（`convert_object_lit` のドキュメントに rightmost-wins セマンティクスを明記）
- **Completion criteria**: clippy 0 warnings, fmt 0 errors, `convert_object_lit` の doc comment が新しい挙動を正確に記述
- **Depends on**: T2

### T4: snapshot テスト更新

- **Work**:
  1. `cargo test test_object_spread` を実行し snapshot が fail することを確認
  2. `cargo insta review` で新しい snapshot を承認
  3. 承認後の snapshot の `spreadAtEnd` と `spreadInMiddle` の出力が正しいことを目視確認:
     - `spreadAtEnd`: `Point { x: p.x, y: p.y }` であること
     - `spreadInMiddle`: `Config { a: cfg.a, b: cfg.b, c: 3.0 }` であること
- **Completion criteria**: snapshot テストが PASS。`spreadAtEnd`/`spreadInMiddle` の出力が TypeScript セマンティクスに一致
- **Depends on**: T2

### T5: E2E テスト拡張

- **Work**: `tests/e2e/scripts/object_spread.ts` に位置順序依存のテストケースを追加:
  1. `spreadOverridesExplicit` 関数: `{ x: 42, ...base }` で `base.x` が使われることを出力検証
  2. `spreadMiddleOverride` 関数: `{ a: 1, ...base, c: 3 }` で `base.a` が使われ `c: 3` が勝つことを出力検証
  3. `spreadAfterAllExplicits` 関数: `{ x: 1, y: 2, ...base }` で `base.x`/`base.y` が使われることを出力検証
  4. `main()` から上記関数を呼び出し、TypeScript 実行時の stdout と Rust 実行時の stdout が一致することを確認
- **Completion criteria**: `cargo test test_e2e_object_spread_ts_rust_stdout_match` が PASS
- **Depends on**: T2

### T6: 最終検証

- **Work**:
  1. `cargo test > /tmp/test-result.txt 2>&1` で全テスト PASS を確認
  2. `cargo clippy --all-targets --all-features -- -D warnings` で 0 warnings
  3. `cargo fmt --all --check` で 0 errors
  4. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` でカバレッジ閾値クリア
- **Completion criteria**: 上記 4 コマンド全てが成功
- **Depends on**: T3, T4, T5

## Test Plan

### ユニットテスト（T1）

#### 位置順序パターン（修正 + 新規）

| テスト名 | パターン | 型登録 | 期待結果 |
|---------|---------|--------|---------|
| `spread_last_position` (修正) | `{ x:10, ...rest }` | 登録 | `x: rest.x, y: rest.y` |
| `spread_middle_position` (修正) | `{ a:1, ...rest, c:3 }` | 登録 | `a: rest.a, b: rest.b, c: 3.0` |
| `spread_before_all_explicits` (既存と同等) | `{ ...base, x:1, y:2 }` | 登録 | `x: 1, y: 2` |
| `spread_after_all_explicits` (新規) | `{ x:1, y:2, ...base }` | 登録 | `x: base.x, y: base.y` |
| `spread_between_explicits` (新規) | `{ x:1, ...base, y:2 }` | 登録(3f) | `x: base.x, y: 2, z: base.z` |
| `spread_after_explicit_unreg` (新規) | `{ x:1, ...base }` | 未登録 | `S { ..base }` |
| `spread_between_explicits_unreg` (新規) | `{ x:1, ...base, y:2 }` | 未登録 | `S { y: 2, ..base }` |
| `multi_spread_with_explicits_between` (新規) | `{ ...a, x:1, ...b }` | 登録 | `x: b.x, y: b.y` |
| `multi_spread_with_explicit_after` (新規) | `{ ...a, ...b, x:1 }` | 登録 | `x: 1, y: b.y` |
| `spread_only` (新規) | `{ ...base }` | 登録 | `x: base.x, y: base.y` |

#### テスト技法レビューで検出された欠落（新規）

| テスト名 | パターン | 技法 | 期待結果 |
|---------|---------|------|---------|
| `option_field_none_fill` | `{ x: 1 }` where y: Option | C1 (D22,D23) | `y: None` 自動補完 |
| `multi_spread_unreg_errors` | `{ ...a, ...b }` 未登録 | C1 (D21) | エラー |
| `string_key_property` | `{ "key": value }` | C1 (D12) | 正常動作 |
| `unsupported_prop_errors` | getter/method | C1 (D15) | エラー |
| `spread_only_unreg` | `{ ...base }` 未登録 | デシジョンテーブル | `S { ..base }` |
| `unsupported_key_errors` | `{ 42: value }` | C1 (D13) | エラー |
| `computed_and_normal_mixed` | `{ [k]: "v", x: 1 }` | 同値分割 | struct パス（HashMap にならない） |

#### デシジョンテーブル（条件の組み合わせ）

| 条件 | 値 |
|------|-----|
| スプレッド位置 | 先頭 / 中間 / 末尾 / なし |
| 型登録 | 登録 / 未登録 |
| スプレッド数 | 0 / 1 / 2+ |
| 明示フィールドの位置 | スプレッド前 / スプレッド後 / 両方 |

上記テストで主要な組み合わせをカバー。

### snapshot テスト（T4）

`object-spread.input.ts` の既存フィクスチャで `spreadAtEnd` / `spreadInMiddle` の出力が正しく変更されることを確認。

### E2E テスト（T5）

3 つの新規関数で、TypeScript の実行結果と Rust の実行結果が一致することをランタイムレベルで検証。

## Completion Criteria

1. `cargo test` 全テスト PASS（修正 2 件 + 新規 15 件のユニットテスト含む）
2. `cargo clippy --all-targets --all-features -- -D warnings` で 0 warnings
3. `cargo fmt --all --check` で 0 errors
4. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` でカバレッジ閾値クリア
5. snapshot テスト `object-spread` が TS セマンティクスに一致する正しい出力
6. E2E テスト `object_spread` が TS/Rust 間で一致する stdout を検証
7. ベンチマークエラー数に退行なし（79 件以下を維持）

**注**: 本 PRD はサイレント意味変更の修正であり、ベンチマークのエラーインスタンス数は変化しない。変換が正しくなる（不正な出力が正しい出力に変わる）ことが成果。
