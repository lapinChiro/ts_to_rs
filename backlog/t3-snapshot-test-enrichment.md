# T-3: スナップショットテスト内容の体系的拡充

## Background

`report/integration-test-review-2026-03-31.md` で 86 fixture 中 30+ 件が WEAK TEST（入力が機能を十分にテストしていない、重要なエッジケースが欠落）と判定された。テスト名と内容が大きく乖離しているもの（`closures` に外部変数キャプチャなし、`basic-types` に基本型の大半が欠落等）もある。

スナップショットテストは「出力が以前と同じか」を検証するもので「出力が正しいか」は検証しない。しかし、入力のカバレッジが不十分だと、変換ロジックの回帰を検出できない。各 fixture が「その機能の本質的なパターンを網羅している」状態にする。

## Goal

- WEAK TEST 判定 30+ 件を全て解消
- 各 fixture がテスト名に対応する機能の本質的パターンを網羅
- 既存の OK 判定 fixture にも、レビューで指摘されていない追加パターンがあれば補完

## Scope

### In Scope

1. WEAK TEST 判定の全 30+ fixture の入力拡充
2. テスト名と内容の乖離修正
3. 拡充に伴うスナップショットの更新
4. collecting モードで拡充した場合の unsupported スナップショット更新

### Out of Scope

- 新規 fixture の作成（既存 fixture の拡充のみ）
- E2E スクリプトの追加（T-4）
- 変換ロジックのバグ修正（S1/S2 は TODO で管理、fixture 拡充時に collecting モードを使用してエラーを許容）
- コンパイルテストの改善（T-2）

## Design

### Technical Approach

各 WEAK TEST fixture について、`.claude/rules/testing.md` のテストケース設計技法（同値分割、境界値分析、分岐網羅 C1）を適用し、欠落パターンを追加する。

#### 拡充対象と追加パターン一覧

**テスト名と内容が大きく乖離しているもの（最優先）**:

| Fixture | 現状 | 追加するパターン |
|---------|------|-----------------|
| `basic-types` | interface 1つのみ | `null`, `undefined`, `void`, `never`, `unknown`, タプル型、リテラル型の変数宣言 |
| `keyword-types` | `any`, `unknown` のみ | `never` 型、`void` 戻り値 |
| `closures` | 外部変数キャプチャなし | 読み取りキャプチャ、可変キャプチャ、複数変数キャプチャ |
| `functions` | 関数2つのみ | void 戻り値、複数 return パス、rest パラメータ |
| `mixed` | interface 1つ + function 1つ | テスト名を変更するか、複合パターン（interface + class + function + type alias の組み合わせ）を追加 |

**1ケースのみのもの**:

| Fixture | 追加するパターン |
|---------|-----------------|
| `nullish-coalescing` | チェーン (`a ?? b ?? c`)、`??=` 演算子、string/null のケース |
| `indexed-access-type` | ネスト (`A['B']['C']`)、ユニオンキーアクセス |
| `do-while` | break/continue 内包、ネスト |

**重要なエッジケースが欠落しているもの**:

| Fixture | 追加するパターン |
|---------|-----------------|
| `optional-fields` | optional フィールドへのアクセス、デフォルト値設定 |
| `array-destructuring` | rest 要素 (`[a, ...rest]`)、デフォルト値 (`[a = 0]`) |
| `object-destructuring` | ネスト分割代入、デフォルト値、rest パターン |
| `class-inheritance` | メソッドオーバーライド、`super.method()` 呼び出し |
| `async-await` | try/catch 内 await |
| `import-export` | リネームインポート (`import { A as B }`) |
| `enum` | enum メンバーアクセス (`Color.Red`) |
| `string-methods` | `.slice()`, `.indexOf()`, `.split()` |
| `unary-operators` | `typeof`, `void`, `~` (bitwise NOT) |
| `update-expr` | prefix increment/decrement (`++count`)、式中使用 (`arr[i++]`) |
| `regex-literal` | グローバルフラグ `/g`、特殊文字 |
| `void-type` | `void` を含む union (`string | void`) |
| `type-assertion` | `as unknown as T` (double assertion) |
| `math-api` | `Math.min`, `Math.round`, `Math.random()` |
| `unsupported-syntax` | decorator, namespace（collecting モードで unsupported として報告されることを確認） |
| `string-literal-union` | 関数パラメータ/戻り値での使用 |
| `generic-class` | メソッド、型制約、複数型パラメータ |
| `type-registry` | ジェネリクス型の登録・参照 |
| `general-for-loop` | break/continue、ネスト |
| `function-calls` | メソッドチェーン |
| `default-params` | オブジェクト型デフォルト値 |
| `discriminated-union` | switch 文での使用 |
| `narrowing-truthy-instanceof` | typeof ナローイング、null チェック |
| `ternary` | 型が分岐で異なるケース |
| `type-alias-utility` | `Required<T>`, `Pick<T, K>` |

