# T-4: E2E テストカバレッジの体系的拡充

## Background

E2E テスト（`tests/e2e_test.rs`）は「TS 実行結果 = Rust 実行結果」を比較する最も価値の高い検証レイヤーだが、スナップショット fixture 86 件中 ~64 件に対応する E2E テストが存在しない（`report/e2e-test-infrastructure-review-2026-03-31.md` §3.1）。

また、E2E テストが存在する 22 フィーチャーでも、一部は入力パターンが不十分（generics, classes, typeof_check, interface_traits, async_await）。

E2E テストはサイレント意味変更（Tier 1）を唯一検出できるレイヤーであり、このカバレッジ不足は品質担保上の最大のリスクである。

### E2E に存在しない重要フィーチャー

| フィーチャー | 重要度 | 理由 |
|-------------|--------|------|
| 配列高階メソッド (map, filter, reduce) | 高 | 最頻出パターン |
| 型ナローイング (typeof ガード, instanceof) | 高 | サイレント意味変更リスク大 |
| getter / setter | 中 | クラスの基本機能 |
| do-while ループ | 中 | 制御フロー基本 |
| abstract class / trait | 中 | OOP 基本パターン |
| 正規表現リテラルの使用 | 中 | regex クレートの統合検証 |
| string メソッド (.slice, .indexOf, .split) | 中 | 文字列操作の頻出パターン |

## Goal

- スナップショット fixture の 50% 以上に対応する E2E テストが存在（現状 ~25% → 50%+）
- 既存 E2E スクリプトの不十分なカバレッジを補完
- 重要度「高」のフィーチャーは全て E2E カバー
- E2E テストで S1（サイレント意味変更）が検出可能な状態

## Scope

### In Scope

1. 新規 E2E スクリプトの作成（15+ 件）
2. 既存 E2E スクリプトの強化（5 件）
3. `e2e_test.rs` へのテスト関数登録

### Out of Scope

- E2E 基盤の構造変更（T-1 で完了済み前提）
- collecting モード対応の E2E（`transpile()` で変換可能なパターンのみ対象）
- 変換ロジックのバグ修正
- スナップショットテストの拡充（T-3）

## Design

### Technical Approach

#### 新規 E2E スクリプト作成方針

各スクリプトは以下のパターンに従う:
- `function main(): void { ... }` エントリポイント
- `console.log()` で検証対象の値を出力
- 複数のケースを含み、各ケースの出力が異なる値になるよう設計
- 出力値がトランスパイラの変換品質に敏感であること（型の違い、ミュータビリティの違い、制御フローの違いが出力に反映される）

#### 新規スクリプト一覧

| スクリプト名 | 対応 fixture | テスト内容 |
|-------------|-------------|-----------|
| `array_higher_order.ts` | array-builtin-methods | map, filter, find, reduce, forEach, some, every |
| `typeof_narrowing.ts` | type-narrowing, any-type-narrowing | typeof ガードで分岐、narrowing 後のメソッド呼び出し |
| `getter_setter.ts` | getter-setter | getter/setter の定義と使用、get のみ/set のみ |
| `do_while.ts` | do-while | do-while ループ、break/continue 内包 |
| `abstract_class.ts` | abstract-class | 抽象メソッド定義、具象クラスで実装、polymorphic 使用 |
| `regex_ops.ts` | regex-literal | regex リテラル作成、test/replace 使用 |
| `string_methods.ts` | string-methods | slice, indexOf, split, startsWith, includes, trim |
| `math_ops.ts` | math-api | floor, ceil, abs, max, min, pow, sqrt |
| `optional_fields.ts` | optional-fields | optional フィールドのアクセス、undefined チェック |
| `for_variations.ts` | general-for-loop, do-while | C-style for, for-of, for-in, do-while, break/continue |
| `type_alias.ts` | type-alias-utility | Partial, type alias の使用 |
| `fn_expr.ts` | fn-expr | 関数式、IIFE、クロージャとしての関数式 |
| `multivar_decl.ts` | multi-var-decl | 複数変数宣言の展開 |
| `class_advanced.ts` | class-default-params, param-properties | コンストラクタデフォルト値、パラメータプロパティ |
| `unary_ops.ts` | unary-operators | 否定、ビット反転、typeof |

#### 既存スクリプト強化

