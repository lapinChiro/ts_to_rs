# E2E テスト基盤 詳細レビュー報告書

**日付**: 2026-03-31
**対象**: テスト基盤全体のアーキテクチャ、実行メカニズム、レイヤー間の整合性
**目的**: テスト基盤が「テストするべき事柄をきちんとテストできているか」を実行方法レベルから検証する

---

## 1. テストレイヤーの全体像

本プロジェクトには4つのテストレイヤーが存在する。

| レイヤー | ファイル | 件数 | 検証内容 |
|----------|----------|------|----------|
| **統合スナップショット** | `tests/integration_test.rs` | 86 fixtures / ~91 tests | 変換出力テキストの一致 |
| **E2E ランタイム** | `tests/e2e_test.rs` | 67 scripts + 1 multi | TS実行stdout = Rust実行stdout |
| **コンパイル検証** | `tests/compile_test.rs` | 全fixtures (skip除外) | 変換出力が `cargo check` を通るか |
| **CLI** | `tests/cli_test.rs` | 3 tests | CLIバイナリの動作 |

---

## 2. 各レイヤーの実行メカニズム詳細

### 2.1 統合スナップショットテスト

**実行フロー**:
```
TS source → transpile() / transpile_collecting() / transpile_with_builtins()
  → Rust source string → insta::assert_snapshot!() でスナップショット比較
```

**3つのモード**:

| モード | API | unsupported の扱い |
|--------|-----|-------------------|
| `snapshot_test!(name)` | `transpile()` | unsupported があればテスト失敗（Error） |
| `snapshot_test!(name, collecting)` | `transpile_collecting()` | unsupported を収集、**output のみ**スナップショット化 |
| `snapshot_test!(name, builtins)` | `transpile_with_builtins()` | ビルトイン型を読み込み + collecting |

**問題点A: collecting モードで `_unsupported` が検証されない**

```rust
// collecting variant のマクロ展開
let (output, _unsupported) = transpile_collecting(&input).unwrap();
insta::assert_snapshot!(output);  // _unsupported は捨てられる
```

9テスト（`callable_interface`, `intersection_empty_object`, `intersection_fallback`, `intersection_union_distribution`, `interface_methods`, `narrowing_truthy_instanceof`, `trait_coercion`, `anon_struct_inference`, `instanceof_builtin`）が collecting モードを使用しているが、**何がサポート外として報告されたかは一切検証されない**。唯一 `test_unsupported_syntax_collecting_output` だけが手動で unsupported の JSON をスナップショット化している。

**影響**: 
- construct signature（Factory）のような要素がサイレントにドロップされても検出されない
- 将来サポートを追加しても、unsupported リストから消えたことを検証するテストがない

**問題点B: スナップショットの「正しさ」は初回承認に依存**

スナップショットテストは「出力が変わったか」を検証するが、「出力が正しいか」は検証しない。初回の `cargo insta review` で不正確な出力を承認すると、以後そのまま「正解」として固定される。

### 2.2 E2E ランタイムテスト

**実行フロー**:
```
TS script (.ts) → transpile() → Rust source
  → rust-runner/src/main.rs に書き込み → cargo run → stdout 取得
TS script + "\nmain();\n" → tsx で実行 → stdout 取得
→ 両者の stdout を行単位で比較
```

**設計上の優れた点**:

1. **TS と Rust の実行結果を直接比較** — 変換の意味的正確性を最も直接的に検証できるアプローチ。スナップショットテストでは検出できないサイレント意味変更をランタイムレベルで検出可能
2. **Mutex による逐次実行** — `E2E_LOCK` で rust-runner プロジェクトの共有を安全に制御
3. **mtime 管理** — WSL2 での cargo 再ビルド検出問題に対する適切なワークアラウンド（`write_with_advancing_mtime`）
4. **多様なテストバリアント** — stdin, env, stderr, multi-file の各パターンをサポート

**問題点C: `transpile()` のみ使用（collecting/builtins なし）**

E2E テストは `transpile()` を使用する。これは unsupported syntax があるとエラーになるモード。つまり：
- **unsupported syntax を含むTSパターンはE2Eテストできない**
- collecting モードでのみスナップショットテストされている機能（callable interface, intersection fallback 等）は、ランタイム検証が一切行われない

