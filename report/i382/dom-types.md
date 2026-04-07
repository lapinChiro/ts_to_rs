# T0.3: DOM/Web API 型 16 件 調査結果

## 該当 dangling ref と参照文脈

| dangling name | 主 referencer (anonymous union 名) | TS 由来 |
|---|---|---|
| `HTMLCanvasElement` | `HTMLCanvasElementOrHTMLImageElementOr...VideoFrame` | `lib.dom.d.ts` (TS browser builtin) |
| `HTMLImageElement` | 同上 | 同上 |
| `HTMLVideoElement` | 同上 | 同上 |
| `SVGImageElement` | 同上 | 同上 |
| `ImageBitmap` | 同上 + `ArrayBufferOrAudioDataOrImageBitmap...` | 同上 |
| `VideoFrame` | 同上 + `ArrayBufferOrAudioDataOr...VideoFrame...` | 同上 |
| `AudioData` | `ArrayBufferOrAudioDataOr...` | 同上 |
| `BufferSource` | `BufferSourceOrString` | 同上 |
| `CanvasGradient` | `CanvasGradientOrCanvasPatternOrString` | 同上 |
| `CanvasPattern` | 同上 | 同上 |
| `MediaSourceHandle` | `ArrayBufferOrAudioDataOr...` | 同上 |
| `RTCDataChannel` | 同上 | 同上 |
| `ImageBitmapRenderingContext` | `ImageBitmapRenderingContextOr...WebGLRenderingContext` | 同上 |
| `WebGL2RenderingContext` | 同上 | 同上 |
| `WebGLRenderingContext` | 同上 | 同上 |
| `ServiceWorker` | `MessagePortOrServiceWorkerOrWindow` | 同上 |
| `Window` | 同上 | 同上 |
| `HeadersInit` | `HashMap...OrHeadersInitOrVecTupleStringString` | 同上 (`lib.dom.d.ts`) |
| `RequestInfo` | `RequestInfoOrURL` | 同上 |
| `TemplateStringsArray` | `CssClassNameOr...`, `StringOrTemplateStringsArray` | `lib.es5.d.ts` (TS lang builtin、tagged template) |

合計 16 件 (TemplateStringsArray は厳密には ES5 だが lib builtin として同分類)。

## 共通根本原因

これらの型は **TypeScript の lib.\*.d.ts で宣言される ambient builtin** で、Hono ソース内には宣言が存在しない。`TypeCollector` は declare されていない型を `TypeRegistry` に登録しないため、参照は dangling になる。

## 全 referencers が anonymous synthetic union である事実の意味

T0.1 と同じく、user 定義 struct/enum からの直接参照は 0 件。全て TypeResolver が生成した anonymous union (`<A>Or<B>` 命名) のフィールド型に焼き込まれている。これは `Response.body: BufferSource | string` のような lib.dom 型を含む union を Hono 自身の型定義が参照するためで、TypeResolver は anonymous union 化する際に内部要素を `RustType::Named` に変換するが、未知型 `BufferSource` をそのまま raw 化する。

## 修正方針案 (PRD 化候補)

### 案 i: lib.dom 型 registry 導入 + 明示的「外部型・unsupported」マーク

`TypeCollector` 起動時に lib.dom.d.ts / lib.es5.d.ts 由来の主要型を `TypeRegistry` に **`TypeDef::ExternalUnsupported`** (新 variant) として事前登録する。`generate_external_struct` がこの variant を識別したら `Item::Unsupported` をクライアントに伝播し、参照元 anonymous union 全体を「変換不可」としてエラー化する。

利点:
- silent stub と異なり明示エラー化、conversion-correctness-priority Tier 3 の正規化
- 将来 `web_sys` 対応を入れる際の単一 hook
- type-fallback-safety 完全準拠 (Safe = compile error)

欠点:
- 現状 stub で握り潰されていた union を error 化するため、Hono ベンチで「クリーン」が一時的に減る可能性
- ただし silent semantic change (Tier 1 リスク) を消すため、これは正の変化

### 案 ii: 各型を Rust 等価物にマップ

例:
- `BufferSource` → `Vec<u8>`
- `HeadersInit` → `HashMap<String, String>`
- `TemplateStringsArray` → `Vec<String>` (tagged template)
- `Window` / DOM canvas/WebGL/RTC/Worker 系 → 全て `serde_json::Value`

利点: 一部 Hono コードがコンパイル可能になる
欠点:
- type-fallback-safety で **UNSAFE** に該当する: 例えば `BufferSource | string` が `Vec<u8> | String` になり、本来 binary だった用途が文字列処理 path に silent に流れるリスク
- アドホックなマッピング表で、設計の理想性が低い

### 案 iii: 全 16 件を `serde_json::Value`

論外。type-fallback-safety で UNSAFE。

### 推奨: **案 i**

理由:
1. 「最も理想的でクリーン」: lib.dom 型は本質的に Rust transpilation 対象外であり、変換不可と明示するのが最も誠実
2. type-fallback-safety 完全準拠
3. silent semantic change を消す (Tier 1 解消)
4. 将来 `web_sys` 連携時の hook が単一箇所
5. アドホックなマッピング表より構造的

## PRD-β スコープ確定事項

- 対象: 上記 16 件
- 完了条件:
  - probe で 16 件 0 件
  - lib.dom registry が `TypeRegistry` に統合され、参照経路でエラー伝播が機能
  - Hono ベンチで対象型由来の error が「カテゴリ unsupported」として明示計上される
- 手段: 新 `TypeDef::ExternalUnsupported` variant + 事前登録 + Pass 5a/5c でのエラー伝播
- 影響範囲: `TypeRegistry`, `TypeCollector`, `external_struct_generator`, anonymous union 生成 path

## 残る検証 (PRD-β Discovery 時)

- lib.dom 型のリストをどう管理するか (静的リテラル / 外部 JSON / TypeScript stdlib をパース)
- 既存 Hono ベンチで「クリーン」減少幅の事前推定
- PRD-α と統合して 1 PRD にすべきか独立 PRD にすべきか (TypeResolver の anonymous synthetic 生成 path を共有するため、統合の合理性あり)

## PRD-α との関係

PRD-α (型パラメータ leak) と PRD-β (lib.dom) は、いずれも「TypeResolver の anonymous union 化で内部要素の型解決が不完全」という共通課題に由来するが、解決策が異なる:
- PRD-α: 型パラメータを generic synthetic として保持
- PRD-β: 未知外部型を unsupported error として伝播

両者は同じ TypeResolver 関数を改修するため、**同一 PRD で扱う方が DRY と整合性が高い** 可能性がある。T0.5 統合時に最終判定する。
