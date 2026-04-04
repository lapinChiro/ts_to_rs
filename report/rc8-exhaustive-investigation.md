# RC-8 残存問題の網羅的調査レポート

**Base commit**: `5e89bc0`
**調査日**: 2026-04-05

---

## 調査目的

`backlog/rc8-remaining-structural-equivalence.md` の記述が正確かつ網羅的であるか検証する。Batch 6 の修正漏れ、および構造的同値性に関する未発見の問題を洗い出す。

---

## 調査結果サマリー

既存の backlog PRD で記述されている 5 問題に加え、**2 つの新規発見**があった。

| # | 問題 | 既存PRD記載 | 実害 |
|---|------|------------|------|
| 1 | union の order-dependent 命名 + dedup 欠如 | ✓ 記載済み | **高** |
| 2 | utility.rs の push_item dedup バイパス | ✓ 記載済み | 低 |
| 3 | register_any_enum の dedup 欠如 | ✓ 記載済み | 低 |
| 4 | impl ブロック命名不一致 (`Impl` vs `__impl`) | ✓ 記載済み | 低 |
| 5 | stub trait の push_item | ✓ 記載済み | 最小 |
| **6** | **variant_name_for_type の型引数消失による名前衝突** | **未記載** | **中** |
| **7** | **Pick/Omit の keys_suffix が順序依存** | **未記載（問題2に含まれるが詳細不足）** | **中** |

---

## 新規発見 6: variant_name_for_type の型引数消失

### 発生箇所

`src/pipeline/synthetic_registry.rs:455-478`

### 問題の詳細

`variant_name_for_type` は `Named { name, .. }` で **type_args をワイルドカードで破棄** している（`:465`）。

```rust
RustType::Named { name, .. } => match name.rsplit_once("::") {
    Some((_, last)) => last.to_string(),
    None => name.clone(),
},
```

結果として、以下のパターンで異なる型が同一の variant name を生成する:

| 型 | variant_name | 衝突 |
|----|-------------|------|
| `Named { name: "Foo", type_args: [String] }` | `"Foo"` | ✓ |
| `Named { name: "Foo", type_args: [f64] }` | `"Foo"` | ✓ |
| `Tuple(vec![String, F64])` | `"Tuple"` | ✓ |
| `Tuple(vec![Bool])` | `"Tuple"` | ✓ |
| `Result { ok, err: A }` | `"Result"` | ✓ |
| `Result { ok, err: B }` | `"Result"` | ✓ |
| `Fn { params: A }` | `"Fn"` | ✓ |
| `Fn { params: B }` | `"Fn"` | ✓ |

一方で `Vec` と `Option` は inner 型を再帰的に名前に含めるため衝突しない。

### 影響

`generate_union_name()` (`:445-449`) は `variant_name_for_type` ベースで enum 名を生成する。dedup の signature は `Debug` フォーマットで正確に区別するが、**生成される enum 名が衝突** する:

```
Union 1: [Named("Foo", [String]), F64] → signature: 異なる → name: "F64OrFoo"
Union 2: [Named("Foo", [i32]),    F64] → signature: 異なる → name: "F64OrFoo"  ← 同名!
```

`self.types` は `BTreeMap` なので後者が前者を **上書き** する。結果として Union 1 の定義が消失する。

### 発生確率の評価

同名の Generic 型が異なる型引数で union メンバーとして共存するケースは稀だが、`Result<T, E>` や `Promise<T>` では発生しうる。Hono ベンチマークでの実害は未検証。

### 解決方向

`variant_name_for_type` で `Named` の type_args を再帰的に名前に含める:
```rust
RustType::Named { name, type_args } => {
    let base = match name.rsplit_once("::") {
        Some((_, last)) => last,
        None => name.as_str(),
    };
    if type_args.is_empty() {
        base.to_string()
    } else {
        let args: Vec<String> = type_args.iter().map(variant_name_for_type).collect();
        format!("{base}{}", args.join(""))
    }
}
```

同様に `Tuple`, `Result`, `Fn` も内容を名前に反映すべき。

---

## 新規発見 7: Pick/Omit の keys_suffix が順序依存

### 発生箇所

`src/ts_type_info/resolve/utility.rs:140-145` (Pick), `:189-194` (Omit)

### 問題の詳細

既存 PRD の問題 2 は「utility.rs の push_item dedup バイパス」としてまとめているが、Pick/Omit には **命名の順序依存** という追加問題がある。

```rust
let keys_suffix: String = keys
    .iter()
    .map(|k| capitalize_first(k))
    .collect::<Vec<_>>()
    .join("");
```

`keys` は `extract_string_keys()` (`:312-318`) から返される `Vec<String>` で、Union メンバーの AST 順序に依存する:

- `Pick<User, "id" | "name">` → `PickUserIdName`
- `Pick<User, "name" | "id">` → `PickUserNameId` ← 異なる名前

### 影響

命名がソース型名ベースのため、**同一入力が常に同一 keys を返す限り** 問題は顕在化しない。しかし AST 順序が異なるファイルで同一の Pick 型が出現した場合、異なる名前の重複 struct が生成される。

### 解決方向

