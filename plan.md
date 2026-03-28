# ts_to_rs 開発計画

## 現在のベースライン（2026-03-28）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 97/158 (61.4%) |
| エラーインスタンス | 106 |
| コンパイル(file) | 96/158 (60.8%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1401 |

| 1000行超ファイル | 0 |
| コンパイルテストスキップ | 8 件 |

OBJECT_LITERAL_NO_TYPE: I-112c Phase 1-3 + I-211 + I-224 + I-266 + I-268 + I-269 で 70→48 件に削減済み。I-195 修正で 48→51 件に増加。I-194+I-35 後 50 件（2026-03-28 実測）。

## 次の開発: エラーインスタンス効率削減（106→<80 目標）

根本的・基盤的な修正を先行し、波及効果で全体コストを最小化する。

| 順序 | イシュー | 直接削減 | 波及効果 | 概要 |
|------|---------|---------|---------|------|
| 1 | I-281 | 6件 | I-200 の前提充足、keyof/typeof 基盤 | typeof ローカル変数の型解決（TypeCollector 拡張） |
| 2 | I-267 | ~10件 | — | return 式のオブジェクトリテラル型推定 |
| 3 | I-221 | 9件 | — | unsupported intersection member type |
| 4 | I-285 | 3件 | I-200 の前提充足 | 型パラメータキー indexed access（`T[K]`） |
| 5 | I-200 | 5+2件 | Discriminant(3) 波及解消 | マップ型。🔗 I-281, I-285 が前提 |
| 6 | I-284 | 2件 | — | typeof qualified name（`typeof A.B.C`） |

**合計削減見込: 106 → ~69（-37件）**

### 順序の根拠

- **I-281 を最優先（基盤修正）**: TypeCollector に `const` 宣言の型登録を追加するパイプライン拡張。6 件を直接解消するだけでなく、I-200（mapped type）の前提条件を充足し、`keyof typeof`/`(typeof X)[key]` パターンの全般的な基盤となる。この修正なしでは I-200 を含む後続の全型解決が blocked。根本原因の解消が最終的なコストを最小化する
- **I-267 を 2 番目**: I-266 で構築済みの expected type 伝播基盤を return 文に拡張。最大の単一削減効果（~10 件）。独立タスクで I-281 と並行可能だが、I-281 の TypeCollector 拡張が OBJECT_LITERAL_NO_TYPE の一部にも波及する可能性があるため、I-281 後に実行して効果を正確に計測する
- **I-221**: 独立した中規模タスク。intersection member type の拡張で 9 件削減
- **I-285 を I-221 の後に**: 型パラメータキーの変換戦略を決定し実装。3 件直接解消 + I-200 の前提を充足
- **I-200 を最後**: mapped type は I-281 と I-285 の解決が前提。前提完了後に残存エラーを分析し、identity mapped type の簡約等を実装
- **I-284**: 独立した 2 件の修正。qualified typeof のドットパス解決 + グローバル変数登録

## 引継ぎ事項

設計判断の詳細は [doc/design-decisions.md](doc/design-decisions.md) を参照。

### コンパイルテストのスキップ（8 件）

| テスト名 | 原因イシュー | 概要 |
|----------|-------------|------|
| `indexed-access-type` | — | I-35 完了済み。スキップ原因は `Env` 型未定義（マルチファイルテストでカバー） |
| `trait-coercion` | I-201 | `null as any` → `None` が `Box<dyn Trait>` に代入不可 |
| `union-fallback` | I-202 | `Box<dyn Fn>` を含む enum に derive 不適合 |
| `any-type-narrowing` | I-209 | `serde_json::Value` → enum 型の自動変換 |
| `type-narrowing` | I-237+I-238 | `toFixed` 未変換 + `Display` 未生成 |
| `array-builtin-methods` | I-217+I-265 | filter/find の参照型 + Option 二重ラップ |
| `instanceof-builtin` | I-270c | メソッド impl 不在（struct 定義は生成済み） |
| `external-type-struct` | I-270 | builtins なし環境で外部型 struct 未生成 |

### 完了済みの大規模タスク

- **I-192 大規模ファイル分割**: 18 ファイル → 全ファイル 1000 行以下（T1-T13、テスト数不変 1369）。再発防止: `scripts/check-file-lines.sh` を `/quality-check` に組み込み済み
