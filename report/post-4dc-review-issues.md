# Batch 4d-C レビューで発見された残存課題

**Base commit**: `02b84da`（+ 未コミットの Batch 4d-C 変更）

## 概要

Batch 4d-C のレビューで、declaration 変換以外にも同一パターンの設計問題が複数箇所に残存していることを確認した。

1. `resolve_typedef` 内のエラー握りつぶし（`filter_map` + `.ok()`）
2. `type_aliases.rs` の SWC AST 直接操作（`convert_method_signature` / `convert_property_signature`）
3. `transformer/functions/params.rs` の `convert_property_signature` 使用

## 詳細分析

### 課題 1: `resolve_typedef` 内のエラー握りつぶし

`src/ts_type_info/resolve/typedef.rs` に 10 箇所のエラー握りつぶしが存在する。

| 行 | 関数 | パターン | 影響 |
|----|------|----------|------|
| L27 | `resolve_type_params` | `.and_then(\|c\| resolve_ts_type(...).ok())` | 型パラメータの制約が消失 |
| L53 | `resolve_typedef` (Struct) | `filter_map(\|f\| resolve_field_def(...).ok())` | Struct フィールドが消失 |
| L69 | `resolve_typedef` (Struct) | `filter_map(\|sig\| resolve_method_sig(...).ok())` | コンストラクタシグネチャが消失 |
| L74 | `resolve_typedef` (Struct) | `filter_map(\|sig\| resolve_method_sig(...).ok())` | コールシグネチャが消失 |
| L134 | `resolve_typedef` (Enum) | `filter_map(\|f\| resolve_field_def(...).ok())` | Enum variant フィールドが消失 |
| L157 | `resolve_typedef` (Function) | `filter_map(\|p\| resolve_param_def(...).ok())` | 関数パラメータが消失 |
| L179 | `resolve_typedef` (ConstValue) | `resolve_ts_type(...).ok()?` | Const フィールドが消失 |
| L190 | `resolve_typedef` (ConstValue) | `resolve_ts_type(...).ok()?` | Const 要素が消失 |
| L260 | `resolve_method_sig` | `filter_map(\|p\| resolve_param_def(...).ok())` | メソッドパラメータが消失 |

**影響**: registry フェーズで TypeDef を構築する際、型解決に失敗したフィールド/パラメータが消失する。結果として transformer は不完全な TypeDef を参照し、生成コードでフィールドやパラメータが欠落する。

**呼び出し元**: `src/registry/collection.rs:162-285` の `resolve_typedef` 呼び出し（9 箇所）。全て `if let Ok(resolved)` で囲まれているため、TypeDef 全体の失敗はハンドリングされるが、部分的な要素消失は検出されない。

**Batch 4d-C で修正済みの同一パターン**:
- `resolve_type_literal_fields` (intersection.rs:231): `filter_map` → `map` + `?` に修正済み
- `resolve_type_literal` (intersection.rs:209): 同上

### 課題 2: `type_aliases.rs` の SWC AST 直接操作

`src/pipeline/type_converter/type_aliases.rs` の `convert_type_alias_items` 内で、TsTypeLit の処理が SWC AST を直接操作している。

| 行 | 操作 | 対応する resolve 版 |
|----|------|---------------------|
| L249-252 | `convert_method_signature` (SWC `TsMethodSignature` → Method) | `resolve_method_info` |
| L267-269 | `convert_property_signature` (SWC `TsPropertySignature` → StructField) | `resolve_type_literal_fields` |
| L200-233 | Call signature 直接解析（SWC `TsCallSignatureDecl`） | なし（新設が必要） |
| L271-295 | Index signature 直接処理（SWC `TsIndexSignature`） | なし（`resolve_type_literal` が内部で処理） |

**影響**: Batch 4d-C で移行した union/intersection と同じ DRY 違反。type alias が `{ method(): T }` や `{ key: T; }` の形を持つ場合に SWC AST を直接走査する。

### 課題 3: `transformer/functions/params.rs` の `convert_property_signature`

`src/transformer/functions/params.rs:50` で、関数パラメータのインライン型リテラルを `convert_property_signature` で処理している。

```typescript
function foo({ a, b }: { a: string; b: number }) { ... }
```

この場合、`{ a: string; b: number }` の TsTypeLit を直接走査して StructField を生成する。

**影響**: declaration 変換と同じく、SWC AST 直接操作による DRY 違反。ただし transformer フェーズであり、type_converter フェーズとは異なるパイプライン段階。

### 課題間の依存関係

```
課題 1（resolve_typedef エラー伝播）
  → 独立。他の課題と並行対応可能。

課題 2（type_aliases.rs の TsTypeInfo 移行）
  → 課題 3 とは独立。
  → Batch 4d-C と同じ手法で対応可能。

課題 3（transformer/params.rs の移行）
  → 課題 2 とは独立。transformer フェーズのため別 PRD が望ましい。
```

## エラーハンドリング方針の調査結果

### `resolve_typedef` の `.ok()` によるフィールド消失の実証

テスト入力:
```typescript
interface WithKeyof { name: string; keys: keyof typeof console; value: number; }
function test(w: WithKeyof): string { return w.name + w.keys; }
```

出力:
```rust
struct WithKeyof { pub name: String, pub value: f64 }  // keys が消失
fn test(w: WithKeyof) -> String { w.name + &w.keys }   // w.keys はコンパイルエラー
```

`resolve_typedef` 内の `filter_map(|f| resolve_field_def(f, ...).ok())` が `keys` フィールドの型解決失敗（`keyof typeof console` → Err）を握りつぶし、フィールドが消失。transformer は TypeDef の情報を参照するため、`w.keys` へのアクセスは生成されるが、型定義にフィールドがないためコンパイルエラーになる。

### 方針 A（strict: 全 `.ok()` → `?`）が理想的

1. 部分的に正しい TypeDef は部分的に間違っている — downstream で不整合が生じる
2. 呼び出し元は既に `if let Ok(resolved)` でハンドリング済み — 安全に失敗する
3. TypeDef 全体が失敗した場合、downstream は「未知の型」として処理 — 一貫性がある
4. `resolve_ts_type` が Err を返すのは稀なケース — ベンチマーク影響は限定的

## 推奨対応

### 課題 1 + 課題 2 → 同一 PRD で対応

- 課題 1 は `resolve_typedef` 内のエラーハンドリング修正（`filter_map` → `map` + `?`）
- 課題 2 は `type_aliases.rs` の TsTypeInfo 移行
- どちらも resolve/typedef 周辺の修正であり、凝集度が高い

### 課題 3 → 別 PRD（課題 1+2 の直後に実施）

- transformer フェーズの修正は type_converter フェーズとは異なるパイプライン段階
- interface 変換の TsTypeInfo 移行と一緒に行うのが自然

## コード参照一覧

### 課題 1
- `src/ts_type_info/resolve/typedef.rs:27,53,69,74,134,157,179,190,260` — `.ok()` パターン
- `src/registry/collection.rs:162-285` — `resolve_typedef` 呼び出し元（9 箇所）
- `src/registry/collection.rs:513` — `resolve_field_def(...).ok()` 直接使用

### 課題 2
- `src/pipeline/type_converter/type_aliases.rs:200-295` — SWC AST 直接操作
- `src/pipeline/type_converter/type_aliases.rs:249-252` — `convert_method_signature` 使用
- `src/pipeline/type_converter/type_aliases.rs:267-269` — `convert_property_signature` 使用

### 課題 3
- `src/transformer/functions/params.rs:44-62` — `convert_property_signature` 使用
- `src/transformer/functions/mod.rs:15` — import