keys をソートしてから suffix を構築する:
```rust
let mut sorted_keys: Vec<_> = keys.iter().map(|k| capitalize_first(k)).collect();
sorted_keys.sort();
let keys_suffix: String = sorted_keys.join("");
```

---

## 既存PRD記載事項の検証結果

### 問題 1（union の order-dependent 命名）: 記述正確、補足あり

PRD の記述は正確。追加の知見:

- `resolve_multi_member_union` は `register_union` と **入力の抽象度が異なる**（`&[&TsTypeInfo]` vs `&[RustType]`）
- 単純な `register_union` への置換は不可。resolve → dedup の 2 段階が必要
- 重複バリアントスキップ（`:71-75`）は `register_union` にない機能。前処理として残す必要がある
- Promise/Result アンラップ（`:68`）も `register_union` にない。前処理として残す

### 問題 2（utility.rs）: 記述正確、Pick/Omit の順序依存追記が必要

Partial/Required は命名がソース型名ベースで実害低い。しかし Pick/Omit は keys_suffix に順序依存があり、新規発見 7 として分離して記載すべき。

### 問題 3（register_any_enum）: 記述正確

命名は関数名 + パラメータ名ベース。insert 上書きで実害なし。

### 問題 4（impl ブロック命名不一致）: 記述正確、補足あり

`resolve/intersection.rs` は `{name}Impl`、`type_converter/intersections.rs` は `{name}__impl`。

重要な補足: この 2 つのパスは **役割が異なる**:
- `resolve/intersection.rs`: 匿名 intersection（フィールド型、パラメータ型）→ synthetic 名（`_TypeLit0Impl`）
- `type_converter/intersections.rs`: type alias 宣言（`type Foo = A & B`）→ ユーザー定義名（`Foo__impl`）

通常パスが交差しないのはこの設計上の理由による。ただし命名規則の統一は設計一貫性の観点で推奨。

### 問題 5（stub trait push_item）: 記述正確、SyntheticTypeKind 不正タグ付け追記が必要

`type_aliases.rs:401` で Trait を `SyntheticTypeKind::UnionEnum` として登録している。`SyntheticTypeKind` に `Trait` バリアントが存在しない。ただし `kind` フィールドは現在コード生成で参照されておらず、**機能的影響なし**。

---

## Batch 6 修正の完全性検証

### 結論: type alias パスは dedup 不要（設計上正しい）

当初 `type_converter/intersections.rs` が dedup をバイパスしているように見えたが、詳細調査の結果:

- **このパスは type alias 宣言を処理** する（`type Foo = A & B`）
- 名前はユーザー定義名（`sanitize_rust_type_name(&decl.id.sym)` = `Foo`）を使用
- ユーザー定義名は **一意性が保証** されている（TypeScript の型名前空間で同名は存在しない）
- したがって synthetic な dedup メカニズムは不要

Batch 6 の修正は **匿名 intersection のパスでは正しく適用** されている:
- `resolve/intersection.rs:147` → `register_intersection_struct` ✓
- `resolve/intersection.rs:348` → `register_intersection_enum` ✓

### is_new 返り値の無視: 意図的設計

`intersection.rs:147` と `:348` で `_is_new` を無視しているのは、コメントで明示的に文書化された意図的な設計判断。dedup ヒット時でも impl ブロックの上書き登録が必要（先行登録がメソッドなしの可能性があるため）。

---

## dedup signature の正確性検証

| メソッド | signature 方式 | 型引数区別 | Optional 区別 | 順序非依存 |
|---------|---------------|-----------|--------------|-----------|
| `union_signature` | `Debug` フォーマット + sort | ✓ | ✓ | ✓ |
| `struct_signature` | `sanitize_field_name:Debug` + sort | ✓ | ✓ | ✓ |
| `intersection_enum_signature` | variant 構造 + sort | ✓ | ✓ | ✓ |

全 signature 関数は `Debug` フォーマットで型引数を含めた完全な文字列化を行い、sort で順序非依存を実現。**signature レベルでの衝突リスクはなし**。

問題は signature と **生成名** の乖離（新規発見 6）。

---

## PRD への反映推奨事項

1. **新規発見 6（variant_name_for_type 型引数消失）を追加**
   - 問題 1（union dedup）と同時に修正可能。union enum 名の正確性に直結
2. **新規発見 7（Pick/Omit keys_suffix 順序依存）を問題 2 から分離して追記**
   - 修正は 1 行のソート追加。問題 2 の他の utility とは性質が異なる
3. **問題 5 に SyntheticTypeKind 不正タグ付けを追記**
4. **Batch 6 の type_converter パスについて「dedup 不要の根拠」を明記**（誤解防止）

---

## カウンタベース命名の順序依存性について

`_TypeLit{N}`, `_Intersection{N}` のカウンタベース名は、dedup ヒット時は最初の登録名を返すため同一構造は同一名になる。しかし **初回登録時のカウンタ値はファイル処理順に依存** する。

これは構造的同値性の問題ではなく、**名前の安定性** の問題:
- 同じプロジェクトの別ビルドで異なる名前が生成される可能性
- 現時点で実害はない（生成コードの動作は正しい）
- ただし将来的に incremental compilation やキャッシュを導入する場合は問題になる

PRD のスコープ外として TODO に記録を推奨。