**問題点D: stdout 比較のみで型安全性を検証できない**

E2E テストは stdout の行一致のみを検証する。以下のケースは検出できない：

1. **型の誤り** — `f64` を返すべき箇所が `i64` を返しても、`console.log` の出力が同じなら pass
2. **未使用コードの正しさ** — `main()` から呼ばれない関数は変換が壊れていても検出されない
3. **mutability の誤り** — `let` と `let mut` の違いは、当該テスト内で再代入がなければ検出されない
4. **型制約の緩和** — Rust 側で型が `Any` (`serde_json::Value`) にフォールバックしても、テスト範囲内の操作が成功すれば pass

**問題点E: rust-runner の依存関係がハードコード**

`tests/e2e/rust-runner/Cargo.toml` に依存クレートが手動で列挙されている：
```toml
[dependencies]
regex = "1.12"
scopeguard = "1.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
```

変換出力が新たなクレートを使用する場合（例: `chrono`, `num-bigint`）、ここに手動で追加する必要がある。追加を忘れると E2E テストがコンパイルエラーで失敗し、原因特定に時間がかかる。

**問題点F: 一時ファイルの残存リスク**

`execute_e2e` は TS 実行用の一時ファイル `{name}_exec.ts` を作成し、テスト終了時に削除する。しかし、テストが途中で panic すると `fs::remove_file` が呼ばれず一時ファイルが残る。`_exec.ts` ファイルが `.gitignore` されているかも確認が必要。

### 2.3 コンパイル検証テスト

**実行フロー**:
```
全 fixture → transpile_collecting() → Rust source
  → use crate:: 文を除去 → #![allow(unused, dead_code)] を追加
  → compile-check/src/lib.rs に書き込み → cargo check
```

**問題点G: `#![allow(unused, dead_code, unreachable_code)]` で警告を全抑制**

```rust
let full_source = format!(
    "#![allow(unused, dead_code, unreachable_code)]\n{auto_imports}{}",
    compilable_source
);
```

これにより、変換出力の以下の問題が隠蔽される：
- 不要な `let mut`（`unused_mut` 警告）
- 使われない struct / enum（`dead_code` 警告）
- 到達不能コード（`unreachable_code` 警告）

本来はこれらの警告も変換品質の指標として検出すべきだが、全て抑制されているため問題が蓄積する。

**問題点H: skip リストが長大化**

12 fixtures がコンパイルテストから除外されている。これらは「変換出力テキストは検証されるが、コンパイル可能性は未検証」の状態。特に以下の7件は**スナップショットのみ + コンパイル skip + E2E なし**の三重に検証が薄い：

| Fixture | 状態 |
|---------|------|
| `any-type-narrowing` | スナップショットのみ、実行もコンパイルも未検証 |
| `array-builtin-methods` | 同上 |
| `instanceof-builtin` | 同上 |
| `ternary-union` | 同上 |
| `trait-coercion` | 同上 |
| `type-narrowing` | 同上 |
| `union-fallback` | 同上 |

**問題点I: compile_test と e2e_test で `strip_internal_use_statements` が重複**

同一ロジックの関数が `e2e_test.rs:47` と `compile_test.rs:52` に別々に定義されている。DRY 違反であり、一方を変更しても他方に反映されない。

### 2.4 CLI テスト

**実行フロー**: CLIバイナリを直接実行し、`--report-unsupported` フラグの動作を検証。

**評価**: 3テストで `--report-unsupported` の基本動作（JSONフォーマット、空配列、デフォルトエラー）をカバー。最小限だが目的に合致。

**問題点J**: `--output` のみテスト。ディレクトリモード、`--format` オプション、複数ファイル入力など他のCLI機能はテストされていない。

---

## 3. レイヤー間の整合性分析

### 3.1 フィーチャーカバレッジの重複と隙間

| メトリクス | 値 |
|------------|------|
| スナップショット fixtures 総数 | 86 |
| E2E scripts 総数 | 67 + 1 multi |
| 両レイヤーで重複 | ~22 フィーチャー |
| スナップショットのみ（E2E なし） | ~64 fixtures |
| E2E のみ（スナップショットなし） | ~45 scripts |
| コンパイル skip (builtins なし) | 12 fixtures |
| コンパイル skip (builtins あり) | 10 fixtures |

