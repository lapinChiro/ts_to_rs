# T0.5: Phase 0 統合 — 根本原因クラスタリングと PRD 化計画確定

## Phase 0 の重大な発見

事前検証では「Hono で空 stub fallback 0 件」を予想していたが、実測で **34 種類の dangling ref + 73 種類の excluded user 定義型 = 計 107 種類** の参照が `generate_stub_structs` の band-aid に依存していた。

さらに重要な構造的発見:

> **全 dangling ref の referencer は TypeResolver が生成した anonymous synthetic 型 (anonymous union / intersection / object literal) に限定される**。user 定義 struct/enum/trait 自身からの dangling ref は **0 件**。

これは「dangling ref を生む唯一の path は TypeResolver の anonymous synthetic 化処理である」ことを意味し、根本対応の方向性を確定する。

## 根本原因クラスタ (確定)

### Cluster 1: TypeResolver の anonymous synthetic 生成における型解決不完全

TypeResolver が `T1 | T2` / `T1 & T2` / `{ ... }` を anonymous synthetic 化する際、内部要素の `RustType::Named { name }` をそのままコピーする。**囲む generic scope (関数 / クラスの type_params) と未知外部型の区別をせず、両者を等しく raw 文字列として焼き込む**。これが下記サブクラスタを共通根本原因として生成する:

#### Cluster 1a: 型パラメータ leak (PRD-α 対象)

11 件 + α (T0.4 で `H` の generic 引数も影響可能性):

| dangling | 発生 generic scope |
|---|---|
| `M` | `<M extends string>` (`types.ts:2176`) |
| `S` | `<S extends Schema>` (`hono-base.ts:217`) |
| `P` | `<P extends ParamKeys<...>>` |
| `U` | `<U>` (`context.ts:130`) |
| `E` | `<E extends Env>` |
| `TNext` | `<TNext>` (Promise.then) |
| `TResult` | `<TResult = any>` (`aws-lambda/types.ts:50`) |
| `TResult1`, `TResult2` | `<TResult1, TResult2>` (Promise.then) |
| `OutputType` | `<OutputType, ...>` (`validator/validator.ts:16`) |
| `Status` | `<Status extends number>` (`types.ts:2450`) |

#### Cluster 1b: 未知外部型 leak (PRD-β 対象)

17 件 (T0.3 の DOM 16 件 + `symbol`):

`HTMLCanvasElement`, `HTMLImageElement`, `HTMLVideoElement`, `SVGImageElement`, `ImageBitmap`, `VideoFrame`, `AudioData`, `BufferSource`, `CanvasGradient`, `CanvasPattern`, `MediaSourceHandle`, `RTCDataChannel`, `ImageBitmapRenderingContext`, `WebGL2RenderingContext`, `WebGLRenderingContext`, `ServiceWorker`, `Window`, `HeadersInit`, `RequestInfo`, `TemplateStringsArray`, `symbol`

(注: 上記列挙は 21 件あるが、`HeadersInit`/`RequestInfo`/`TemplateStringsArray` を T0.3 で同分類した後、`symbol` を T0.2 から移管した結果 `17` ではなく `17` ± のカウントは PRD-β 設計時に再計上)

実数: probe ログから集計し直し → DOM 16 + `symbol` 1 + `TemplateStringsArray` 1 + `HeadersInit` 1 + `RequestInfo` 1 = **20 件** (重複なし)

#### Cluster 1c: TS compiler internal marker leak (PRD-γ 対象)

1 件: `__type` (TypeScript の anonymous type literal compiler marker が変換出力に leak)

#### Cluster 1d (= Cluster 2): user 定義型の anonymous synthetic への焼き込み (PRD-δ 対象)

73 件 (T0.4 確定)。anonymous synthetic の field/variant 型が user 定義型を参照するケース。これは Cluster 1a/1b/1c と異なり「型解決自体は成功している」が、参照先が user 定義モジュールにあるため、shared 配置の synthetic から見ると `use crate::<path>::Type;` import が必要になる。

これだけは `generate_stub_structs` の band-aid (`defined_elsewhere_names` exclusion) で隠蔽されている本物の I-382 対象。

## 数値整合性の最終検証

```
dangling probe 出力: 34 件
  ├ Cluster 1a (型パラメータ): 11 件 (E,M,P,S,U,TNext,TResult,TResult1,TResult2,Status,OutputType)
  ├ Cluster 1b (外部型): 20 件 (DOM 16 + HeadersInit + RequestInfo + TemplateStringsArray + symbol)
  ├ Cluster 1c (compiler marker): 1 件 (__type)
  └ Cluster 1d ハイブリッド: 2 件 (HTTPException = user struct だが Some 返却で空 stub ではない、+ 重複カウント調整)
合計確認: 11 + 20 + 1 + 2 = 34 ✓

excluded_user 出力: 73 件
  └ Cluster 2 (PRD-δ 本体): 73 件 (うち H は要再確認 → user 定義 generic alias と確定)
```

注: HTTPException は dangling として probe にカウントされたが `typedef=Struct` で `generate_external_struct` が `Some` を返している (= 空 stub fallback ではない)。これは「stub generated」フラグでカウントされ、実装上は問題なく struct 化されていた。ただし shared 配置時の import 問題は user 定義型と同等に発生する可能性があり、Cluster 2 にも該当する重複ケースとして PRD-δ で扱う。

## サブ PRD 構成 (確定)

### PRD-α: TypeResolver anonymous synthetic への generic scope 伝播