#### 拡充の原則

1. **追加するパターンがトランスパイラで未対応の場合**: fixture を `collecting` モードに変更し、unsupported として報告されることを確認（T-1 で unsupported のスナップショット化が完了しているため）
2. **既存のパターンが不正確な出力を固定している場合**: 変換ロジックの修正は行わず、現状の出力をスナップショットとして固定する。出力の不正確さは TODO に記録
3. **テスト名の変更**: `mixed` のように実態と乖離が大きい場合、テスト名を変更するか内容を拡充する。テスト名変更は `integration_test.rs` のマクロ呼び出しとスナップショットファイル名の両方に影響する

### Design Integrity Review

- **Higher-level consistency**: fixture の拡充はスナップショットテストレイヤーに閉じる。E2E テストやコンパイルテストへの直接影響なし（コンパイルテストは全 fixture を自動検出するため、新たにコンパイルエラーが発生する可能性はある → collecting モードで対応）
- **DRY**: fixture 間での重複パターン（例: `basic-types` と `keyword-types` の `void` 型）は許容。各 fixture は独立した機能のテストであり、重複排除より網羅性を優先

Verified, 上記以外の問題なし。

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `tests/fixtures/*.input.ts` | 30+ ファイルの内容拡充 |
| `tests/snapshots/integration_test__*.snap` | 対応するスナップショット更新 |
| `tests/integration_test.rs` | 一部 fixture のモード変更（`transpile` → `collecting`） |

### Semantic Safety Analysis

Not applicable — テスト入力の拡充であり、型解決の変更なし。

## Task List

### T1: テスト名と内容が大きく乖離している 5 fixture の拡充

- **Work**: `basic-types`, `keyword-types`, `closures`, `functions`, `mixed` の 5 fixture に欠落パターンを追加。各 fixture で同値分割に基づくパーティションを列挙し、未カバーのパーティションを追加
- **Completion criteria**: 各 fixture がテスト名に対応する機能の主要パターンを全て含む。`cargo test --test integration_test` pass、スナップショット承認済み
- **Depends on**: None
- **Prerequisites**: T-1 完了（collecting モード使用時の unsupported スナップショット化）

### T2: 1 ケースのみの 3 fixture の拡充

- **Work**: `nullish-coalescing`, `indexed-access-type`, `do-while` に複数パターンを追加
- **Completion criteria**: 各 fixture が 3+ ケースを含む。スナップショット承認済み
- **Depends on**: None
- **Prerequisites**: T-1 完了

### T3: エッジケース欠落の 22 fixture の拡充

- **Work**: 上記の「重要なエッジケースが欠落しているもの」22 fixture に対し、各 fixture の設計意図に基づいて欠落パターンを追加。追加するパターンがトランスパイラ未対応の場合は collecting モードに変更
- **Completion criteria**: 各 fixture にレビュー指摘のエッジケースが追加されている。スナップショット承認済み
- **Depends on**: None
- **Prerequisites**: T-1 完了

### T4: スナップショット承認と確認

- **Work**: 全拡充済み fixture の `cargo insta review` を実行。各スナップショットの出力を目視確認し、新たに追加したパターンの変換結果が（正確か否かはともかく）存在することを確認。変換結果が欠落している場合は collecting モードへの変更を検討
- **Completion criteria**: 全スナップショット承認済み。WEAK TEST 判定が 0 件
- **Depends on**: T1, T2, T3
- **Prerequisites**: None

## Test Plan

- 各タスクの完了時: `cargo test --test integration_test` pass
- 全タスク完了後: `cargo test` 全体 pass（コンパイルテストで新たな失敗が発生していないか確認）
- スナップショットの内容確認: 拡充したパターンの変換結果が出力に含まれること

## Completion Criteria

1. WEAK TEST 判定の 30+ fixture 全てに欠落パターンが追加されている
2. 各 fixture がテスト名に対応する機能の主要パターンを網羅
3. 全スナップショットが `cargo insta review` で承認済み
4. `cargo test` 全 pass
5. 新たにコンパイルテストで失敗する fixture があれば skip リストに追加 + TODO 紐付け
