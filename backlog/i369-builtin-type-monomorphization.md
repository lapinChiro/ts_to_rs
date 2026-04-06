# I-369: ビルトイン外部型のモノモーフィゼーション未適用

## 背景

Batch 10 で型パラメータ制約のモノモーフィゼーションを実装した。ユーザー定義型では正しく動作するが、ビルトイン型（`web_api.json`, `ecmascript.json` 由来）から生成される外部型 struct にはモノモーフィゼーションが適用されない。

具体例:
```rust
// 期待: ArrayBufferView は制約 ArrayBuffer|SharedArrayBuffer でモノモーフィゼーション → 型パラメータ除去
pub struct ArrayBufferView {
    pub buffer: ArrayBufferOrSharedArrayBuffer,
}

// 実際: 型パラメータと非 trait 制約がそのまま残る → E0404
pub struct ArrayBufferView<TArrayBuffer: ArrayBufferOrSharedArrayBuffer> {
    pub buffer: TArrayBuffer,
}
```

## 現象

Hono types.rs で以下のエラーが発生（I-368 解消後に顕在化する）:

| エラー | 件数 | 対象型 | 原因 |
|--------|------|--------|------|
| E0404 | 2+ | `ArrayBufferView`, `Uint8Array` | 非 trait 型（enum）を trait bound に使用 |
| E0107 | 1+ | `ArrayBufferView(ArrayBufferView)` | モノモーフィゼーション未適用型への型引数省略 |

## 根本原因

2 つの独立した原因が組み合わさっている。

### 原因 1: `generate_external_struct` がモノモーフィゼーション未適用

`external_struct_generator/mod.rs:190`:
```rust
type_params: type_params.clone(),
```

TypeDef から `type_params` をそのままコピーしており、`monomorphize_type_params` を適用していない。ユーザー定義型は `extract_type_params`（`type_converter/utilities.rs:29`）経由でモノモーフィゼーションが適用されるが、外部型生成パスにはこのロジックがない。

### 原因 2: `base_synthetic` の transformer 未伝播

モノモーフィゼーション判定で `is_valid_trait_bound` が SyntheticTypeRegistry を参照するが、ビルトイン合成型（`ArrayBufferOrSharedArrayBuffer` 等）は `base_synthetic` にのみ存在する。

パイプラインのデータフロー:
```
load_builtin_types() → (builtin_reg, base_synthetic)
                                          ↓
                    synthetic = base_synthetic  (pipeline/mod.rs:75)
                                          ↓
              file_resolver_synthetic = synthetic.fork_dedup_state()  ← types は空!
                                          ↓
              file_synthetic = any_synthetic + resolver_synthetic     ← base_synthetic の types なし
                                          ↓
              Transformer(&tctx, &mut file_synthetic)                ← is_valid_trait_bound がビルトイン合成型を発見不可
```

`fork_dedup_state()`（`synthetic_registry/mod.rs:414-424`）は dedup カウンタのみコピーし、`types: BTreeMap::new()` で空にする。結果、`is_valid_trait_bound` がビルトイン合成型名（`ArrayBufferOrSharedArrayBuffer` 等）を TypeRegistry にも SyntheticTypeRegistry にも発見できず、`true`（外部 trait 仮定）を返してモノモーフィゼーションが非適用になる。

ただし、`generate_external_struct` はパイプラインの後段（`pipeline/mod.rs:153`, `200-203`）で呼ばれ、transformer フェーズとは別のコンテキストで実行される。原因 2 の伝播修正は `generate_external_struct` にモノモーフィゼーションを追加する際に必要になる。

## 意味論的安全性分析

モノモーフィゼーション（型パラメータ除去 + 制約型での置換）は Batch 10 で設計・検証済み。本修正はその適用対象をビルトイン外部型に拡張するのみ。

- **フィールド型置換** (`buffer: TArrayBuffer` → `buffer: ArrayBufferOrSharedArrayBuffer`): 型パラメータが制約型に置換。TS では `TArrayBuffer extends ArrayBufferOrSharedArrayBuffer` なので `ArrayBufferOrSharedArrayBuffer` のサブタイプのみが代入可能。Rust では union enum のため、コンパイル時に不正な型が検出される → **Safe**
- **型パラメータ除去**: 生成 struct からジェネリクスが消えるため、参照側も型引数不要になる。E0107 が自動解消 → **Safe**

## 設計

### Phase 1: `generate_external_struct` にモノモーフィゼーション適用

**変更箇所**: `external_struct_generator/mod.rs:159-196`

```rust
pub fn generate_external_struct(
    name: &str,
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,  // ← 追加
) -> Option<Item> {
    let typedef = registry.get(name)?;
    match typedef {
        TypeDef::Struct { type_params, fields, .. } => {
            // モノモーフィゼーション適用
            let (mono_params, mono_subs) =
                monomorphize_type_params(type_params.clone(), registry, synthetic);

            let struct_fields = fields.iter().map(|field| {
                let ty = field.ty.substitute(&mono_subs);  // 置換適用
                // ... (既存の Box ラップ等)
                StructField { vis: Some(Visibility::Public), name: ..., ty }
            }).collect();

            Some(Item::Struct {
                vis: Visibility::Public,
                name: name.to_string(),
                type_params: mono_params,  // モノモーフィゼーション後
                fields: struct_fields,
            })
        }
        // ...
    }
}
```

### Phase 2: 呼び出し元に `SyntheticTypeRegistry` を伝播

`generate_external_struct` の全呼び出し元にシグネチャ変更を反映:

