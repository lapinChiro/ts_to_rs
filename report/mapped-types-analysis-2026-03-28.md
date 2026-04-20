# Mapped Types Analysis: 残存パターンと変換戦略

**初回作成**: 2026-03-28  
**最終更新**: 2026-04-05（レポート最適化）  
**Status**: Identity 簡約実装済み。P2-P5 は HashMap フォールバックのまま

---

## 現状

Mapped type (`{ [K in keyof T]: V }`) の変換は `src/ts_type_info/resolve/mod.rs:269-302` の `resolve_mapped` 関数で行われる。

- **Identity 検出**: `{ [K in keyof T]: T[K] }` → `T` に簡約（symbol filter noop 対応済み）
- **非 identity**: `HashMap<String, V>` にフォールバック

---

## 未解決パターン（Hono 実例）

### P2: Symbol Key Filtering（条件付きキーリマッピング）

```typescript
type OmitSymbolKeys<T> = { [K in keyof T as K extends symbol ? never : K]: T[K] }
```

`as` 句でキーをフィルタリング。現在 `HashMap<String, T[K]>` に変換される。正しくはソース型のフィールドを走査し、symbol キーを除外した struct を生成すべき。

### P3: 複雑なキーリマッピング（テンプレートリテラル）

```typescript
param: { [K in keyof ExtractParams<SubPath> as K extends `${infer Prefix}{${infer _}}` ? Prefix : K]: string }
```

Hono のルートパラメータ抽出。キー名をテンプレートリテラルで変換。`HashMap<String, string>` ではパラメータ名のマッピングが失われる。

### P4: 条件付き値型変換

```typescript
type InferInputInner<Output, Target, T> = SimplifyDeep<{
  [K in keyof Output]: IsLiteralUnion<Output[K], string> extends true
    ? Output[K]
    : Target extends 'form' ? T | T[] : ...
}>
```

キーごとに異なる条件で値型を変換。`HashMap<String, V>` では全プロパティが同一型に。

### P5: 配列インデックスの mapped type

```typescript
type JSONParsed<T extends ReadonlyArray<unknown>> = { [K in keyof T]: JSONParsed<T[K]> }
```

配列要素を再帰的に変換。`HashMap<String, V>` ではタプル構造が失われる。

---

## 変換戦略

### 推奨: Synthetic Struct 生成

ソース型 `T` が解決可能な場合、フィールドを走査して合成 struct を生成する。

1. `T` を `TypeDef::Struct` に解決
2. 全フィールドを抽出
3. `as` 句によるキーフィルタリング適用
4. 各フィールドの値型を mapped type の式に従って変換
5. `SyntheticTypeRegistry` に登録

ソース型が解決不可能な場合のみ `HashMap` にフォールバック。

### HashMap が正しいケース

`{ [key: string]: T }` (index signature) は `HashMap<String, T>` が正確な表現。これは `resolve/intersection.rs:179-183` で処理済み。

---

## 関連

- コード: `src/ts_type_info/resolve/mod.rs:269-302` (`resolve_mapped`)
- identity 簡約: `src/ts_type_info/resolve/mod.rs:330+` (`try_simplify_identity_mapped`)
- intersection 内: `src/pipeline/type_converter/intersections.rs:342` (`try_simplify_identity_mapped_type`)
- Hono ソース: `utils/types.ts`, `types.ts`, `client/types.ts`, `validator/utils.ts`