| スクリプト | 追加パターン |
|-----------|-------------|
| `generics.ts` | ジェネリッククラスのインスタンス化、型制約 (`<T extends ...>`) |
| `classes.ts` | static メソッド、private フィールド |
| `typeof_check.ts` | typeof をタイプガードとして使用 (`if (typeof x === "string")`) |
| `interface_traits.ts` | 複数 interface 実装、polymorphic な変数代入 |
| `async_await.ts` | try/catch 内 await |

#### E2E スクリプト作成時の注意事項

1. **`transpile()` で変換可能なパターンのみ使用**: unsupported syntax を含むとテスト自体がエラーになる。未対応パターンは使わない
2. **出力の比較精度**: 浮動小数点の出力は `toFixed` 等で桁数を固定するか、整数のみ使用
3. **外部依存なし**: `fs`, `process` 以外のモジュールは使用しない（rust-runner に対応する依存がない）
4. **事前検証**: 各スクリプトを `tsx` で実行し、期待通りの出力が得られることを確認してから `transpile()` → `cargo run` のテストを行う

### Design Integrity Review

- **Higher-level consistency**: E2E テストの追加パターンは既存の `run_e2e_test` 関数を使用。新しい実行パターン（stdin, env 等）は既に `e2e_test.rs` に定義済み
- **DRY**: 各 E2E スクリプトは独立。スクリプト間での共通コードは不要（各スクリプトは自己完結）
- **Coupling**: 新規テスト関数は `e2e_test.rs` に追加するのみ。他ファイルへの影響なし

Verified, 上記以外の問題なし。

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `tests/e2e/scripts/*.ts` | 新規 15+ スクリプト作成 |
| `tests/e2e/scripts/*.ts` | 既存 5 スクリプトへのパターン追加 |
| `tests/e2e_test.rs` | 新規テスト関数 15+ 件追加 |

### Semantic Safety Analysis

Not applicable — テストの追加であり、型解決の変更なし。

## Task List

### T1: 重要度「高」の新規スクリプト作成（3 件）

- **Work**: `array_higher_order.ts`, `typeof_narrowing.ts`, `string_methods.ts` を作成。各スクリプトを `tsx` で実行し出力確認。`e2e_test.rs` にテスト関数登録
- **Completion criteria**: 3 スクリプトの E2E テストが pass。各スクリプトが 5+ ケースを含む
- **Depends on**: None
- **Prerequisites**: T-1 完了（test_helpers.rs が存在すること）

### T2: 重要度「中」の新規スクリプト作成（12 件）

- **Work**: getter_setter, do_while, abstract_class, regex_ops, math_ops, optional_fields, for_variations, type_alias, fn_expr, multivar_decl, class_advanced, unary_ops を作成。各スクリプトを `tsx` で実行し出力確認。`e2e_test.rs` にテスト関数登録
- **Completion criteria**: 12 スクリプトの E2E テストが pass
- **Depends on**: None
- **Prerequisites**: T-1 完了

### T3: 既存スクリプトの強化（5 件）

- **Work**: generics, classes, typeof_check, interface_traits, async_await の各スクリプトに欠落パターンを追加。`tsx` での出力確認後、E2E テスト pass を確認
- **Completion criteria**: 5 スクリプト全てで追加パターンの E2E テストが pass
- **Depends on**: None
- **Prerequisites**: T-1 完了

### T4: E2E テストの全体 pass 確認

- **Work**: `cargo test --test e2e_test` で全テスト pass を確認。失敗するテストがあれば原因を特定し、変換ロジックの問題であれば TODO に記録してスクリプトから該当パターンを一時的に除外
- **Completion criteria**: E2E テスト全 pass。除外した場合は TODO に記録済み
- **Depends on**: T1, T2, T3
- **Prerequisites**: None

## Test Plan

- 各新規スクリプト: `tsx` での実行確認 → `cargo test --test e2e_test -- test_e2e_<name>` で個別テスト pass
- 全体: `cargo test --test e2e_test` で全テスト pass
- 回帰確認: `cargo test` 全体 pass

## Completion Criteria

1. 新規 E2E スクリプト 15+ 件が作成・登録されている
2. 既存 E2E スクリプト 5 件が強化されている
3. スナップショット fixture の 50%+ に対応する E2E テストが存在
4. 重要度「高」のフィーチャー（配列高階メソッド、型ナローイング、string メソッド）が全て E2E カバー
5. `cargo test --test e2e_test` 全 pass
6. `cargo test` 全体 pass
