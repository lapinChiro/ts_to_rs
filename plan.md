# ts_to_rs 開発計画

## 現在のベースライン（2026-03-28）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 95/158 (60.1%) |
| エラーインスタンス | 111 |
| コンパイル(file) | 94/158 (59.5%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1369 |
| 1000行超ファイル | 0 |
| コンパイルテストスキップ | 8 件 |

OBJECT_LITERAL_NO_TYPE: I-112c Phase 1-3 + I-211 + I-224 + I-266 + I-268 + I-269 で 70→48 件に削減済み。I-195 修正で 48→51 件に増加（パラメータ変換成功により隠れていた本体エラーが顕在化）。

## 次の開発: エラーインスタンス効率削減（110→~79 目標）

開発コストと削減数の比率で選定。フェーズ移行基準（< 80）達成を目指す。

| 順序 | イシュー | 削減見込 | コスト | 概要 |
|------|---------|---------|--------|------|
| ~~1~~ | ~~I-195~~ | ~~4件~~ | ~~低~~ | ~~arrow デフォルトパラメータの残存パターン~~ **完了（110件に削減）** |
| 1 | I-194 | 3件 | 低 | typeof 未登録識別子（fetch ×2, WebSocket ×1） |
| 2 | I-267 | ~10件 | 中 | return 式のオブジェクトリテラル型推定（OBJECT_LITERAL_NO_TYPE 最大サブセット） |
| 3 | I-221 | 9件 | 中 | unsupported intersection member type |
| 4 | I-35 | 6件 | 中 | indexed access type の非文字列キー対応（コンパイルテストスキップ解消にも寄与） |
| 5 | I-219 | 8件 | 要設計 | conditional type (`T extends U ? X : Y`) + infer type |
| 6 | I-200 | 5件 | 要設計 | マップ型 (`{ [K in keyof T]: V }`) |

### 順序の根拠

- **I-194 を先行**: 低コストで早期に 3 件削減
- **I-267 を 2 番目**: I-266 で構築済みの expected type 伝播基盤を return 文に拡張。最大の単一削減効果（~10 件）
- **I-221 → I-35**: 独立した中規模タスク。件数順に消化
- **I-219, I-200 を後半に**: 変換戦略の設計検討が必要なため、先行タスクでコードベース理解を深めてから着手。ただし「困難」は見送り理由ではなく、実現方法を徹底的に検証する

## 引継ぎ事項

設計判断の詳細は [doc/design-decisions.md](doc/design-decisions.md) を参照。

### コンパイルテストのスキップ（8 件）

| テスト名 | 原因イシュー | 概要 |
|----------|-------------|------|
| `indexed-access-type` | I-35 | 非文字列キーの indexed access（**今回 I-35 で解消予定**） |
| `trait-coercion` | I-201 | `null as any` → `None` が `Box<dyn Trait>` に代入不可 |
| `union-fallback` | I-202 | `Box<dyn Fn>` を含む enum に derive 不適合 |
| `any-type-narrowing` | I-209 | `serde_json::Value` → enum 型の自動変換 |
| `type-narrowing` | I-237+I-238 | `toFixed` 未変換 + `Display` 未生成 |
| `array-builtin-methods` | I-217+I-265 | filter/find の参照型 + Option 二重ラップ |
| `instanceof-builtin` | I-270c | メソッド impl 不在（struct 定義は生成済み） |
| `external-type-struct` | I-270 | builtins なし環境で外部型 struct 未生成 |

### 完了済みの大規模タスク

- **I-192 大規模ファイル分割**: 18 ファイル → 全ファイル 1000 行以下（T1-T13、テスト数不変 1369）。再発防止: `scripts/check-file-lines.sh` を `/quality-check` に組み込み済み
