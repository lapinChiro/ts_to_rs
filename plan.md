# ts_to_rs 開発計画

## 現在のベースライン（2026-03-29）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 105/158 (66.5%) |
| エラーインスタンス | 87 |
| コンパイル(file) | 104/158 (65.8%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1429 |

| 1000行超ファイル | 0 |
| コンパイルテストスキップ | 10 件（builtins なし） / 10 件（builtins あり） |

### エラーカテゴリ内訳（87 件、2026-03-29 実測）

| カテゴリ | 件数 | 関連イシュー |
|----------|------|-------------|
| OBJECT_LITERAL_NO_TYPE | 35 | 複数（詳細: report/object-literal-no-type-investigation-2026-03-28.md） |
| TYPE_ALIAS_UNSUPPORTED | 10 | Disc(16)=mapped(5)→I-200, Disc(15)=indexed(3: I-284=2件, mapped+indexed複合=1件→I-200), Disc(3)=conditional(2)→I-200波及 |
| INTERSECTION_TYPE | 9 | I-221（PRD作成済み） |
| OTHER | 8 | parseInt(2), delete(2), class expr(1), update target(1), rest type(1), array destr(1) |
| QUALIFIED_TYPE | 3 | I-36（`NodeJS.WritableStream`, `globalThis.ResponseInit` — 型位置の A.B 形式） |
| FN_TYPE_PARAM | 3 | I-259（rest param in fn type） |
| INDEXED_ACCESS | 3 | I-285（型パラメータキー indexed access） |
| その他 | 16 | ASSIGN_TARGET(3), MEMBER_PROPERTY(3), OBJ_KEY(2), INTERFACE_MEMBER(2), 各1件×6 |

### 完了済み

- **I-281**: typeof ローカル変数の型解決 — 6件削減（106→100）
- **I-267**: OBJECT_LITERAL_NO_TYPE 削減 — 10件削減（100→91）
- **I-276**: MethodSignature.has_rest 追加 — 全収集パスで rest パラメータ型を保持
- **I-286 Phase A**: sink-source expected type 伝播 — 4件削減（91→87）。Vec→Array マッピング、代入 LHS→RHS 伝播、`as T` 逆伝播、三項演算子 union 化
- **B-0a (I-287+I-288+I-289)**: テスト基盤整備。snapshot_test! マクロ全テスト移行 + resolve_with_builtins ヘルパー + ビルトインありコンパイルテスト追加

## 次の開発

### 設計原理

**割れ窓と基盤欠陥を先に修正する。** 機能開発（Phase B）の前に、放置すると影響範囲が拡大する設計問題とテスト基盤の欠陥を解消する。

---

### Phase B-0: 基盤修正（Phase B の前提）

Phase B で型変換を改善しても、テスト基盤が不確実なまま品質確認ができない。また、TypeResolver と Transformer のオーバーロード選択二重実装を放置すると、Phase B 以降の全メソッド関連改善で二重修正が必要になる。

#### B-0b: オーバーロード選択統一（I-290）

| 問題 | 影響 |
|------|------|
| Transformer が独自の簡易オーバーロード選択を持つ | TypeResolver の `select_overload` 改善が Transformer に波及しない |
| Transformer が Vec/String のメソッドシグネチャを取得不能 | 配列・文字列メソッドの変換品質が低いまま |
| TypeResolver の引数解決順序が不正 | `select_overload` Stage 4 が常に無効 |

放置すると Phase B 以降の全メソッド関連改善（I-261, I-270c, I-286c S3 等）で二重修正が必要。

---

### Phase B: 型変換範囲の拡張

Phase B-0 でテスト基盤とオーバーロード統一が完了した状態で実施。

#### B-1: I-221 — intersection メンバー型の網羅的サポート（📋 PRD作成済み）

- TsMappedType(5), TsUnionType(3), TsConditionalType(1) を intersection メンバーとして処理
- `& {}` 除去 + identity mapped type 簡約 + union 分配法則 + convert_ts_type フォールバック
- **削減: -9 エラー**

#### B-2: I-285 — 型パラメータキー indexed access

- `T[K]` where `K extends keyof T` の変換戦略
- I-200 の前提条件
- **削減: -3 エラー**

#### B-3: I-200 — マップ型

- Disc(16)=5, Disc(15)=1(mapped+indexed複合), Disc(3)=2
- 🔗 I-281(完了), I-285 が前提
- **削減: -8 エラー**

#### B-4: I-284 — typeof qualified name

- `typeof A.B.C` の再帰的解決 + グローバル変数登録
- **削減: -2 エラー**

#### Phase B 合計

**87 → 65（-22 エラー）**

---

### Phase C: 高度な型推論（I-286c）

Phase B 完了後。ジェネリクス制約解決・型引数推論等。

| 順序 | Sink パターン | 対象 | エラー削減 |
|------|-------------|------|-----------|
| C-1 | S7: typeof/instanceof ガード型推論 | typeof 被演算子型不明で `todo!()` → ガード文字列から型推論 | 品質 |
| C-2 | S8: プロパティアクセス型推論 | Unknown オブジェクトのメンバーアクセスでプロパティ名から型推論 | 品質 |
| C-3 | S2: `\|\|`/`??` ジェネリクス制約解決 | `options.verification \|\| {}` で制約 T の構造から V を推論 | H: ~8件 |
| C-4 | S3: 呼び出し側型引数推論 | `fn<T>(x: T)` の呼び出しで実引数型から T を推論 | D: ~9件 |

---

### 全フェーズ合計

| フェーズ | エラー削減 | 品質向上 | 目標値 |
|---------|-----------|---------|--------|
| Phase A | -4（実績） | ★★★ | 91→87 ✅ |
| Phase B-0 | 0 | ★★★（基盤修正） | 87（品質向上のみ） |
| Phase B | -22 | ★★ | 87→65 |
| Phase C | ~-17 | ★★★ | 65→~48 |

---

## 引継ぎ事項

設計判断の詳細は [doc/design-decisions.md](doc/design-decisions.md) を参照。

### 調査レポート

| レポート | 内容 |
|---------|------|
| `report/object-literal-no-type-investigation-2026-03-28.md` | OBJECT_LITERAL_NO_TYPE 50件の個別分類 |
| `report/i-221-intersection-investigation-2026-03-28.md` | INTERSECTION_TYPE 9件の根本原因分析 |
| `report/bottom-up-type-inference-analysis-2026-03-28.md` | Sink-source 逆伝播の統一原理と 10 パターン設計 |
| `report/fallback-type-inventory-2026-03-28.md` | 全フォールバック箇所の網羅的インベントリ（46+ 箇所） |

### コンパイルテストのスキップ（10 件）

| テスト名 | 原因イシュー | 概要 |
|----------|-------------|------|
| `indexed-access-type` | — | I-35 完了済み。`Env` 型未定義（マルチファイルテストでカバー） |
| `trait-coercion` | I-201 | `null as any` → `None` が `Box<dyn Trait>` に代入不可 |
| `union-fallback` | I-202 | `Box<dyn Fn>` を含む enum に derive 不適合 |
| `any-type-narrowing` | I-209 | `serde_json::Value` → enum 型の自動変換 |
| `type-narrowing` | I-237+I-238 | `toFixed` 未変換 + `Display` 未生成 |
| `array-builtin-methods` | I-217+I-265 | filter/find の参照型 + Option 二重ラップ |
| `instanceof-builtin` | I-270c | メソッド impl 不在 |
| `external-type-struct` | I-270 | builtins なし環境で外部型 struct 未生成 |
| `ternary-union` | I-11 | 分岐値の enum variant ラッピング未実装 |
| `vec-method-expected-type` | I-289 | ビルトイン前提（I-289 解決でスキップ不要になる見込み） |

### 完了済みの大規模タスク

- **I-192 大規模ファイル分割**: 18 ファイル → 全ファイル 1000 行以下（T1-T13、テスト数不変 1369）。再発防止: `scripts/check-file-lines.sh` を `/quality-check` に組み込み済み