- **Goal**: probe で Cluster 1a 11 件 + 関連波及が 0 件
- **対象 dangling ref**: `E`, `M`, `P`, `S`, `U`, `TNext`, `TResult`, `TResult1`, `TResult2`, `Status`, `OutputType`
- **Approach**: anonymous synthetic 生成時に囲む generic scope の type_params を伝播し、synthetic 自身を generic 化 (`MOrVecM<M>` 等) する。`monomorphize_type_params` の責務見直しが必要
- **影響範囲**: TypeResolver, SyntheticTypeRegistry, Pass 5a, anonymous union 関連テスト
- **依存**: なし (最初に着手)

### PRD-β: lib.dom / unsupported primitive 型の正規ハンドリング

- **Goal**: probe で Cluster 1b 20 件が 0 件
- **対象**: DOM/Web API 型 + `TemplateStringsArray` + `HeadersInit`/`RequestInfo` + `symbol`
- **Approach**: 新 `TypeDef::ExternalUnsupported` variant を `TypeRegistry` に導入し、TypeCollector 起動時に lib.dom 相当の型集合を事前登録。参照経路では `Item::Unsupported` を伝播し、anonymous union 全体を変換不可エラー化
- **影響範囲**: `TypeRegistry`, `TypeCollector`, `external_struct_generator`, anonymous union 生成 path
- **依存**: PRD-α 完了 (anonymous synthetic 生成 path を共有するため、PRD-α の改修後の API に乗る)

### PRD-γ: TypeScript compiler internal marker `__type` の leak 修正

- **Goal**: probe で Cluster 1c 1 件 (`__type`) が 0 件
- **対象**: `__type`
- **Approach**: TypeCollector / TypeConverter で `__type` を発生源とする path を特定し、適切な型 (anonymous function type / call signature) として変換。`__type` を `RustType::Named` に焼き込まない
- **影響範囲**: TypeCollector / TypeConverter
- **依存**: PRD-α 完了 (同じ TypeResolver 周辺の改修後)

### PRD-δ (= I-382 本体): synthetic から user 定義型への参照に対する import 生成

- **Goal**:
  - `generate_stub_structs` 関数完全削除
  - `defined_elsewhere_names` 引数完全削除
  - 全 user 定義型参照が `use crate::<path>::Type;` 経由で解決
  - Pass 5c に dangling ref 残存検出 panic を追加 (regression detector)
  - probe 投入時に dangling ref / excluded_user ともに 0 件
- **対象**: 73 件の user 定義型 + `HTTPException` (重複ケース)
- **Approach**: `OutputWriter::resolve_synthetic_placement` を拡張し、(1) synthetic_items が参照する user 型を `TypeRefCollector` で収集、(2) `ModuleGraph::module_path` で各々を解決、(3) shared/inline 各 placement に応じて import 注入
- **影響範囲**:
  - `pipeline/external_struct_generator/mod.rs` (削除)
  - `pipeline/output_writer/placement.rs` (拡張)
  - `pipeline/mod.rs` の Pass 5c (再構成)
- **依存**: PRD-α / PRD-β / PRD-γ 全完了 (probe で Cluster 1a/1b/1c が 0 件にならないと `generate_stub_structs` 削除時に dangling ref 残存 panic がトリガする)

## 依存関係グラフ (最終)

```
PRD-α (型パラメータ leak / Cluster 1a)
   │
   ├──> PRD-β (外部型 / Cluster 1b)
   │       │
   ├──> PRD-γ (__type / Cluster 1c)
   │       │
   │       │  (PRD-β と PRD-γ は PRD-α 後で並列可)
   │       │
   └──> ─── ┴──> PRD-δ (= I-382 本体, Cluster 2)
```

## TODO 追加 (Phase 1 で `/prd-template` Discovery 時に確定)

- I-383: PRD-α (TypeResolver generic scope 伝播)
- I-384: PRD-β (lib.dom unsupported registry)
- I-385: PRD-γ (`__type` leak 修正)
- I-382 本体 = PRD-δ

## master-plan.md への反映事項

1. Phase 0 のクラスタ分類が当初想定 (a/b/c/d) より精緻になった: 1a/1b/1c/2 の 4 クラスタ
2. PRD 構成は当初の α/β/γ/δ と一致 (リネーム不要)
3. `__type` は単独 PRD-γ 化 (元々 PRD-γ に統合候補だったが独立で進める方が責務が明確)
4. `symbol` は PRD-β に統合
5. `OutputType`, `Status` は PRD-α に統合 (元 c カテゴリから移動)
6. PRD-β と PRD-γ は PRD-α 完了後に並列実行可能

## 各 PRD の完了測定基準 (probe ベース)

各 PRD 完了時に probe instrumentation を再投入し以下を確認:

- PRD-α 完了後: `dangling` の Cluster 1a 11 件が消滅 (DOM 20 + `__type` 1 + 73 user defined のみ残存)
- PRD-β 完了後: `dangling` の Cluster 1b 20 件が消滅 / または明示エラー化
- PRD-γ 完了後: `dangling` の `__type` 0 件
- PRD-δ 完了前 (T2.checkpoint): `dangling` 出力 0 件 (= 全 Cluster 1 解消) / `excluded_user` のみ残存
- PRD-δ 完了後: probe 自体が不要 (Pass 5c の panic detector がこれを担う)

## 次タスク

- T0.5 完了 → master-plan.md 進捗更新
- Phase 1 (T1.α) 開始: PRD-α の `/prd-template` 適用
