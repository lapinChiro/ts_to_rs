# T11 archive — Static accessor (B8) dispatch + matrix expansion

本 file は **I-205 PRD doc から削除される T11 task description を保存** する archive。

## 削除根拠 (2026-05-01 user 確定)

T11 sub-tasks (11-b/c/d/f) は **subsequent review iteration で発見された orthogonal な追加 architectural concern** であり、I-205 本来の concern (= "Class member access dispatch with getter/setter framework"、cells 9/18 の Tier 1 化) とは別軸。元 T11 scope (11-a Static accessor dispatch) は T5/T6 で完了済み (cells 9/18 Tier 1 lock-in 済み)、(11-e) Static setter Write も T6 で完了済み。

残 sub-tasks の architectural concern boundary は以下に再分類され、**2 つの新 PRD として独立起票** される方針:

- **(11-b) Mixed class `is_static` filter** → 新 PRD I-A "Method static-ness IR field propagation" (61 site Field Addition Symmetric Audit)
- **(11-d) Static field associated const emission** + **(11-f) Receiver Ident unwrap robustness** + **I-214 (calls.rs DRY violation + 3 latent gaps)** → 新 PRD I-B "Class TypeName context detection unification" (codebase-wide structural fix via TypeResolver `RustType::ClassConstructor(String)` type marker)
- **(11-c) Static {B3/B6/B7/None field} matrix cell expansion** → 上記 PRD I-A / I-B の completion criteria に integrate (= 4 cells の matrix expansion + tsc oracle + per-cell E2E fixture lock-in)

I-205 PRD doc では (11-c) cells を deferred section に明示 record + 「I-205 scope 外、新 PRD I-A/I-B で expansion」と記載することで `prd-completion.md` matrix 全セルカバー条件 compliance を維持する。

---

## T11 task description (verbatim copy from I-205 PRD doc、line 2287-2326 at commit 1c5d084)

### T11: Static accessor (B8) dispatch + matrix expansion (Iteration v9 second-review 追加 scope)