**構造的ギャップ**: スナップショットと E2E のフィーチャー命名体系が異なる（例: snapshot `switch` vs E2E `switch_match`, snapshot `break-continue` vs E2E `loop_control`）。どのフィーチャーがどのレイヤーでカバーされているかの対応関係が不明瞭。

### 3.2 API 使用の非対称性

| テストレイヤー | 使用 API | builtins | unsupported |
|----------------|----------|----------|-------------|
| スナップショット | `transpile` / `transpile_collecting` / `transpile_with_builtins` | 一部あり | collecting で収集するが検証しない |
| E2E | `transpile` のみ | なし | エラーとして扱う |
| コンパイル | `transpile_collecting` / `transpile_with_builtins` | あり版となし版 | 収集するが検証しない |

E2E が `transpile()` のみ使用するため、`collecting` モードでしか動作しない機能（construct signature、一部のintersection等）はランタイム検証の対象外となる。

### 3.3 検証の段階性

理想的なテスト段階:
```
1. 変換成功（パースエラーなし）          ← 全レイヤーで検証
2. 出力テキストが期待通り               ← スナップショットで検証
3. 出力がコンパイル可能                 ← コンパイルテストで検証（skip除外あり）
4. 出力の実行結果が元TSと一致           ← E2Eで検証（67/86フィーチャーのみ）
5. unsupported の報告が正確             ← ほぼ未検証
```

段階4（ランタイム正確性）が最も価値の高い検証だが、全フィーチャーの **~25%** でしか両レイヤー（スナップショット + E2E）がカバーしていない。

---

## 4. E2E テストスクリプトの品質分析

### 4.1 共通パターン

全スクリプトが以下のパターンに従っている：
- `function main(): void { ... }` をエントリポイントとする
- `console.log()` で検証対象の値を出力
- E2E ランナーが `\nmain();\n` を追記して tsx で実行

例外: `async_await.ts` は `async function main(): Promise<void>` を使用（意図的）。

### 4.2 カバレッジが優れたスクリプト

| スクリプト | 評価理由 |
|-----------|---------|
| `discriminated_union.ts` | switch分岐、フィールドアクセス、等値比較、variant固有フィールド返却。5インスタンスで網羅的 |
| `null_option.ts` | null/undefined代入、nullチェック、nullable戻り値、配列内null。重要パターンを広くカバー |
| `object_spread.ts` | 基本spread、追加フィールド、複数spread、位置による上書きルール。5パターンで rightmost-wins を検証 |
| `closures.ts` | 純粋関数、読み取りキャプチャ、可変キャプチャの3パターン |
| `error_handling.ts` | try/catch基本 + 両方がreturnする関数。2パターン |

### 4.3 カバレッジが不十分なスクリプト

| スクリプト | 欠落 |
|-----------|------|
| `generics.ts` | `identity<T>` と `wrapValue<T,U>` のみ。型制約、ジェネリッククラス、デフォルト型引数なし |
| `classes.ts` | 単一クラス `Point` のみ。継承なし（`class_inheritance.ts` が別途あるが） |
| `typeof_check.ts` | `typeof n` を出力するだけ。タイプガードとしての使用（`if (typeof x === "string")`）なし |
| `interface_traits.ts` | 単一interface + 単一実装。複数interface実装、polymorphic使用なし |
| `async_await.ts` | 基本awaitチェーンのみ。try/catch内await、Promise.all なし |

### 4.4 E2E に存在しないフィーチャーカテゴリ

| 未テスト機能 | 重要度 |
|-------------|--------|
| 配列高階メソッド (`map`, `filter`, `reduce`) | 高 — 最頻出パターン |
| `do-while` ループ | 中 |
| getter / setter | 中 |
| abstract class / trait 変換 | 中 |
| conditional type | 低（型レベルのみ）|
| indexed access type | 中 |
| 型ナローイング全般（typeof ガード、instanceof） | 高 |

---

## 5. 構造的問題の総括

### 5.1 最大の構造的問題: 3つのレイヤーが独立に成長し、統合的なカバレッジ管理がない

各レイヤーが個別にテストを追加しており、「このフィーチャーはどのレベルまで検証されているか」を俯瞰する仕組みがない。結果として：

