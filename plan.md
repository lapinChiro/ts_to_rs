# ts_to_rs 開発計画

## 現在のベースライン（2026-03-30 B-2完了後）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 108/158 (68.4%) |
| エラーインスタンス | 66 |
| コンパイル(file) | 107/158 (67.7%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1400 |
| コンパイルテストスキップ | 11 件（builtins なし） / 10 件（builtins あり） |

### エラーカテゴリ内訳（66 件）

| カテゴリ | 件数 | 関連イシュー |
|----------|------|-------------|
| OBJECT_LITERAL_NO_TYPE | 36 | 複数（詳細: report/object-literal-no-type-investigation-2026-03-28.md） |
| OTHER | 8 | parseInt(2), delete(2), class expr(1), update target(1), rest type(1), array destr(1) |
| QUALIFIED_TYPE | 3 | I-36 |
| FN_TYPE_PARAM | 3 | I-259 |
| MEMBER_PROPERTY | 3 | |
| ASSIGN_TARGET | 3 | |
| その他 | 10 | OBJ_KEY(2), INTERFACE_MEMBER(2), 各1件×6 |

## 次の開発

### Phase B: 型変換範囲の拡張

#### B-2: I-285 + I-200 バッチ — indexed access + mapped type 改善 ✅

- `T[K]` 型パラメータキー対応（generics erasure）
- `[number]` on non-const の graceful fallback
- ネスト indexed access の再帰解決
- identity mapped type 検出拡張（symbol filter no-op 対応）
- standalone mapped type での identity 簡約
- **実績: -13 エラー（TYPE_ALIAS_UNSUPPORTED 10→0, INDEXED_ACCESS 3→0）**

#### B-4: I-284 — typeof qualified name（延期）

- 2 インスタンスとも複合パターン（typeof qualified + indexed access + utility type）
- typeof qualified 解決だけでは 0 エラー削減
- Phase B の indexed access 改善で graceful fallback 済み（エラーなしの Any 出力）
- **Phase C 以降で再評価**

#### Phase B 合計: **79 → 66（-13 エラー）** ✅

---

### Phase C: 高度な型推論（I-286c）

Phase B 完了後。

| 順序 | パターン | エラー削減 |
|------|---------|-----------|
| C-1 | typeof/instanceof ガード型推論 | 品質 |
| C-2 | プロパティアクセス型推論 | 品質 |
| C-3 | `\|\|`/`??` ジェネリクス制約解決 | H: ~8件 |
| C-4 | 呼び出し側型引数推論 | D: ~9件 |

---

### 全フェーズ合計

| フェーズ | エラー削減 | 状態 |
|---------|-----------|------|
| Phase A | -4 | ✅ 91→87 |
| Phase B-0 | 0（基盤修正） | ✅ |
| Phase B-1 (I-221) | -8 | ✅ 87→79 |
| Phase B-fix | 0（正確性保証） | ✅ |
| I-297 | 0（正確性修正） | ✅ サイレント意味変更解消 |
| Phase B-2 (I-285+I-200) | -13 | ✅ 79→66 |
| Phase C | ~-17 | ~66→~49 |

---

## 引継ぎ事項

設計判断: [doc/design-decisions.md](doc/design-decisions.md)。調査レポート: `report/` ディレクトリ。

### コンパイルテストのスキップ

**builtins なし（11 件）**:

| テスト名 | 原因イシュー | 概要 |
|----------|-------------|------|
| `indexed-access-type` | — | `Env` 型未定義（マルチファイルテストでカバー） |
| `trait-coercion` | I-201 | `null as any` → `None` が `Box<dyn Trait>` に代入不可 |
| `union-fallback` | I-202 | `Box<dyn Fn>` を含む enum に derive 不適合 |
| `any-type-narrowing` | I-209 | `serde_json::Value` → enum 型の自動変換 |
| `type-narrowing` | I-237+I-238 | `toFixed` 未変換 + `Display` 未生成 |
| `array-builtin-methods` | I-217+I-265 | filter/find の参照型 + Option 二重ラップ |
| `instanceof-builtin` | I-270c | メソッド impl 不在 |
| `external-type-struct` | I-270 | builtins なし環境で外部型 struct 未生成 |
| `ternary-union` | I-11 | 分岐値の enum variant ラッピング未実装 |
| `vec-method-expected-type` | I-289 | ビルトイン前提 |
| `intersection-empty-object` | — | 未使用型パラメータ T (E0091) |

**builtins あり（10 件）**: 上記から `vec-method-expected-type` を除いた 10 件。
