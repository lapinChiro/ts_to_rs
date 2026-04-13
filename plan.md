# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-13)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 114/158 (72.2%) |
| Hono bench errors | 58 |
| cargo test (lib) | 2393 pass |
| cargo test (integration) | 99 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 89 pass |
| clippy | 0 warnings |
| fmt | 0 diffs |

---

## 設計判断の引継ぎ (後続 PRD 向け)

### `push_type_param_scope` は correct design であり interim ではない

PRD 起票時は `push_type_param_scope` を完全削除する想定だったが、実装調査で方針変更:

- `convert_external_type` (外部 JSON ローダ) と `convert_ts_type` (SWC AST コンバータ) は
  独立した 2 つの変換経路。`convert_ts_type` の TypeVar routing を後者が直接流用できない
- `convert_external_type::Named` も scope を参照して TypeVar routing する必要があり、
  scope 自体は「lexical scope management」として残すのが構造的に正しい
- 「interim」だったのは scope を介してフィルタ判定していた `extract_used_type_params` の
  heuristic 部分であり、それは walker-only 実装 (`collect_type_vars`) で完全置換済

**引継ぎ**: scope push を見て「interim 残存では?」と思った場合、上記の判断に立ち戻ること。

### `PrimitiveType` 9 variant の YAGNI 例外

`src/ir/expr.rs::PrimitiveType` は 9 variant 定義で、production で使われるのは `F64` のみ
(`f64::NAN` / `f64::INFINITY`)。「9 variant 維持」を採用した理由: (1) 基盤型としての概念的完全性、
(2) 将来 `i32::MAX` 等で再追加する総コストが現状維持より高い、(3) variant 網羅テストで
dead_code lint 発火しない。

**引継ぎ**: 後続 PRD で primitive associated const を使う際、既存 variant をそのまま利用すべき。

### `switch.rs::is_literal_match_pattern` の意味論微変化

判定基準を `name.contains("::")` 文字列マッチから `Expr::EnumVariant` 構造マッチに変更。
`case Math.PI:` / `case f64::NAN:` のような (TS で稀な) ケースは guarded match に展開される。
Hono 後退ゼロ確認済。

**引継ぎ**: 将来 `case` で primitive const / std const を使う TS fixture を追加する場合、
`is_literal_match_pattern` に `Expr::PrimitiveAssocConst { .. } | Expr::StdConst(_) => true`
追加を検討。ただし `f64` 値の pattern matching は Rust で unstable のため guarded match が安全。

### lock-in テスト (削除禁止)

`tests/enum_value_path_test.rs` / `tests/math_const_test.rs` / `tests/nan_infinity_test.rs`
は `Expr::EnumVariant` / `PrimitiveAssocConst` / `StdConst` 構造化の lock-in テスト。
**削除・スキップ禁止**。

### 残存 broken window

- **`Item::StructInit::name: String`** に display-formatted `"Enum::Variant"` 形式が格納される
  (`transformer/expressions/data_literals.rs:90`)。`StructInit` IR に
  `enum_ty: Option<UserTypeRef>` を追加して構造化すべき（TODO I-074）。

### union return wrapping の実行順序 (RC-13 PRD で確立)

`convert_fn_decl` 内の処理順序は以下でなければならない:

1. **Union return wrapping** — return/tail 式を enum variant constructor でラップ
2. **has_throw wrapping** — return 式を `Ok()` でラップし、return_type を `Result` に変更
3. **`convert_last_return_to_tail`** — 最後の return を tail 式に変換

理由: (1) `wrap_returns_in_ok` は `Stmt::TailExpr` を処理しないため 3 の後に実行不可。
(2) has_throw が return_type を `Result<T, String>` に変更すると union 型 `T` が隠蔽され
union wrap 判定が失敗するため 2 の前に実行必須。(3) throw 由来の `Err(...)` return は
SWC leaf collection に対応がないため `wrap_body_returns` でスキップする。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。
skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済み:**
- Step 0: `basic-types` unskip
- Step 1 (RC-13): `union-fallback`, `ternary`, `ternary-union` unskip + `external-type-struct` with-builtins unskip

**永続 skip (2件):** `callable-interface-generic-arity-mismatch` (意図的 error-case), `indexed-access-type` (マルチファイル用、別テストでカバー)

**残: 15 fixture / 15 イシュー**

#### 次の Step

```
Step 2 (Tier 1: RC-2 iterator) ←── 次はここ
  ↓
Step 3 (Box::new + Option)         Step 6 (string + intersection)
  ↓                                  type-narrowing は Step 1 + 6 で完全解消
Step 4 (control flow + DU)
  ↓
Step 5 (type conversion + null)
  ↓
Step 7 (builtin impl)
```

---

**Step 2: RC-2 iterator メソッドの所有権** — Tier 1、methods.rs

