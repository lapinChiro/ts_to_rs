# ts_to_rs 開発計画

## 現在のベースライン（2026-03-30 C-3完了後）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 110/158 (69.6%) |
| エラーインスタンス | 61 |
| コンパイル(file) | 109/158 (69.0%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1454 |
| コンパイルテストスキップ | 11 件（builtins なし） / 10 件（builtins あり） |

### エラーカテゴリ内訳（61 件）

| カテゴリ | 件数 | 主要イシュー |
|----------|------|-------------|
| OBJECT_LITERAL_NO_TYPE | 29 | I-301(7), I-305(2), I-306(1), I-300 imported(11), 他(8) |
| OTHER | 10 | parseInt(2), delete(2), class expr(1), 他 |
| QUALIFIED_TYPE | 3 | I-36 |
| FN_TYPE_PARAM | 3 | I-259 |
| MEMBER_PROPERTY | 3 | |
| ASSIGN_TARGET | 3 | |
| その他 | 10 | OBJ_KEY(2), INTERFACE_MEMBER(2), 各1件×6 |

---

## ロードマップ

### フェーズ移行基準

| フェーズ | 基準 | 現状 |
|---------|------|------|
| **現在: 変換率改善** | エラー < 60 件 | 61 件。C-4+C-5 で達成見込み |
| **次: コンパイル品質** | ディレクトリコンパイルエラー 0 | 残 2 ファイル（I-273） |
| **その後: DX + 品質** | コンパイルテストスキップ 0 | 残 11 件 |

### Phase R-1: コンポーネント責務境界の正常化（次の開発）

構造リファクタリング。変換ロジック変更なし。後続の全作業で正しい依存方向を前提にできるようにする。

| 対象 | 内容 | 効果 |
|------|------|------|
| 逆方向依存 | `transformer/any_narrowing.rs` → `pipeline/` に移動 | registry → transformer の逆方向依存解消 |
| 二重処理 | registry/enums.rs の any-narrowing 登録を pipeline 側に一本化 | 重複排除 |
| re-export 残骸 | `transformer/types/` 除去、233 テストを `pipeline/type_converter/tests/` に移動 | 技術的負債解消 |

PRD: `backlog/r1-component-boundary-cleanup.md`

### Phase C-4: TypeRegistry 型登録基盤

R-1 で依存方向が正常化された状態で、TypeRegistry の型登録基盤を改善する。

| 対象 | 内容 | 効果 |
|------|------|------|
| I-307 | TypeAlias の TsTypeRef RHS 登録（`type BodyCache = Partial<Body>` 等） | 基盤改善。一部の I-301 ケースが不要になる可能性 |
| I-305 | callable interface の return 型解決（`GetCookie` 等） | OBJECT_LITERAL_NO_TYPE ~2件 |
| I-308 | `resolve_type_params_in_type` の indexed access 複合名解決（`E['Bindings']`） | OBJECT_LITERAL_NO_TYPE ~1件 |

PRD: `backlog/c4-type-registry-foundation.md`。C-4 完了後にベンチマークを取り直し、残存パターンを再評価してから I-312/C-5 を設計する。

### Phase R-2: TypeDef の TS 型メタデータ分離（I-312）

C-4 で `call_signatures` を TypeDef に追加した後、TypeDef 全体の設計を再検討する。TypeDef のフィールド型を TS 型のまま保持する設計に変更し、registry の責務を「純粋な型メタデータ収集」に正す。C-5 の匿名構造体生成のアプローチに影響するため、C-5 設計前に確定させる。

### Phase C-5: 匿名構造体 + 残存パターン

R-2 完了後の設計に基づき実施。現時点の見込み:

| 対象 | 内容 | 効果 |
|------|------|------|
| I-301 | 型注釈なしオブジェクトリテラル → 匿名構造体自動生成 | ~7件（C-4 の結果で変動） |
| I-306 | `.map()` callback への戻り値型伝播 | ~1件 |

### Phase D: コンパイル品質

| 対象 | 内容 | 効果 |
|------|------|------|
| I-273 | trait/struct 混同修正（`extends` の変換精度） | ディレクトリコンパイル 156→157+/158 |
| I-310 | HashMap computed access の transformer 変換 | コンパイルエラー削減 |
| I-217+I-265 | filter/find の参照型 + Option 二重ラップ | コンパイルテストスキップ解消 |
| I-237+I-238 | toFixed 変換 + Display 自動生成 | コンパイルテストスキップ解消 |
| I-311 | 型引数推論結果の引数 expected type フィードバック | 型推論精度向上 |

### Phase E: DX + 生成コード品質

| 対象 | 内容 | 効果 |
|------|------|------|
| I-30 | Cargo.toml 依存追加 | I-183, I-34 のゲート解除 |
| I-182 | Hono クリーンファイルのコンパイルテスト CI 化 | 回帰検出自動化 |
| I-282+I-283 | デフォルトパラメータの DRY 化 + unwrap_or_else | 生成コード品質 |

### 長期ビジョン

| マイルストーン | 指標 |
|---------------|------|
| 変換率 80% | クリーン 126/158（現在 110） |
| コンパイル率 80% | ファイルコンパイル 126/158（現在 109） |
| コンパイルテストスキップ 0 | 全 fixture がコンパイル通過 |

---

## 引継ぎ事項

設計判断: [doc/design-decisions.md](doc/design-decisions.md)。調査レポート: `report/`。

### コンパイルテストのスキップ（11 件）

| テスト名 | 原因 | 概要 |
|----------|------|------|
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

builtins あり（10 件）: 上記から `vec-method-expected-type` を除く。