- **Work**:
  - **(11-a)** `Foo.x` (Foo = TypeName) detection + `Foo::x()` / `Foo::set_x(v)` emit (= 元 T11 scope)
  - **(11-b、Iteration v9 second-review insight #1 由来)** Mixed (static + instance) class context での `is_static` filter 検討。例: `class Foo { static get version() {}; get name() {} }` で `Foo.name` (static access に instance getter) を **dispatch_static_member_read で誤 hit**、生成 Rust が `Foo::name()` (associated fn 不在) で compile error。silent semantic divergence ではない (compile error で surface = Tier 2 等価) が、本質問題 (TS で `undefined` 期待) を user message が伝えない。**Implementation 候補**:
    - **オプション A**: `MethodSignature.is_static: bool` field 追加 (registry/mod.rs + ts_type_info/mod.rs) + collect_class_info で `method.is_static` propagate + 8 production construction site で symmetric audit (= Field Addition Symmetric Audit Rule 9 (c-1) compliance) + dispatch_static_member_read で `is_static = true` の sigs のみ filter、`is_static = false` 混在時は instance method を skip + fallback emit
    - **オプション B**: dispatch_static_member_read で receiver type の context を transformer level で判別し instance method を filter (= registry に is_static field なし、shallow runtime dispatch)
    - **判断基準**: Hono / e2e fixture で reachability audit、Mixed class での static access が reachable なら オプション A (structural fix) 採用、reachable でないなら現状維持 (compile error で surface する Tier 2 等価)
  - **(11-c、Iteration v9 second-review Spec gap #3 由来)** Static × {B3 setter only / B6 method-as-fn-ref / B7 inherited / None static field} の **matrix cell 明示 enumerate**。現 matrix の static cells = 9, 18 (Read/Write static getter/setter) のみ、Tier 2 path の cells が cell 化されていない (本 PRD I-205 T5 で defensive specific wording を導入済 = "read of write-only static property" / "static-method-as-fn-reference (no-paren)" / "inherited static accessor access" / Path-based fallback、`Spec → Impl Dispatch Arm Mapping` の `dispatch_static_member_read` table に明示記載済)。本 T11 で matrix cell 化 + tsc oracle observation + per-cell E2E fixture (red lock-in)
  - **(11-d、Iteration v9 deep deep review で詳細化)** Static field (`Class.staticField`) emission strategy 確定。**現状の挙動 (本 T5 deep deep review で empirical 確認済)**: `dispatch_static_member_read` は lookup hit case のみ呼ばれ、lookup None (= static field、registered class methods に該当 entry なし) は `resolve_member_access` の **最終 fallback (5. FieldAccess) 経由**で `Expr::FieldAccess { object: Ident(class_name), field }` を emit、Rust 出力 = `Class.field` (Rust 上 invalid `.` syntax = compile error E0599 等、Tier 2 等価)。**Ideal output**: `Class::field` (associated const path access、`Config.DEFAULT` → `Config::DEFAULT` で Rust 上 valid)。**Implementation 候補**:
    - **オプション A (generator-level fix)**: `Expr::FieldAccess { object: Ident(name), field }` を generator が emit する際、`name` が registered class TypeName + class.fields に該当 field 登録 + `is_static` field 認識 (cf. Review insight #1) なら `name :: field` (Rust path) として emit。pipeline-integrity.md 違反なし (generator が IR 上の receiver type context を見て decision)
    - **オプション B (IR variant 拡張)**: `Expr::AssociatedConst { ty: UserTypeRef, name: String }` を IR に新規追加、transformer の `dispatch_static_member_read` で None case → instance dispatch fallback の path で **static class TypeName 経由**を分離検出して `AssociatedConst` emit、generator は `ty :: name` で 1-to-1 emit
    - **判断基準**: (a) Hono / e2e fixture で static field access の reachability audit、(b) `MethodSignature.is_static` field (Review insight #1) の追加要否と cohesive integration、(c) IR pipeline-integrity 観点で transformer-side resolution が ideal (= IR が emit context を保持)。reachability あれば オプション B 採用 (IR-level の structural fix)、reachability なくても本 deep deep review で発覚した defective fallback path = subsequent T11 で必須 fix (matrix cell 化 + ideal output 達成は本 PRD scope の completeness)
  - **(11-e、Iteration v9 deep deep review で発覚)** Write context (LHS) で static class TypeName 経由 setter dispatch (`Config.x = 5;` for static setter) は本 T5 で対応外。本 T5 deep deep review fix で Write context は本 T5 Read dispatch logic を skip (`convert_member_expr_inner` の `for_write=true` で 5. FieldAccess fallback 維持) のため、static setter dispatch は subsequent **T6 (Write context dispatch、`dispatch_member_write` helper)** で **instance + static 両方の Write context dispatch arm を統合実装**。T11 は static read 側、T6 は static write 側 (matrix cell 18) の dispatch arm 整合 (= INV-2 External (E1) と internal (E2 this) dispatch path symmetry の static counterpart)
  - **(11-f、Iteration v10 second-review で発覚した pre-existing latent gap、defer 詳細記載)** **Receiver Ident unwrap (Paren / TsAs / TsNonNull wrap) を static gate で対応する robustness 改善**。現 `classify_member_receiver` (T6 v10 で Read/Write 共通化) の static gate は `if let ast::Expr::Ident(ident) = receiver` 直接 match で、以下の TS-valid wrap 経由 access が **static dispatch を逃す**:
    - `(Counter).x = v` (Paren wrap、`ast::Expr::Paren`) — TS-valid、意味的に `Counter.x = v` と同一
    - `(Counter as ClassConstructor).x = v` (TsAs wrap、`ast::Expr::TsAs`) — TS-valid type assertion
    - `Counter!.x = v` (TsNonNull wrap、`ast::Expr::TsNonNull`) — TS-valid (Counter never null)
    - `(Counter satisfies SomeInterface).x = v` (TsSatisfies、SWC support 状況確認要) — TS 5.0+ syntax
    
    これら wrap 経由 access は static gate skip → instance gate (`get_expr_type` で `RustType::Named` 取得試行) → 多くの場合 Fallback fall-through で Tier 2 等価 emit (= **Tier 1 dispatch を逃す latent silent reachability gap**)。
    
    **Pre-existing (T5 から)**: 同 issue は T5 `resolve_member_access` の Enum special case (`Color.Red`) でも `if let ast::Expr::Ident(ident) = ts_obj` 直接 match のため、`(Color).Red` は EnumVariant emit を逃して FieldAccess fallback。T6 で導入された defect ではなく、T5 から続く framework gap。
    
    **Implementation 候補**:
    - **オプション A (helper 関数で AST level peel)**: `unwrap_paren_ts_as_ts_non_null(expr: &ast::Expr) -> &ast::Expr` を `member_access.rs` に追加、`classify_member_receiver` 冒頭で receiver を peel してから `if let Ident` match。pre-existing 全 sites (Read static gate / Read Enum special case / Write static gate) を本 helper 経由に統一 (= DRY refactor with T6 v10 と相補的)。Wrap 経由 access が **TS-valid な static dispatch context で Tier 1 dispatch fire**、TypeResolver expectation との整合保つ (= TypeResolver 上は wrap 内 ident の type で resolve、AST level でも同じ ident を見るのが ideal)。
    - **オプション B (TypeResolver で `ClassConstructor` type marker 拡張)**: `RustType::ClassConstructor(name: String)` variant を新規追加、TypeResolver が `Counter` (class TypeName context) の expr_type を `Some(ClassConstructor("Counter"))` で記録。`classify_member_receiver` の Static gate を `if let RustType::ClassConstructor(name) = self.get_expr_type(receiver)` で AST shape を見ない type-level dispatch に refactor。Wrap 経由でも TypeResolver が type を propagate する限り fire。pipeline-integrity 観点で IR pipeline (TypeResolver → Transformer) の type-information enrichment、structural fix degree 高。
    - **判断基準**: 
      - (a) Hono / e2e fixture で wrap 経由 class TypeName access の reachability audit (= TsAs cast や Paren wrap が class TypeName access pattern で reachable か)。Hono は library code なので reachability 限定的予想、subsequent batch の matrix cell 化判断 driver。
      - (b) オプション A は AST surface 改善 (small footprint)、オプション B は TypeResolver semantic 拡張 (large footprint)。pipeline-integrity 観点では B が ideal (= IR が context を保持)、design-integrity 観点では A が pragmatic (= helper 1 関数で全 site coverage)。
      - (c) T11 (11-b) Mixed class `is_static` filter と同 spec stage (matrix cell 化 + dispatch arm extension) で取り扱う = 両 issue とも static dispatch gate の robustness 改善 = cohesive batch 候補。
    
    **Pre-existing impact range (本 PRD scope 外で影響受ける site)**:
    - `member_access.rs::resolve_member_access` の Enum special case (line ~93-99) — `Color.Red` 等 enum variant access、wrap 経由 reachability あれば silent semantic loss (FieldAccess fallback で `Color.Red` Rust 出力 = invalid `.` syntax compile error)
    - `member_access.rs::classify_member_receiver` の Static gate (Iteration v10 で集約済) — 同 issue
    - その他 transformer 内 `if let ast::Expr::Ident` 直接 match site (= grep 必要、別 audit task として T11 (11-f) 内に integrate)
    
    **本 T6 scope 外定義**: T6 architectural concern = "Write context member access dispatch" は Write/Read symmetric な dispatch logic provision、Receiver expression shape の robustness 改善は orthogonal axis。T6 v10 review で本 issue を発見したが、本 T6 scope 外 = T11 (11-b/f) batch で取り扱う = **1 PRD = 1 architectural concern** + **`design-integrity.md` "broken window detection and response"** (本 entry 自体が broken window 記録 = TODO 起票相当の record-keeping、forget しない structural defense-in-depth)。
- **Completion criteria**: cell 9, 18 unit test green + (11-b) Mixed class reachability audit 結論 + (11-c) Static × {B3/B6/B7/None field} matrix cell 明示 + tsc oracle observation embed + per-cell E2E fixture + **(11-f) Receiver Ident unwrap robustness 改善 (Paren/TsAs/TsNonNull、Iteration v10 source)**
- **Depends on**: T1, T5, T6

---

## 新 PRD への mapping (再構成 mapping table)

| 元 T11 sub-task | 移行先 PRD | architectural concern |
|---|---|---|
| (11-a) Static accessor `Class.x()` / `Class::set_x(v)` dispatch | **完了済 (T5/T6)** | cells 9/18 Tier 1 化、I-205 本体内で達成 |
| (11-b) Mixed class `is_static` filter | **新 PRD I-A** | "Method static-ness IR field propagation" — `MethodSignature.is_static` field addition + Rule 9 (c-1) Field Addition Symmetric Audit (61 site) |
| (11-c) Static {B3/B6/B7/None field} matrix cell expansion | **新 PRD I-A / I-B completion criteria に integrate** | matrix cell 4 件 + tsc oracle observation + per-cell E2E fixture (red lock-in) |
| (11-d) Static field `Class::field` associated const emission | **新 PRD I-B** | `Expr::AssociatedConst { ty: UserTypeRef, name: String }` 新 IR variant + Generator 1-to-1 emit + Read context 経由 dispatch |
| (11-e) Static setter Write dispatch | **完了済 (T6)** | cell 18 dispatch_static_member_write |
| (11-f) Receiver Ident unwrap (Paren/TsAs/TsNonNull) robustness | **新 PRD I-B** | `RustType::ClassConstructor(String)` 新 TypeResolver type marker + 全 Ident match sites unification |
| **I-214 (calls.rs DRY + 3 latent gaps)** | **新 PRD I-B に内包** | `calls.rs:213-225` Static method call dispatch を `classify_member_receiver` 経由に refactor + 3 latent gaps (interface filter / shadowing / inherited) fix |

---

## 関連 reference

- I-205 PRD doc: `backlog/I-205-getter-setter-dispatch-framework.md` (T11 削除前 commit = `1c5d084`)
- 元 T11 提案 source: Iteration v9 second-review (T5 commit、2026-04-28) + Iteration v10 second-review (T6 commit、2026-04-28) + Iteration v9 deep-deep review
- Plan η chain: `plan.md` "次の作業" section
- 本再構成の logic 詳細: 本 file 冒頭「削除根拠」section + 直前の user-assistant 対話 (2026-05-01)