1. **`generate_external_structs_to_fixpoint`** (`pipeline/mod.rs:231`): `&SyntheticTypeRegistry` パラメータ追加
2. **`generate_stub_structs`** (`external_struct_generator/mod.rs:132`): 同上（内部で `generate_external_struct` を呼ぶ）
3. **パイプライン呼び出し元** (`pipeline/mod.rs:153`, `172`, `203`):
   - ファイルループ内（line 153）: `&file_synthetic` を渡す → ビルトイン合成型が不足
   - **修正**: `base_synthetic` の types を `file_synthetic` に伝播するか、`base_synthetic` 自体を別途渡す

### Phase 3: `base_synthetic` の伝播

選択肢 A: `fork_dedup_state` を拡張して types もコピーする
- **却下**: 全ファイルに全ビルトイン合成型がコピーされ、メモリ消費増大。不要な合成型がファイル出力に混入するリスク

選択肢 B: `generate_external_struct` 専用に `base_synthetic` を渡す
- **採用**: `generate_external_structs_to_fixpoint` と `generate_stub_structs` に `base_synthetic` を追加パラメータで渡す
- `is_valid_trait_bound` は両方のレジストリを参照する必要があるため、呼び出し時に `base_synthetic` と `file_synthetic` をマージした一時レジストリを作成するか、`is_valid_trait_bound` に両方渡す

選択肢 C（最小変更）: `generate_external_struct` 呼び出し時のみ `base_synthetic` を渡す
- **採用**: パイプライン後段（`pipeline/mod.rs:200-206`）の共有 synthetic items 生成では `synthetic`（= base + 全ファイル分マージ済み）が利用可能。ファイルループ内（line 153）では `synthetic` はまだファイル分しかマージされていないが、外部型生成に必要なのはビルトイン合成型のみなので `base_synthetic` の read-only 参照で十分

**最終設計**: 選択肢 C を採用。`generate_external_structs_to_fixpoint` と `generate_stub_structs` に `&SyntheticTypeRegistry`（base + file をマージした read-only ビュー）を渡す。

### 設計レビュー

- **凝集度**: `generate_external_struct` の責務（TypeDef → Item 変換）に「モノモーフィゼーション適用」を追加。この責務は同関数に閉じた判断であり、凝集度を損なわない
- **責務分離**: モノモーフィゼーションロジック自体は `typedef.rs:345` の既存関数を呼ぶのみ。新たな走査・判定ロジックは追加しない
- **DRY**: `monomorphize_type_params` の呼び出しが `extract_type_params`（type_converter パス）と `generate_external_struct`（外部型パス）の 2 箇所になるが、これは異なるパイプラインステージでの適用であり、知識の重複ではない

## タスク

### Phase 1: `generate_external_struct` のモノモーフィゼーション
1. `generate_external_struct` に `&SyntheticTypeRegistry` パラメータ追加
2. `monomorphize_type_params` を呼び出し、`type_params` と `fields` に置換適用
3. 既存テスト修正（`tests.rs:598` 等のシグネチャ変更）
4. モノモーフィゼーションのテスト追加（制約付き型パラメータが除去されることを検証）

### Phase 2: 呼び出し元のシグネチャ変更
5. `generate_external_structs_to_fixpoint` のシグネチャ変更
6. `generate_stub_structs` のシグネチャ変更
7. `collect_undefined_type_references` のシグネチャ変更（必要な場合）
8. パイプライン呼び出し元（`pipeline/mod.rs:153`, `172`, `203`）の修正

### Phase 3: `base_synthetic` の伝播
9. ファイルループ内の `generate_external_structs_to_fixpoint` 呼び出しで `base_synthetic` + `file_synthetic` のマージビューを渡す
10. 共有 synthetic items 生成（`pipeline/mod.rs:200-206`）で `synthetic`（既にマージ済み）を渡す

### 検証
11. Hono ベンチマーク実行、`ArrayBufferView` / `Uint8Array` の E0404 解消確認
12. E0107 の自動解消確認

## 完了条件

1. ビルトイン外部型（`ArrayBufferView`, `Uint8Array` 等）の非 trait 制約がモノモーフィゼーションされる
2. Hono types.rs の E0404（ビルトイン型の非 trait bound）が全て解消
3. Hono types.rs の E0107（モノモーフィゼーション未適用による型引数不一致）が解消
4. 既存テスト全 pass
5. モノモーフィゼーション適用のテスト追加（ビルトイン型の制約除去を検証）

## スコープ外

- **E0405**（`Input::out` indexed access 解決失敗）: `lookup_field_type` の indexed access 解決の問題であり、モノモーフィゼーションとは別系統。TODO I-367 の E0405 記述として残す
- **ユーザー定義型の前方参照**: TypeRegistry の 2-pass collection で既に解決済み。調査で検証済み

## 関連

- Batch 10 レポート: `report/batch10-rc15-type-param-context.md`
- `monomorphize_type_params`: `src/ts_type_info/resolve/typedef.rs:345`
- `is_valid_trait_bound`: `src/ts_type_info/resolve/typedef.rs:294`
- `generate_external_struct`: `src/pipeline/external_struct_generator/mod.rs:159`
- `generate_external_structs_to_fixpoint`: `src/pipeline/mod.rs:231`
- `fork_dedup_state`: `src/pipeline/synthetic_registry/mod.rs:414`
- `load_builtin_types`: `src/external_types/mod.rs:166`
- I-368（OutputWriter 衝突修正）: 本 PRD の前提条件（I-368 が未修正だと types.rs の内容が上書きされエラー確認不可）
