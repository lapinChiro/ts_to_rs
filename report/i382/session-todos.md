# I-382 関連 セッション内発見 TODO

I-382 の作業中に発見した、本タスクのスコープ外だが将来的に対応すべき課題を記録する。
セッション開始時と完了時に振り返り、対応の優先順位を検討する。

## 運用ルール

- 新たに発見した課題は本ファイルに追記する (file:line + 発見経緯 + 推奨対応)
- 対応完了したら本ファイルから削除し、master-plan.md または完了 commit に経緯を残す
- PRD 化が妥当な規模に育ったら `backlog/` に切り出す

---

## 未対応 TODO

### T-1: 抽出器の intersection 型 fallback が `any` 退避

**発見**: 2026-04-08 (T2.A-i 実装中)

**箇所**: `tools/extract-types/src/extractor.ts::convertType` line 367-374

**現状**: TypeScript の intersection 型 (`A & B`) を構造化できないため、安全
fallback として `{ kind: "any" }` を返している。

```ts
// I-383 T2.A-i: 旧実装は `{ kind: "named", name: typeStr }` で raw 文字列を返していたが、
// これは Rust loader 側で `ArrayBuffer & { BYTES_PER_ELEMENT?: never; }` のような
// 不正な named 型として外部参照に leak する silent semantic defect。intersection を
// ExternalType に構造化するのは ExternalType schema 拡張 + Rust loader での struct
// merge 実装が必要で T2.A-i のスコープ外なので、ここでは安全な `any` fallback に
// 退避する (型推論の constraint 弱化に留まり、後段の構造を破壊しない)。
```

**発見経緯**: T2.A-i で signature-level type_params 抽出を追加したことで、TypedArray
コンストラクタの `<TArrayBuffer extends ArrayBuffer & { BYTES_PER_ELEMENT?: never } |
SharedArrayBuffer & { BYTES_PER_ELEMENT?: never }>` constraint 内の intersection が
JSON に出現するようになり、旧来の raw 文字列化 fallback が dangling external ref
として probe 検出された。

**インパクト**: 現状は型推論の constraint 弱化のみ (silent semantic break ではない)
が、メソッドシグネチャの param/return 型に intersection が現れるとその構造情報が
失われ、`Any` への退避は後段で型エラーを誘発する可能性がある。Hono では未観測。

**推奨対応**:
1. `tools/extract-types/src/types.ts::ExternalType` に
   `{ kind: "intersection"; members: ExternalType[] }` を追加
2. extractor の intersection 分岐で member 型を再帰的に変換
3. `src/external_types/mod.rs::convert_external_type` で intersection を struct merge
   に展開 (transformer 側 `intersections.rs` のロジックと共有可能か検討)
4. PRD 化が妥当 (新 schema + loader 拡張 + 既存 intersection 経路との整合性)

**優先度判定 (todo-prioritization.md L1-L4)**:
- L4 (局所的問題) — 現状 Hono ベンチで observable な regression なし、constraint
  弱化のみ。ただし将来 builtin lib 範囲拡大時に L3 に昇格する可能性あり

---

### T-2: 型パラメータ constraint が同 scope の sibling param を参照できない (TypeResolver パスは解消済、external_types パスは未対応)

**更新 (2026-04-08, T2.A-ii 完了)**: TypeResolver パスは `enter_type_param_scope` で
scope push を constraint 変換**前**に行うよう変更したため、TypeResolver 経由の sibling
参照ケースは解消した。`src/external_types/mod.rs::convert_external_typedef` /
`convert_external_signature` パスは旧来の順序のまま (scope push が constraint 変換後)
で残存。外部 builtin JSON で sibling 参照を含む constraint は現状未観測のため未対応。

(以下旧記載)



**発見**: 2026-04-08 (T2.A-i 実装中、`convert_external_typedef` / `convert_external_signature` の設計検討時)

**箇所**: `src/external_types/mod.rs::convert_external_typedef` (interface 単位) および
`convert_external_signature` (method 単位)

**現状**: `converted_type_params` を構築する際、各 `tp.constraint` の変換 (`convert_external_type`) を
**`push_type_param_scope` の前** に実行している。このため、`<K, V extends Record<K, string>>` のように
constraint が sibling type param (`K`) を参照する場合、その `K` は scope に未登録のため
**dangling external ref として leak** する可能性がある。

**発見経緯**: T2.A-i 実装時、constraint と scope push の順序を意識したコメントを残したが、
sibling 参照ケースの安全性を未検証のまま現状の順序で確定した。Hono / lib.es5 / lib.dom の
範囲では sibling-referencing constraint の実例は未観測で、現状 regression なし。

