# Research Scratchpad: ソフトウェアテストケース設計技法

Task: testing-techniques research
Created: 2026-03-29
Target: report/testing-techniques.md

---

## Problem Definition

### Research Questions
- Primary: ソフトウェアテストケース設計技法の体系的な理解
- Secondary: TypeScript→Rustトランスパイラプロジェクトへの適用方法

### Context
- Tech Stack: Rust + SWC AST + insta snapshot testing
- Test Structure: integration_test (snapshot), compile_test (cargo check), e2e_test (実行比較), cli_test
- Fixtures: 87ファイル、fixtures/*.input.ts → snapshot比較

---

## Research Log

### Entry 1: ISTQB ブラックボックステスト技法
Source: ISTQB CTFL Syllabus v4.0 + ToolsQA + SoftwareTestingHelp
Date: 2026-03-29
Confidence: High

#### 同値分割 (Equivalence Partitioning)
- 入力を「同じ処理をされる」グループに分割
- 各パーティションから1ケースのみテスト
- 有効パーティション + 無効パーティション
- 適用: 型変換規則（string→&str, number→f64 etc）

#### 境界値分析 (Boundary Value Analysis)
- 2値BVA: 境界値 + 隣接値
- 3値BVA: 境界前・境界・境界後
- ISTQB CTFL v4.0では「境界でバグが多い」根拠に基づく
- 適用: 数値型の変換境界、文字列長の処理

#### デシジョンテーブル
- 条件の組み合わせをテーブル形式で網羅
- N条件 → 最大2^N列（簡約化可能）
- 適用: 型変換ルールの優先度組み合わせ、複合型処理

#### 状態遷移テスト
- 有限状態機械のテスト
- 状態遷移図 + 状態遷移表
- 適用: パーサー状態、スコープ追跡、型収集フェーズ

#### ペアワイズテスト (All-pairs)
- Microsoft PICT ツール
- 全パラメータのペア組み合わせをカバー
- 10^20 → 217ケースに削減（95%以上の削減）
- 2-way interactionの欠陥を検出
- 3-way以上は見逃す可能性

#### エラー推測
- 経験・直感に基づくエラーの予測
- 境界値、ヌル値、空文字列、特殊文字
- 適用: TS固有の罠（undefined vs null, NaN, type coercion）

### Entry 2: ホワイトボックステスト技法
Source: Wikipedia MC/DC + NIST SP500-235 + GeeksforGeeks + Toronto CS course
Date: 2026-03-29
Confidence: High

#### Statement Coverage (C0)
- 全命令を最低1回実行
- 最弱のカバレッジ基準
- 現プロジェクト: llvm-cov 89%閾値

#### Branch Coverage (C1)
- 全分岐（true/false）をカバー
- C0より強い
- if-else, match arm等

#### Condition Coverage (C2)
- 各条件の真偽をカバー
- 分岐結果のカバーは保証しない

#### MC/DC (Modified Condition/Decision Coverage)
- DO-178B/DO-178C（航空ソフト）必須
- NASA安全重要ソフトウェアでも必須
- N条件 → N+1テストケースで達成（2^Nの代わり）
- 各条件が独立して決定全体に影響することを証明
- 適用: トランスパイラの複雑な条件ロジック

#### Path Coverage
- 全実行パスをカバー
- サイクロマティック複雑度 = 線形独立パス数
- 限界: ループがあると無限パス
- McCabe 1976による

#### Data Flow Testing (定義-使用ペア)
- 変数の定義(def)と使用(use)のペアをカバー
- du-pair: (定義箇所, 使用箇所)
- 定義済み未使用、未定義使用を検出
- 適用: 型変数の収集→参照の正確性

### Entry 3: コンパイラ/トランスパイラ特有テスト
Source: "How to Write a Compiler #6", insta.rs, WhiteFox paper, Snapshot testing articles
Date: 2026-03-29
Confidence: High

#### スナップショットテスト (Golden Master)
- テキスト→テキスト変換に最適（コンパイラ/トランスパイラ）
- 初回実行で「正解」を確定、以降は回帰テスト
- insta: assert_snapshot!, assert_yaml_snapshot!, etc.
- cargo insta review で変更を対話的にレビュー
- 現プロジェクトで採用済み (tests/integration_test.rs)

#### Differential Testing（差分テスト）
- リファレンス実装との比較
- 現プロジェクト: e2e_test.rs でTS実行結果 vs Rust実行結果を比較

#### Fuzzing / ランダムテスト
- YARPGen, CSmith等のランダムプログラム生成
- WhiteFox: LLMによるホワイトボックスファジング
- コンパイラバグを最も多く発見する手法のひとつ

#### AST変換のパターン網羅
- ASTノード種別ごとの変換テスト
- SWC AST: ExprKind, StmtKind, Decl, etc.の全種別
- パターンマッチの網羅性確認（Rust exhaustiveness check）

### Entry 4: 探索的テスト・経験ベース
Source: Session-based testing Wikipedia, Satisfice, TestRail
Date: 2026-03-29
Confidence: High

#### 探索的テスト
- Jonathan & James Bach, 2000年
- チャーター（1-2時間のセッション目標）
- 設計・実行・学習を同時並行
- SBT (Session-Based Test Management)
- フリースタイル vs ガイド付き

#### チェックリストベーステスト
- 経験からの観点リスト
- 再利用可能、但し形式化が難しい
- リスクベースの視点

### Entry 5: テストケース品質基準
Source: ISTQB, ACCELQ, quinnox
Date: 2026-03-29
Confidence: High

#### 独立性
- 1テスト = 1観点
- 実行順序に依存しない
- 他テストの状態に依存しない

#### 最小性
- テスト数を最小化しつつカバレッジを最大化
- ペアワイズ、MC/DC等が技法として存在

#### 再現性
- 同一環境で同一結果
- 決定論的テスト実行

#### トレーサビリティ
- 要件→テストケースの対応付け

---

## プロジェクト観察

現プロジェクト ts_to_rs のテスト構造:
1. **integration_test.rs**: スナップショットテスト（主力）
   - transpile() / transpile_collecting() / transpile_with_builtins() の3種
   - fixtures/XXX.input.ts → snapshot比較
   - 87フィクスチャ（2026-03時点）

2. **compile_test.rs**: コンパイル確認テスト
   - 生成Rustコードを実際にcargo checkでコンパイル
   - マルチファイルフィクスチャ対応
   - compile-lock（直列実行）

3. **e2e_test.rs**: 実行等価性テスト
   - TS実行（tsx）とRust実行の出力を比較
   - write_with_advancing_mtime でWSL2のmtime問題対処

4. **cli_test.rs**: CLIインターフェースのテスト

カバレッジ閾値: 89%（--fail-under-lines 89）
