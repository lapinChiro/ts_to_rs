# 未対応構文の検出・報告

## 背景・動機

現在、モジュールレベルの未対応宣言（`transform_decl` 内の `_ => Ok(vec![])`）はサイレントにスキップされ、ユーザーが変換結果の欠落に気づけない。文・式レベルではエラーになるが、モジュールレベルだけ挙動が異なる。

また、実際の OSS の TS ソースコードを変換して未対応構文を網羅的に把握したいというニーズがある。デフォルトではエラー終了しつつ、全ファイルを走査して未対応構文の一覧を JSON で出力するオプションが必要。

## ゴール

1. デフォルト: モジュールレベルの未対応構文でもエラー終了する（文・式レベルと統一）
2. `--report-unsupported` オプション: エラーで中断せず全ファイルを処理し、未対応構文の一覧を stdout に JSON 出力する

### 出力例

```json
[
  { "kind": "TsEnumDecl", "location": "src/main.ts:10:1" },
  { "kind": "TsModuleDecl", "location": "src/utils.ts:25:1" },
  { "kind": "CondExpr", "location": "src/main.ts:15:5" }
]
```

## スコープ

### 対象

- モジュールレベルの `transform_decl` でサイレントスキップしている箇所をエラーに変更
- `--report-unsupported` CLI オプションの追加
- 未対応構文を収集するバッファ機構
- 変換完了後の JSON 出力（stdout）

### 対象外

- 未対応構文の自動修正や代替変換の提案
- 出力形式のカスタマイズ（JSON 固定）

## 設計

### 技術的アプローチ

1. **エラー型の拡張**: 未対応構文を表す専用エラー型を追加。構文の種類（`kind`）とソース位置（`location`）を保持する
2. **transformer の変更**: `transform_decl` のワイルドカードをエラーに変更。未対応構文に遭遇したとき、専用エラーを返す
3. **収集モード**: `--report-unsupported` 時は、各ファイルの変換で未対応エラーが発生しても中断せず、エラーをバッファに蓄積する。変換可能な部分は通常通り出力する
4. **CLI 変更**: `--report-unsupported` フラグを `Args` に追加。有効時は全ファイル処理後に JSON を stdout に出力

### 影響範囲

- `src/main.rs` — CLI オプション追加、収集モードの制御
- `src/lib.rs` — 収集モード用の API 追加（未対応構文リストを返す）
- `src/transformer/mod.rs` — `transform_decl` のワイルドカードをエラーに変更
- `src/transformer/statements.rs` — 未対応構文のエラーに位置情報を付与
- `src/transformer/expressions.rs` — 同上

## 作業ステップ

- [ ] ステップ1: エラー型の拡張 — 未対応構文の種類と位置を保持する `UnsupportedSyntax` 型を定義
- [ ] ステップ2: transformer 修正 — `transform_decl` のワイルドカードを `UnsupportedSyntax` エラーに変更
- [ ] ステップ3: 既存テストの修正 — デフォルト挙動変更に伴うテストの更新
- [ ] ステップ4: 収集 API — 未対応構文を中断せず収集する `transpile_collecting` 関数を `lib.rs` に追加
- [ ] ステップ5: CLI オプション — `--report-unsupported` フラグの追加と JSON 出力
- [ ] ステップ6: スナップショットテスト — 未対応構文を含む fixture で E2E 検証

## テスト計画

- 正常系: 全て対応済みの TS ファイル → エラーなし、JSON 出力は空配列
- 正常系: 未対応構文を含むファイル → デフォルトでエラー終了
- 正常系: `--report-unsupported` で未対応構文を含むファイル → JSON に一覧出力
- 正常系: ディレクトリモードで複数ファイルに未対応構文 → 全ファイルの結果を集約
- 異常系: 構文エラー（パースエラー）と未対応構文の区別
- 境界値: 未対応構文が 0 件の場合の JSON 出力（`[]`）

## 完了条件

- モジュールレベルの未対応構文がデフォルトでエラーになる
- `--report-unsupported` で全ファイルを走査し、JSON 一覧が stdout に出力される
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
