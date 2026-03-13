# リファクタリング分析レポート

**基準コミット**: `1e94b7c`

## 調査範囲

全ソースファイル（12,607 行、16 ファイル）を通読して分析した。

## 検出した課題

### 1. `convert_param` と `convert_param_pat` の知識の重複 [既知・TODO 記載済み]

| 項目 | 値 |
|------|-----|
| 箇所 | `functions.rs:146-172` (`convert_param`) / `classes.rs:537-553` (`convert_param_pat`) |
| 深刻度 | 中 |
| 状態 | TODO に記載済み |

`Pat::Ident` からパラメータを抽出するロジックが 2 箇所に存在する。`convert_param` は追加で `Pat::Object`（分割代入）、`Pat::Assign`（デフォルト値）、resilient モード対応しているが、`convert_param_pat` は `Pat::Ident` のみ。Ident ケースのロジック（型注記取得 → `convert_ts_type` → `Param` 構築）は同一。

**推奨**: `convert_param_pat` を `convert_param` の Ident ケースに委譲する形で統合する。ただし `convert_param_pat` は resilient/fallback_warnings を扱わないため、それらを optional にするか、`convert_param_pat` を `convert_param(&pat, false, &mut vec![])` のラッパーにする。

### 2. `expressions.rs` の肥大化 [既知・TODO 記載済み]

| 項目 | 値 |
|------|-----|
| 箇所 | `transformer/expressions.rs` (2,635 行) |
| 深刻度 | 中 |
| 状態 | TODO に記載済み |

テストを含めると最大ファイル。以下の関心事が混在している:

- **メソッド名マッピング** (`map_method_call`: 442-563 行) — TS メソッド名を Rust メソッド名にマッピングするテーブル的なロジック
- **組み込み API 変換** (`convert_math_call`, `convert_number_static_call`, `convert_global_builtin`: 566-701 行) — `Math.*`, `Number.*`, `parseInt` 等のグローバル関数変換
- **オプショナルチェーン** (`convert_opt_chain_expr`, `extract_method_from_callee`: 130-215 行) — `x?.y` の変換

**推奨**: `map_method_call` + 組み込み API 変換を `expressions/builtin.rs` に、オプショナルチェーンを `expressions/optional.rs` に分離する候補。ただし、テスト（944-2635 行、約 1,700 行）が大半を占めるため、本体のロジックは約 940 行であり、即座に分割が必要な深刻さではない。

### 3. `generate_class_with_implements` の配置

| 項目 | 値 |
|------|-----|
| 箇所 | `transformer/mod.rs:452-500` |
| 深刻度 | 低 |

`generate_class_with_implements` はクラスから IR アイテムを生成するロジックであり、`classes.rs` の `generate_standalone_class`, `generate_child_class`, `generate_parent_class_items` と同レベルの責務を持つ。しかし `iface_methods: &HashMap<String, Vec<String>>` を受け取る必要があり、`classes.rs` の他の関数は `HashMap` を受け取らないため、`mod.rs` に配置されている。

**推奨**: `classes.rs` に移動し、引数として `iface_methods` を渡す。`classes.rs` の他の生成関数と一貫性を持たせる。影響は小さい。

### 4. `main.rs` のディレクトリ変換ロジックの重複

| 項目 | 値 |
|------|-----|
| 箇所 | `main.rs:125-209` (`transpile_directory_collecting`) / `main.rs:211-289` (`transpile_directory`) |
| 深刻度 | 中 |

2 関数で以下のロジックがほぼ同一:
- Pass 1: ファイル収集 → パース → レジストリ構築 → マージ（約 20 行）
- Pass 2: ファイルごとの出力パス計算 → ディレクトリ作成 → 書き出し（約 20 行）
- mod.rs 生成 + rustfmt 実行（約 15 行）

違いは Pass 2 の変換関数呼び出し（`transpile_with_registry` vs `transpile_collecting_with_registry`）と、結果の収集方法のみ。

**推奨**: 共通のディレクトリ変換ワークフローを抽出し、変換関数をクロージャまたはトレイトで差し替え可能にする。ただし `main.rs` は CLI エントリポイントであり、今後大きく変更される箇所ではないため、優先度は低い。

### 5. `map_method_call` の反復パターン

| 項目 | 値 |
|------|-----|
| 箇所 | `expressions.rs:489-556` |
| 深刻度 | 低 |

