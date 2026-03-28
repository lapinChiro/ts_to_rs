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

開発コストと削減数の比率で選定。フェーズ移行基準（< 80）達成を目指す。

| 順序 | イシュー | 削減見込 | コスト | 概要 |
|------|---------|---------|--------|------|
| 1 | I-267 | ~10件 | 中 | return 式のオブジェクトリテラル型推定（OBJECT_LITERAL_NO_TYPE 最大サブセット） |
| 2 | I-221 | 9件 | 中 | unsupported intersection member type |
| 3 | I-281 | 6件 | 要設計 | typeof ローカル変数の型解決（パイプライン拡張） |
| 4 | I-200 | 5+2件 | 要設計 | マップ型 (`{ [K in keyof T]: V }`)。🔗 I-281, I-285 が前提 |
| 5 | I-285 | 3件 | 要設計 | 型パラメータキー indexed access（`T[K]`） |
| 6 | I-284 | 2件 | 中 | typeof qualified name（`typeof A.B.C`） |

### 順序の根拠

- **I-267 を最優先**: I-266 で構築済みの expected type 伝播基盤を return 文に拡張。最大の単一削減効果（~10 件）。106→~96 でフェーズ移行基準（<80）に向けた前進
- **I-221**: 独立した中規模タスク。9 件削減
- **I-281 を 3 番目**: 6 件に直接影響し、I-200 の前提条件。パイプラインの TypeCollector 拡張（const 宣言の型登録）が中核。`as const` リテラル型推論も含む
- **I-200 を I-281 の後に**: mapped type は I-281（typeof ローカル変数）と I-285（型パラメータキー）が前提。前提の解決で波及的に一部解消される可能性あり
- **I-285, I-284**: 独立した型解決の拡張。件数は少ないが変換正確性に直結

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