- 64 fixtures がスナップショットのみで E2E なし（テキスト一致は見るが実行しない）
- 45 E2E スクリプトがスナップショットなし（実行は見るが出力テキストを固定しない）
- 7 fixtures がスナップショットのみ + コンパイル skip で**最弱の検証状態**

### 5.2 collecting モードの unsupported 検証欠如

`_unsupported` を捨てる設計により、**何がドロップされたか**の検証が構造的に不可能。callable-interface の Factory 欠落はこの問題の典型例。

### 5.3 `#![allow(...)]` による品質情報の隠蔽

コンパイルテストが全警告を抑制しているため、変換品質の重要な指標（不要な mut、dead code、unreachable code）が見えない。

### 5.4 E2E の Mutex 逐次実行によるパフォーマンス問題

67 テストが全て `E2E_LOCK` で逐次実行される。各テストで `cargo run` を実行するため、テスト全体の実行時間が長い。rust-runner プロジェクトの共有が原因だが、テストごとに別ディレクトリを使うか、全スクリプトを1回のビルドでまとめて実行する方式に変更すれば並列化が可能。

---

## 6. 推奨改善

### 即座に対応すべき（構造的欠陥の修正）

#### R1: collecting モードの unsupported 検証追加

`snapshot_test!` マクロの `collecting` / `builtins` variant で `_unsupported` もスナップショット化する。

```rust
// 改善案
($name:ident, collecting) => {
    #[test]
    fn $name() {
        // ...
        let (output, unsupported) = transpile_collecting(&input).unwrap();
        insta::assert_snapshot!(concat!(stringify!($name), "_output"), output);
        let unsupported_json = serde_json::to_string_pretty(&unsupported).unwrap();
        insta::assert_snapshot!(concat!(stringify!($name), "_unsupported"), unsupported_json);
    }
};
```

これにより：
- Factory のドロップがスナップショットに記録される
- サポートを追加した際に unsupported リストの変化が検出される

#### R2: コンパイルテストの警告レベル改善

`#![allow(unused, dead_code, unreachable_code)]` を `#![allow(dead_code)]` のみに限定し、`unused_mut` と `unreachable_code` は検出可能にする。ただし、`unused_variables` や `unused_imports` はトランスパイラの性質上発生しやすいため、これらは `allow` を維持。

#### R3: 最弱検証状態の 7 fixtures に対応する E2E テスト追加

スナップショットのみ + コンパイル skip の 7 fixtures のうち、可能なものについてE2Eテストを作成し、ランタイム検証を追加する。コンパイルが通らないものは、まずコンパイルエラーの修正を優先。

### 中期的改善

#### R4: フィーチャー × レイヤーのカバレッジマトリクス作成

各フィーチャーがどのレイヤーでテストされているかを自動的に可視化するスクリプトを作成。新しいフィーチャーを追加する際に、全レイヤーでの検証を漏れなく計画できるようにする。

#### R5: `strip_internal_use_statements` の DRY 化

`e2e_test.rs` と `compile_test.rs` に重複する同名関数を共通モジュールに抽出する。

#### R6: E2E テストの並列化検討

現在の逐次実行モデルから、テストごとに独立した作業ディレクトリを使用するか、全スクリプトを1つの Rust バイナリにまとめてバッチ実行する方式への移行を検討。

### 長期的改善

#### R7: E2E テストでの `transpile_collecting` 対応

E2E ランナーに collecting モードのオプションを追加し、unsupported syntax を含む TS パターンもランタイム検証可能にする。これにより、callable-interface のような「部分的に変換される」フィクスチャの動作確認が可能になる。

#### R8: コンパイルテスト skip リストの削減計画

12 fixtures の skip 理由を TODO と紐付け、skip 解消の優先順位を明確にする。skip リストは技術的負債であり、放置すると変換品質の回帰を検出できないまま蓄積する。

---

## 7. 付録: テスト実行コマンド対応表

| テスト | コマンド | 実行時間目安 |
|--------|---------|-------------|
| 統合スナップショット | `cargo test --test integration_test` | 数秒 |
| E2E | `cargo test --test e2e_test` | 数分（逐次実行） |
| コンパイル | `cargo test --test compile_test` | 数十秒 |
| CLI | `cargo test --test cli_test` | 数秒 |
| 全テスト | `cargo test` | 数分 |