**インパクト**: 現状 silent。lib 拡張 (lib.es2020+ など) や user-defined builtin 追加で
sibling-referencing constraint が現れた場合に dangling stub が生成される可能性。

**推奨対応**:
1. constraint 解決を 2-pass にする: 先に param 名のみで scope を push し、その後 constraint を変換
2. または constraint なしの scope push → 各 constraint 変換 → 完成した `converted_type_params` を
   typedef に格納、という順序に変更
3. `<K, V extends Record<K, string>>` 相当の test fixture を追加して RED-GREEN で検証

**優先度判定**: L4 (現状未観測)。T2.A-iii 完了後に他の scope 補完タスクと合わせてバッチ対応するのが妥当。

---

### T-3: `convert_interface_as_fn_type` が overload の最大 params signature しか採用しない (T2.A-ii 検証で誤認判明、別仮説に修正)

**更新 (2026-04-08)**: T2.A-ii の trace 解析で当初仮説 (E 漏れの原因) は誤りだったと判明。
P/S は TypeResolver scope 漏れが root cause で、E は **expected type 経由の free type
param 伝搬** という別経路 (現 master-plan の T2.A-iv) で発生していた。本箇所の overload
切り捨ては silent semantic loss として独立した defect であることに変わりはないが、
E の dangling とは無関係。

**現状の独立 defect 評価**: `interfaces.rs:158` の `max_by_key` は依然として未選択 overload を
完全に捨てているため、TypeScript の overload semantics を保てていない。Hono ベンチで
observable な regression は無いが、将来 overload を活用する fixture (例: `Array.prototype.push(...items)` の
overload バリエーション) が増えると顕在化する可能性。

(以下旧記載)



**発見**: 2026-04-08 (T2.A-ii 調査の前段、`SSGParamsMiddleware` referencer 解析時)

**箇所**: `src/pipeline/type_converter/interfaces.rs:158-161`

```rust
let sig = call_sigs
    .iter()
    .max_by_key(|s| s.params.len())
    .ok_or_else(|| anyhow!("no call signatures found"))?;
```

**現状**: interface が複数の call signature overload を持つ場合 (例: `SSGParamsMiddleware` の
`<E>(generateParams) | <E>(params)`)、最も params が多い 1 つだけを採用し、他の overload は
完全に捨てている。

**インパクト**: 未選択 overload の param 型 / return 型 / 副作用が IR に反映されない。
T2.A-ii (`E` の dangling) の root cause 候補の 1 つ。Rust に「同名複数 signature」の概念が無いため
完全には保てないが、現状 silent dropping は **silent semantic loss** に該当する。

**推奨対応**:
1. T2.A-ii 調査の中で正確な発生経路 (trace) を確認した上で対応設計を決める
2. 候補: 全 overload を Vec として保持し、param 型を union 化して 1 関数にマージ、または
   各 overload を別の trait method として展開
3. T2.A-ii の修正と密結合のため、本 TODO は T2.A-ii 完了時に削除/更新する

**優先度判定**: L1 (silent semantic loss) 候補。ただし T2.A-ii 解消の副産物で消える可能性が
高いため、独立対応ではなく T2.A-ii の中で扱う。

---

### T-4: `npm install` で typescript ^5.9 の最新版が入り `web_api.json` が大幅増 (+17k 行)

**発見**: 2026-04-08 (T2.A-i の JSON 再生成時)

**箇所**: `tools/extract-types/package.json` (`typescript: ^5.9.0` の caret range)

**現状**: T2.A-i の `npm install` 実施で、過去に JSON が生成された時点の TypeScript より
新しい lib.dom.d.ts / lib.es5.d.ts が取り込まれ、`web_api.json` は約 6800 → 約 23800 行に
増加 (差分の大半は signature_type_params 追加ではなく、新 API の追加)。

**インパクト**:
- 過去の JSON が **古い TS バージョン由来** で出力されていた可能性が高い → 今後ローカル/CI
  によって含まれる builtin 範囲が変動する unstable な状態
- JSON 再生成タイミングが個人環境依存になり、再現性が低い

**推奨対応**:
1. `package.json` を `typescript: "5.9.x"` のように patch range に固定 (またはより厳密に pin)
2. または `package-lock.json` を必ず使う運用にし、`npm ci` で再現性を担保する手順を README に明記
3. JSON 再生成は CI で実施し、人手は触らない運用も検討