`src/transformer/expressions/methods.rs:35-63` に集中。I-011 と I-012 は同一関数。

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-011 | `build_iter_method_call()` (`methods.rs:35`) | `.iter().cloned()` 後のクロージャ引数型を `&T` → `T` に整合 |
| I-012 | 同上 + `return_wrap.rs` | `find()` が `Option<T>` を返す文脈での `Some()` 二重ラップ防止 |

- unskip: `array-builtin-methods`
- 部分解消: `closures`（I-020 残）

---

**Step 3: クロージャ Box 化 + Option 暗黙返却** — Tier 2、レバレッジ最大（4 fixture）

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-020 | `needs_trait_box_coercion()` (`expressions/mod.rs:288`) | クロージャ式 → `Box<dyn Fn>` の `Box::new()` ラップ追加 |
| I-025 | control_flow 周辺 | `Option<T>` 戻り値の if 文に暗黙 `else { None }` を補完 |
| I-024 | `try_generate_narrowing_match()` (`control_flow.rs:250`) | `if (x)` where `x: Option<T>` → `if let Some(x) = x` の truthy パス対応 |

- unskip: `closures`（Step 2 と合わせて完全解消）, `keyword-types`, `void-type`
- 部分解消: `functions`（I-024 解消で残エラー消滅を要確認）

---

**Step 4: 制御フロー + DU** — Tier 2、独立した 2 修正

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-023 | `convert_try_stmt()` (`error_handling.rs:96-138`) | try/catch 両方に return がある場合の unreachable code 除去 |
| I-021 | `is_du_field_binding()` (`type_resolution.rs:209`) | match body でデストラクチャ変数を使うべき箇所が `event.x` のまま |

- unskip: `async-await`, `discriminated-union`
- `functions` 完全解消（Step 3 と合わせて）

---

**Step 5: 型変換 + null セマンティクス** — Tier 2、型変換パイプライン

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-022 | `binary.rs:45-60` | ネスト `??` の中間結果型を `Option<T>` → `T` に unwrap |
| I-026 | 型 assertion 変換 | `as unknown as T` の中間 `unknown` を消去して直接キャスト |
| I-029 | null/any 変換 | `null as any` → `None` が `Box<dyn Trait>` 文脈で型不一致 |
| I-030 | `build_any_enum_variants()` (`any_narrowing.rs:85`) | any-narrowing enum の値代入で型強制 |

- unskip: `nullish-coalescing`, `type-assertion`, `trait-coercion`, `any-type-narrowing`

---

**Step 6: string メソッド + intersection** — Tier 2、独立した小修正群

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-033 | `methods.rs` | `charAt` → `chars().nth()`, `repeat` → `.repeat()` マッピング追加 |
| I-034 | `methods.rs` | `toFixed(n)` → `format!("{:.N}", v)` 変換 |
| I-028 | `intersections.rs:132-145` | mapped type の非 identity 値型で型パラメータ T が消失 (E0091) |

- unskip: `string-methods`, `intersection-empty-object`
- `type-narrowing` 完全解消（Step 1 の I-007 と合わせて）

---

**Step 7: ビルトイン型 impl 生成** — Tier 2、大規模

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-071 | `external_struct_generator/` + generator | ビルトイン型（Date, RegExp 等）の impl ブロック生成 |

- unskip: `instanceof-builtin`（`String()` コンストラクタ呼び出し問題が別途残る可能性あり）
- `external-type-struct` の no-builtin skip はテスト設計上の制約（with-builtin は Step 1 で解消済み）

---

#### fixture × Step 解消マトリクス

| fixture | 解消 Step | 依存 |
|---------|-----------|------|
| ~~basic-types~~ | ~~Step 0~~ | — |
| ~~union-fallback~~ | ~~Step 1~~ | — |
| ~~ternary~~ | ~~Step 1~~ | — |
| ~~ternary-union~~ | ~~Step 1~~ | — |
| ~~external-type-struct (with-builtins)~~ | ~~Step 1~~ | — |
| array-builtin-methods | **Step 2** | — |
| closures | Step 3 | Step 2 (I-011) |
| keyword-types | Step 3 | — |
| void-type | Step 3 | — |
| functions | Step 4 | Step 3 (I-020, I-024) |
| async-await | Step 4 | — |
| discriminated-union | Step 4 | — |
| nullish-coalescing | Step 5 | — |
| type-assertion | Step 5 | — |
| trait-coercion | Step 5 | — |
| any-type-narrowing | Step 5 | — |
| string-methods | Step 6 | — |
| intersection-empty-object | Step 6 | — |
| type-narrowing | Step 6 | Step 1 (I-007) |
| instanceof-builtin | Step 7 | — |

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE 27件)

Phase A 完了後、Hono ベンチマーク最大カテゴリ（全エラーの 47%）に着手。
I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。

---

## リファレンス

- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
- TODO 記載標準: `.claude/rules/todo-entry-standards.md`
- TODO 全体: `TODO`
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
