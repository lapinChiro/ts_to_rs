# C-4 調査: TypeRegistry 型登録基盤（I-307 + I-305 + I-308）

**Base commit**: 0f4a3c3（uncommitted changes あり: C-3 未コミット）

## 概要

Phase C-4 の対象3イシューについて、影響範囲・設計方針・テストギャップを調査した。

## I-307: TypeAlias の TsTypeRef RHS 未登録

### 根本原因

`collect_type_alias_fields`（`src/registry/collection.rs:466`）が `TsTypeLit` と `TsIntersectionType` の2パターンのみ対応。`TsTypeRef`（`type X = Partial<Body>` 等）は `_ => None` で無視。

### 影響パス

```
collect_decl (L163) → try_collect_* → 全て None → collect_type_alias_fields → None
→ TypeDef::Struct { fields: [] } のまま（Pass 1 プレースホルダー）
```

### 設計方針

`collect_type_alias_fields` が `None` を返した場合に `convert_ts_type` にフォールバック。変換結果が `Named` なら registry/synthetic からフィールドを取得して `TypeDef::Struct` として登録。既存の `convert_ts_type` がユーティリティ型（`Partial`, `Required`, `Pick`, `Omit`, `Readonly`）を全て処理するため、ロジック重複なし。

## I-305: callable interface の return 型解決

### 根本原因

`collection.rs:130-161` が `interface` 宣言を**常に** `TypeDef::Struct` として登録。call signature のみの interface（`interface GetCookie { (c, key): Cookie; (c): Record<string, string> }`）でも例外なし。一方、type alias の `type H = { (c): number }` は `try_collect_call_signature_fn` で `TypeDef::Function` として登録される。

`resolve_fn_type_info` は `TypeDef::Function` のみ対応 → callable interface が `TypeDef::Struct` だと `(None, None)` を返す。

### 影響範囲

Hono に **14個** の callable-only interface が存在。`resolve_fn_type_info` の4つの呼び出し元全てが自動的に恩恵を受ける。

### 設計方針

registry 登録時に callable-only interface を `TypeDef::Function` として登録。type alias と interface で同じ分類ロジックを使う。

### DRY 違反の解消

`is_callable_only` 判定が registry（`functions.rs`）と converter（`interfaces.rs`）に重複。共通関数 `fn is_callable_only(members: &[TsTypeElement]) -> bool` を抽出し、`TsTypeLit.members` と `TsInterfaceBody.body` の両方で使用。

## I-308: indexed access 複合名 "E::Bindings" の未解決

### 根本原因

`convert_indexed_access_type`（`indexed_access.rs:55-58`）が型パラメータベースの indexed access を `Named("E::Bindings")` として出力。`resolve_type_params_in_type` はキー全体 `"E::Bindings"` で `type_param_constraints` を検索し、`"E"` にマッチしない。

### 設計方針

`resolve_type_params_in_type` で `"::"` を含む名前を検出した場合、ベース部分で制約を検索し、解決結果の型からフィールドを取得。将来的には IR を構造化するが、現時点では消費箇所が5+箇所あるため表現変更は別PRD規模。

## テストギャップまとめ

| エリア | 不足テスト | 重要度 |
|-------|-----------|-------|
| I-307 | `collect_type_alias_fields` に TsTypeRef RHS | 高 |
| I-307 | intersection 内 TsTypeLit & TsTypeLit | 中 |
| I-305 | interface 宣言 callable interface → TypeDef::Function | 高 |
| I-305 | resolve_fn_type_info + TypeDef::Struct (callable) | 高 |
| I-305 | callable interface expected type → arrow return type 伝播 | 高 |
| I-308 | resolve_type_params_in_type に "::" 複合名 | 高 |
| I-308 | 型パラメータベース indexed access の type_resolver テスト | 中 |