`find`, `some`, `every`, `forEach` は全て同じパターン（`object.iter().rust_method(args)` を生成）に従う。`map`, `filter` は追加で `.collect()` がチェーンされる。

```
iter_call → method_call → (optionally collect)
```

**推奨**: 現状は 4+2 のマッチアーム。テーブル駆動にすると短くなるが、各メソッドの挙動が今後分岐する可能性（TODO に `.reduce()`, `.sort()`, `.reverse()` 等の追加が記載）を考慮すると、マッチアームの方が拡張しやすい。**現時点では変更不要**。

### 6. `iface_methods` のパラメータ伝搬

| 項目 | 値 |
|------|-----|
| 箇所 | `mod.rs` の `transform_module` → `transform_module_item` → `transform_decl` → `transform_class_with_inheritance` |
| 深刻度 | 低 |

`iface_methods` は 4 関数を通過するが、実際に使用されるのは `transform_class_with_inheritance` の 1 箇所のみ。同様に `class_map` も末端でのみ使用される。

**推奨**: これは変換パイプラインの構造上避けられないパターン（コンテキスト情報を末端まで伝搬する必要がある）。`class_map` が既に同じパターンで伝搬されているため、一貫性はある。コンテキスト構造体にまとめる選択肢もあるが、YAGNI の観点からパラメータが 6-7 個を超えるまでは不要。**現時点では変更不要**。

### 7. `VecSpread` の生成ロジックの重複

| 項目 | 値 |
|------|-----|
| 箇所 | `generator/statements.rs:156-242` (`generate_vec_spread_let_stmts` + `generate_vec_spread_stmts`) / `generator/expressions.rs:170-197` (`generate_vec_spread`) |
| 深刻度 | 低 |

3 箇所で `[...arr]` → `arr.clone()` の最適化と、一般ケースの `Vec::new()` + `extend`/`push` パターンが繰り返されている。ただし各箇所で出力形式が微妙に異なる（式コンテキスト vs let 文コンテキスト vs return 文コンテキスト）。

**推奨**: segments を受け取ってヘルパー行リスト（`Vec::new()`, `push()`, `extend()`）を生成する共通関数を抽出し、各コンテキストで前後を付加する形にする。現時点では 3 箇所で合計約 90 行、共通化の効果は限定的。**今後 `VecSpread` のパターンが増えた時に実施**。

### 8. pre-scan の重複走査

| 項目 | 値 |
|------|-----|
| 箇所 | `mod.rs:155-219` (`pre_scan_classes` + `pre_scan_interface_methods`) |
| 深刻度 | 低 |

モジュールの `body` を 2 回走査している。1 パスに統合可能。

**推奨**: 1 パスに統合して `(HashMap<String, ClassInfo>, HashMap<String, Vec<String>>)` を返す関数にする。パフォーマンスへの影響はごく小さい（モジュールアイテム数は通常数十〜数百）。コードの明確さが下がる可能性があるため、**pre-scan が 3 種類以上に増えたときに実施**。

## 課題の優先度まとめ

| # | 課題 | 深刻度 | 推奨アクション | 状態 |
|---|------|--------|---------------|------|
| 1 | `convert_param` / `convert_param_pat` 重複 | 中 | PRD 化して統合 | TODO 記載済み |
| 2 | `expressions.rs` 肥大化 | 中 | PRD 化してモジュール分割 | TODO 記載済み |
| 3 | `generate_class_with_implements` 配置 | 低 | `classes.rs` に移動 | **新規** |
| 4 | `main.rs` ディレクトリ変換重複 | 中 | 共通関数抽出 | **新規** |
| 5 | `map_method_call` 反復パターン | 低 | 現時点で変更不要 | - |
| 6 | `iface_methods` パラメータ伝搬 | 低 | 現時点で変更不要 | - |
| 7 | `VecSpread` 生成ロジック重複 | 低 | パターン増加時に実施 | - |
| 8 | pre-scan 重複走査 | 低 | 3 種類以上になった時に実施 | - |

## 結論

- **即座にリファクタリングが必要な重大な設計問題はない**
- 既知の課題 2 件（#1, #2）は TODO に記載済みで、PRD 化の準備ができている
- 新規検出の課題のうち、#3（`generate_class_with_implements` 配置）は次の class 関連機能追加時に一緒に対応するのが自然
- #4（`main.rs` 重複）は独立して対応可能だが、CLI のエントリポイントは変更頻度が低く、優先度は低い
- #5〜#8 は YAGNI の原則に従い、条件が揃うまで保留が妥当
