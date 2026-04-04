# RC-8 残存: union 型の構造的同値性欠如 + push_item dedup バイパスの網羅的分析

## 概要

Batch 6 で intersection 型の構造的同値性を解決したが、同一根本原因（RC-8: synthetic 型の出現順ベース命名）が **union 型にも存在** する。さらに、`push_item` を直接使用して dedup メカニズムを完全にバイパスするパスが複数存在する。

## 発見経緯

Batch 6 のレビュー中に `push_item()` の全呼び出し元を網羅調査した結果、dedup バイパスが計 10 箇所存在することを確認。

---

## 問題 1（同一根本原因 RC-8）: `resolve_multi_member_union` の order-dependent 命名 + dedup 欠如

### 発生箇所

`src/ts_type_info/resolve/union.rs:84-104`

### 現状のコード

```rust
// AST 順でバリアント名を結合した enum 名を生成
let enum_name = name_parts.join("Or");
// ... variant 構築 ...
synthetic.push_item(enum_name.clone(), SyntheticTypeKind::UnionEnum, item);
```

### 問題の詳細

1. **命名が AST 出現順に依存**: `string | number` → `StringOrF64`、`number | string` → `F64OrString`。構造的に同一の型に異なる名前が付く
2. **dedup なし**: `push_item` を直接使用し、`register_union()` を呼んでいない

### 既存の dedup メカニズムとの関係

`SyntheticTypeRegistry::register_union()` は以下の dedup を実装済み:
- `union_signature()`: メンバー型の Debug 表現をソートしてシグネチャ計算（順序非依存）
- `generate_union_name()`: バリアント名をアルファベット順にソートして結合（`F64OrString` は常に同一）
- `union_dedup: HashMap<String, String>`: シグネチャ → 名前のキャッシュ

しかし `resolve_multi_member_union` はこのメカニズムを完全にバイパスしている。

### 影響

- 同一の union 型が異なるファイル/スコープで出現すると、異なる名前の重複 enum が生成される
- `register_union` が生成する名前（ソート済み）と `resolve_multi_member_union` が生成する名前（AST 順）が異なるため、同一型でも 2 つの enum が共存しうる

### 解決方向

`resolve_multi_member_union` 内の手動 enum 構築 + `push_item` を `synthetic.register_union(&resolved)` に置換。

**注意点**:
- `register_union` は名前をソートするため、既存のスナップショットや生成コードの enum 名が変わる（`StringOrF64` → `F64OrString` 等）
- `resolve_multi_member_union` には重複バリアント名スキップ（`:72-74`）と Promise/Result アンラップ（`:68`）の独自ロジックがある。`register_union` 側にこれらを吸収するか、前処理として残すか設計が必要

---

## 問題 2（異なる根本原因）: utility.rs の push_item dedup バイパス

### 発生箇所

`src/ts_type_info/resolve/utility.rs:48, 98, 147, 196`（4 箇所）

### 詳細

`Partial<T>`, `Required<T>`, `Pick<T, K>`, `Omit<T, K>` の synthetic struct 生成が `push_item` を直接使用。同一のユーティリティ型が複数箇所で出現すると重複生成される。

命名は `format!("Partial{inner_name}")` 等でソース型名ベースのため、同一入力なら同一名になり `push_item` の insert が上書きする。**実質的な害は低い**（上書きされるだけで重複定義にはならない）が、dedup 設計の一貫性に反する。

### 解決方向

`push_item` の前に `if !synthetic.get(&name).is_some()` チェックを追加、または `register_inline_struct` の名前指定版を追加。

---

## 問題 3（異なる根本原因）: register_any_enum の dedup 欠如

### 発生箇所

`src/pipeline/synthetic_registry.rs:113-141`（`register_any_enum` メソッド）

### 詳細

`any` 型パラメータの typeof/instanceof narrowing で生成される enum。命名は `format!("{}{}Type", function_name, param_name)` で、関数名 + パラメータ名ベース。同一関数の同一パラメータなら同一名になる。

`push_item` ではなく `self.types.insert()` を直接使用しているが、dedup キャッシュなし。上書きセマンティクスで実害は低い。

---

## 問題 4（異なる根本原因）: impl ブロックの命名不一致

### 発生箇所

- `src/ts_type_info/resolve/intersection.rs:152, 352`: `format!("{name}Impl")`
- `src/pipeline/type_converter/intersections.rs:409, 482`: `format!("{name}__impl")`

### 詳細

同一の intersection 型に対して 2 つの異なるコードパスが impl ブロックを生成する場合、命名規則が異なる（`Impl` vs `__impl`）。

- `resolve/intersection.rs`: アノテーション位置（パラメータ型、フィールド型）
- `type_converter/intersections.rs`: type alias 宣言（`type Foo = A & B`）

通常、同一型が両パスを通ることはないため実害は低いが、設計の一貫性に反する。

---

## 問題 5（異なる根本原因）: type_aliases.rs のスタブ trait push_item

### 発生箇所

`src/pipeline/type_converter/type_aliases.rs:401`

### 詳細

conditional type の `infer` パターンで生成されるスタブ trait。`push_item` で直接登録。命名はコンテナ名ベース（例: `"Promise"`）で、同一名なら上書き。実害最小。

---

## 分類と推奨対応

| # | 問題 | 根本原因 | 実害 | 推奨 |
|---|------|---------|------|------|
| 1 | union 型の order-dependent 命名 + dedup 欠如 | RC-8（同一） | **高**: 異なる名前の重複 enum | **Batch 6 スコープで対応** or 次バッチ |
| 2 | utility.rs の push_item | 別（命名はソース型ベース） | 低: insert 上書きで無害 | TODO 記録 |
| 3 | register_any_enum の dedup 欠如 | 別（命名は関数名ベース） | 低: insert 上書きで無害 | TODO 記録 |
| 4 | impl ブロック命名不一致 | 別（コードパス分離） | 低: パス交差なし | TODO 記録 |
| 5 | stub trait push_item | 別 | 最小 | TODO 記録 |

### 問題 1 の対応判断ポイント

- **Batch 6 に含める場合**: RC-8 の根本原因を完全に解決。スナップショット更新が追加で発生
- **次バッチに分離する場合**: Batch 6 のスコープを I-338 + I-318 に限定。union の dedup は別イシューとして追跡

`resolve_multi_member_union` の独自ロジック（重複バリアントスキップ、Promise アンラップ）を `register_union` と統合する設計が必要なため、単純な置換ではない。

---

## 完了条件（問題 1 を含める場合）

上記の Batch 6 完了条件に加えて:

6. `resolve_multi_member_union` が `register_union` 経由で dedup される
7. union enum の命名がソース順序に依存しない（`F64OrString` は常に `F64OrString`）
8. 重複バリアントスキップと Promise アンラップが `register_union` 呼び出し前の前処理として維持される