**優先度判定**: L3 (拡大する技術的負債) — 環境差分による silent JSON drift は将来の bench /
変換結果の不一致原因になり得る

---

### T-5: `SyntheticTypeRegistry` の dedup first-write-wins が scope 漏れを silent に伝播させる

**発見**: 2026-04-08 (T2.A-ii の trace 解析時)

**箇所**: `src/pipeline/synthetic_registry/mod.rs::register_union` (および
`register_struct_dedup` / `register_intersection_enum`)

**現状**: 構造的に同一の union/struct/enum は `union_dedup` / `struct_dedup` /
`intersection_enum_dedup` で**最初に登録された名前を返す**。`Item::Enum.type_params`
は最初の登録時の `type_param_scope` から計算されるため、

1. パス A (例: TypeResolver) が空 scope で `MergeSchemaPath<...> | S` を登録 →
   `MergeSchemaPathOrS` (type_params=[]) が dedup に格納される
2. パス B (例: Transformer の T7-T9) が正しく `["S", ...]` scope で同じ union を登録 →
   **dedup ヒットで bad first entry を再利用** → S が dangling external ref として leak

**インパクト**:
- T2.A-ii で P/S が解消したのは「すべての register_union 経路に scope push を補完したから」
  ではなく、結果的に「TypeResolver パスでも正しい scope が active になるよう全カバーした
  から」。1 箇所でも push 漏れがあれば再発する
- silent: スコープ漏れが新規に追加されても synthetic enum の type_params が空になるだけ
  で、コンパイルエラーにも test failure にもならない (dangling stub に化けるまで気づかない)
- 将来の機能追加で `register_union` を呼ぶ新パスを追加するたびに scope push を忘れる
  リスクが構造的に存在

**推奨対応**:
1. **(a) dedup を type_params 込みで判定**: 同じ member 構造でも type_param_scope が
   異なれば別 entry にする。ただし monomorphize 等との整合性検討が必要
2. **(b) merge on dedup hit**: dedup ヒット時に既存 entry の `type_params` と新規 scope
   を append-merge する (空→非空への昇格を許容)
3. **(c) 検出機構**: register 時に member 型の free 型変数を計算し、`type_param_scope`
   に該当しないものがあれば debug_assert で発火する (新規 scope 漏れの早期検出)

**優先度判定 (todo-prioritization.md L1-L4)**:
- L2 (設計基盤) — 「scope push 補完」という同種の問題が今後の register_* 呼び出し追加で
  繰り返し発生し、毎回 silent に regression する。早期に構造的に防ぐべき
- 対応は I-382 完了後の Batch 候補。または PRD-B (= I-382 本体) 設計時に併せて検討

---

### T-6: `expected_types` マップが Fn 型だけ保持し原型 generic 情報を失う

**発見**: 2026-04-08 (T2.A-ii の E trace 解析時)

**箇所**: `src/pipeline/type_resolver/expressions.rs::resolve_arrow_expr` の
`expected_types.get(&arrow_span)` を読む箇所、および expected type を **書き込む** 側
(var_decl visitor 等)

**現状**: `const ssgParams: SSGParamsMiddleware = (params) => async (c, next) => {...}`
のような pattern で、`SSGParamsMiddleware` interface の call signature `<E extends Env>(...)`
が解決され `expected_types[arrow_span]` には `RustType::Fn { params: [..Context<E>..] }`
が格納される。**`Fn` 単体には元の type_params 情報が無い** ため、arrow が自身では
`<E>` を宣言していない場合、E は free 変数として arrow body に伝搬し dangling refs に化ける。

**インパクト**:
- 現状 master-plan の T2.A-iv (E 1 件残存) の root cause
- TS の generic interface を arrow に annotation する pattern は Hono / Express / fastify
  等の middleware library で頻出、将来的に類似 leak が増える可能性

**推奨対応** (T2.A-iv 設計時に決定):
1. expected type の値型を `(RustType, Vec<TypeParam>)` のペアにし、call signature 解決時
   に type_params を保持。resolve_arrow_expr が読む際に scope に push
2. または `expected_types` を `RustType` のままにし、arrow 進入時に expected type 内の
   free `Named` 名を抽出して暫定 scope push (構造的に近いが誤検出リスク)
3. または arrow が自身では type_param を宣言しない場合、expected type 内の free 名を
   `Any` に置換 (silent semantic loss にはならないが callee 型情報が失われる)

**優先度判定**: L3 (拡大する技術的負債)。T2.A-iv の本体タスクとして対応する。

---

## 完了済み (参照用、定期削除)

なし
