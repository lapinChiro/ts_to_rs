# T0.1: 型パラメータ leak 調査結果

## 結論

11 件 (probe 計測ベース) の dangling ref が「TypeResolver が anonymous union/intersection を synthetic 化する際に、囲む generic scope の型パラメータを認識せず raw `RustType::Named { name: "<type-param>" }` として焼き込む」単一根本原因に集約される。

## 該当 dangling ref と発生 fixture

| dangling name | 発生 referencer (synthetic) | TS 発生源 | TS 行 (相対) |
|---|---|---|---|
| `M` | `MOrVecM` | `types.ts` | `methods: M \| M[]` (L2176, generic 関数 `<M extends string, ...>` 内) |
| `S` | `MergeSchemaPathOrS` | `hono-base.ts` | `Hono<E, MergeSchemaPath<...> \| S, ...>` (L217, generic class メソッド `<S extends Schema>`) |
| `P` | `AnyOrHashMap...HashMapPOptionString...` | (Hono client/router 系) | generic `<P extends ParamKeys<...>>` の `Record<P, ...>` 等 |
| `U` | `ResponseOrInitOrU`, `TupleContextWSEventsTUOrTupleFnWSEventsTU`, `UOrVecU` | `context.ts` 他 | `init?: ResponseOrInit<U>` (L130, generic `<U>` ) |
| `E` | `FnSSGParamsOrSSGParams`, `OmitServeStaticOptionsNamespace` | adapter/ssg | generic `<E extends Env>` |
| `TNext` | `TupleOrTupleTNext` | (Promise then 系) | generic `<TNext>` (Promise.then 標準) |
| `TResult` | `PromiseLikeTResultOrTResult`, `TOrTResult` | `aws-lambda/types.ts` L50-56 | `type Callback<TResult = any> = ...` / `Handler<TEvent, TResult>` |
| `TResult1` | `PromiseLikeTResult1OrTResult1`, `TResult1OrTResult2` | (Promise.then) | generic `<TResult1, TResult2>` |
| `TResult2` | `PromiseLikeTResult2OrTResult2`, `TResult1OrTResult2` | (Promise.then) | 同上 |
| `Status` | `_TypeLit4` | `types.ts` L2450 | `ExtractSchemaForStatusCode<T, Status extends number>` の `{ status: Status }` |
| `OutputType` | `OutputTypeOrTypedResponse` | `validator/validator.ts` L16-22 | generic `<OutputType, ...>` の `OutputType \| TypedResponse \| Promise<OutputType> \| ...` |

合計: **11 種類** (= probe (b) カテゴリ 9 件 + (c) Status + (c) OutputType)。

## 共通根本原因 (仮説)

TypeResolver は `T1 | T2` / `T1 & T2` / `{ ...obj }` を anonymous union/intersection/object literal として synthetic 化する際、内部要素の `RustType::Named { name }` をそのままコピーする。囲む関数/クラスの `type_params: Vec<TypeParam>` が **TypeResolver の anonymous 構築 context に伝播していない** ため、型パラメータと普通の型参照を区別できず、`M` / `S` / `Status` 等を「ユーザー定義型への参照」として synthetic struct/enum に焼き込んでしまう。

その結果、Pass 5a (`resolve_external_types_globally`) の `collect_undefined_type_references` で「外部型でない」と判定され除外されて生き残り、Pass 5c の `generate_stub_structs` で空 stub `pub struct M;` 等として生成される。これは silent semantic change の典型 (Tier 1) — 関数 scope の型パラメータ `M` と global struct `M` がコンパイル可能になり、後者が前者を shadow するか、最悪別の type param と name 衝突する。

## 修正方針案 (PRD 化候補)

### 案 A: TypeResolver に generic scope を伝播し、anonymous synthetic を generic 化

`type Foo<M> = M | M[]` を anonymous union 化する際、`MOrVecM` という monomorphic 名ではなく `MOrVecM<M>` のような generic synthetic として登録する。参照側は型引数を渡して具体化する。

利点: 完全に正確、monomorphize 不要、user の generic 構造を保持
欠点: TypeResolver / SyntheticTypeRegistry の API 拡張が必要、generic synthetic の重複検出ロジック追加

### 案 B: anonymous synthetic を呼び出し側で展開 (intern しない)

generic scope 内の anonymous union は synthetic 登録せず、各呼び出し点で `Either<M, Vec<M>>` 風に展開する。

利点: synthetic registry の責務縮小
欠点: 同一 union が複数箇所で重複展開、コード膨張

### 案 C: TypeResolver に type_param scope を渡し、synthetic 化対象から除外する型を判定

generic scope 内では union 化を抑制し、call site で構築する (案 B の弱版)。または、union 内の型パラメータ参照を検出したら synthetic 化を skip する。

利点: 最小変更
欠点: 部分対応で他バリエーション (intersection, object literal) に対応漏れリスク

### 推奨: **案 A** (PRD-α 設計セクションで Discovery 確定)

理由: 「最も理想的でクリーン」な解。`MOrVecM<M>` のように generic を保持することは TypeScript の意味論を完全に保存する唯一の方法。アドホック禁止原則と最も整合する。

## PRD-α スコープ確定事項

- 対象: 上記 11 件すべて
- 完了条件: 11 件が probe で 0 件
- 手段: TypeResolver の anonymous synthetic 生成 API に generic scope を伝播し、synthetic 自身を generic 化
- 影響範囲: TypeResolver, SyntheticTypeRegistry, Pass 5a (`generate_external_struct` の monomorphize ロジック), 既存の anonymous union 関連テスト

## 残る検証 (PRD-α Discovery 時)

- `PromiseLikeTResult1OrTResult1` のように Promise built-in と user generic が混ざるケースで案 A が機能するか
- `_TypeLit4` のような object literal type が型パラメータを含むケースの命名規約
- 既存の `monomorphize_type_params` (`src/ts_type_info/resolve/typedef.rs`) との責務分離
