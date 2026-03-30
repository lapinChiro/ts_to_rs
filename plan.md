# ts_to_rs 開発計画

## 現在のベースライン（2026-03-30 C-2完了後）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 110/158 (69.6%) |
| エラーインスタンス | 62 |
| コンパイル(file) | 109/158 (69.0%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1588 |
| コンパイルテストスキップ | 11 件（builtins なし） / 10 件（builtins あり） |

### エラーカテゴリ内訳（62 件）

| カテゴリ | 件数 | 関連イシュー |
|----------|------|-------------|
| OBJECT_LITERAL_NO_TYPE | 32 | I-300（関数引数 ~14件）、I-301（型注釈なし ~3件）、I-302（this.field ~2件）、その他 ~13件 |
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

#### C-0: resolve_struct_fields_by_name 統合 ✅

- `resolve_struct_fields_by_name`: TypeRegistry → SyntheticTypeRegistry → type_param_constraints の3層フィールド解決を single source of truth として抽出
- `resolve_member_type`, `resolve_spread_source_fields`, `resolve_object_lit_fields` の3関数を共通メソッドに統合
- **実績: -3 エラー（OBJECT_LITERAL_NO_TYPE 36→33）、クリーン 108→110**

#### C-1: TypeResolver パラメータ型伝播 + patterns.rs todo!() 解消 ✅

- `visit_param_pat` が `Pat::Assign` を処理するよう拡張（デフォルト値パラメータの変数登録 + expected type 伝播）
- `visit_fn_decl`/`resolve_arrow_expr`/`resolve_fn_expr` のパラメータ型収集で `Pat::Assign` を処理
- `extract_type_ann_from_pat` ヘルパー: 任意の Pat バリアントから型注釈を抽出
- `patterns.rs` の `todo!()` 8 箇所を保守的フォールバック（`false`）に置換（Tier 2 パニック → Tier 3 保守的誤り）
- `convert_in_operator`/`convert_instanceof` で複雑式の実際の変換を試行（HashMap の contains_key、Option の is_some）
- **実績: -1 エラー（OBJECT_LITERAL_NO_TYPE 33→32）、生成コードの todo!() パニック 8 箇所解消**

#### C-2: expected type 基盤改善 ✅

- OBJECT_LITERAL_NO_TYPE（32件）の根本原因を体系的に調査し、以下の基盤バグ・欠陥を修正:
  1. **ClassProp 初期化子の順序バグ修正**: `resolve_expr` が `expected_types.insert` より先に実行されていた問題を修正（`visit_var_decl` と同じ正しい順序に）
  2. **`resolve_type_params_in_type` ヘルパー**: 型パラメータ名を制約型に再帰的に解決。Named, Option, Vec, Fn, Tuple の全 RustType バリアントに対応
  3. **全 expected type 挿入箇所への型パラメータ解決適用**: `visit_var_decl`, return 文, `Pat::Assign`, `ClassProp`, `propagate_fallback_expected`, `propagate_arg_expected_types` の 6 箇所
  4. **`resolve_object_lit_fields` への type_args 伝播**: ジェネリック構造体のフィールドを具象化して解決
  5. **PrivateMethod/PrivateProp の TypeResolver 処理追加**: TypeResolver が private メンバーの body を未訪問だったバグを修正。`visit_method_function`/`visit_class_prop_init` ヘルパーで Method/PrivateMethod, ClassProp/PrivateProp を DRY 化
  6. **コンストラクタのみクラスの TypeRegistry 登録条件修正**: `!fields.is_empty() || !methods.is_empty()` → `|| constructor.is_some()` 追加
  7. **空オブジェクト `{}` + HashMap expected type → `HashMap::new()` 生成**
  8. **型パラメータ制約マージ**: メソッド/アロー関数/関数式/関数宣言で `std::mem::replace` → マージに変更。ネストしたジェネリック関数で親スコープの制約を保持
- **実績: 0 エラー削減（数値不変）。理由: 32件全てにおいて、修正したパターン以外に「関数呼び出し引数」(I-300) や「型注釈なしオブジェクト」(I-301) が同一関数内に共存し、それらが残る限り関数単位のエラーカウントは減少しない。個別パターンの正常動作はユニットテスト・個別テストケースで確認済み**
- **発見された後続課題**: I-300（関数引数 expected type 伝播）、I-301（型注釈なしオブジェクト）、I-302（this.field = {} パターン）を TODO に記録

| 順序 | パターン | エラー削減 |
|------|---------|-----------|
| C-3 | 関数引数 expected type + 呼び出し側型引数推論（I-300 + I-286c S3） | ~14件 |
| C-4 | 型注釈なしオブジェクト + this.field パターン（I-301 + I-302） | ~5件 |

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
| Phase C-0 (resolve_member_type) | -3 | ✅ 66→63 |
| Phase C-1 (Pat::Assign + todo!()) | -1 + 品質 | ✅ 63→62 |
| Phase C-2 (expected type 基盤) | 0（基盤修正） | ✅ 62→62 バグ8件修正 + テスト16追加 |
| Phase C (残: C-3, C-4) | ~-19 | ~62→~43 |

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
