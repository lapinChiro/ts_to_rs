# plan.prd.md — 新 framework PRD の計画 (meta-plan)

## 目的

本ドキュメントは、**PRD を作るための計画** である。I-142 PRD で観測された「毎
review で同オーダーの Tier 1-2 defect が発見される」構造的欠陥を解消するため、
どのような PRD (複数可) を新規作成すべきかを導出し、その計画自身を批判的に
レビューして磨く。

計画の最終形を確定した後、それに基づいて実際の PRD (`backlog/*.md`) を作成する。
計画段階で spec-first の思考を適用することで、PRD 作成段階で既に bottom-up
enumeration の罠を避ける。

---

## 1. 問題提起

### 観測された pattern

I-142 PRD (`??=` NullishAssign Ident LHS structural rewrite) の 3 実装サイクル:

| Cycle | 実装 scope | 実装後 review で発見された Tier 1-2 defect 数 |
|---|---|---|
| Step 1 完了 → Step 2 起票 | reported defect 修正 | 6 (Cell #5/#9/#14/#6/#10/#12 gap) |
| Step 2 完了 → Step 3 起票 | matrix + pick_strategy 集約 | 7 (D-1〜D-7) |
| Step 3 完了 → Step 4 起票 | 敵対レビュー defect 解消 | 10 (C-1〜C-9 + D-1) |

各サイクルで発見される defect の数・severity が収束しない。実装者の intuition が
深化するに伴って review の insight も深化するため、defect 発見は無限に続く構造。

### これは異常か

通常のソフトウェアでも review は何かを発見する。しかし:

- 発見される defect が **Tier 1 (silent compile error lock-in、silent semantic change)
  含む** → polish ではなく correctness issue。
- **収束しない** (reduction ratio が 1 以下にならない)。
- 同じ PRD 内で 3 cycle も続く → local な「もうひと頑張り」で解決しない兆候。

これは process-level の構造的欠陥の徴候であり、I-142 固有の実装問題ではない。

---

## 2. 根本原因の再確認

前セッションの俯瞰観察で特定した 4 つの structural 欠陥:

### 原因 1: 「理想」が declaration であって derivation でない

PRD 内で「ideal 出力は X」と宣言するが、その根拠が:
- 実装者 (= 私) の intuition
- または先行 implementation の結果

であり、**プロジェクト外部の reference** (tsc 挙動 / Rust 仕様 / 形式意味論) で
grounding されていない。

結果: 「理想」が実装者の mental model に依存し、mental model の盲点が理想の盲点
になる。Review で blind spot が指摘される循環。

### 原因 2: 問題空間 enumeration が bottom-up

`problem-space-analysis.md` は top-down 列挙を要求するが、実運用では:
- 思いつく AST variant を列挙
- 思いつく Type variant を列挙
- 思いつく Context を列挙

という bottom-up 作業になっている。**Grammar (SWC AST 定義) / Type system (IR
`RustType`) / Context grammar (transformer 呼出構造) からの形式的直積計算** は
行われていない。

結果: Grammar に存在する variant のうち、私が「??= で使われるのを見た」ものだけ
が matrix に入る。見たことがない variant は silent に欠落する。

### 原因 3: Test が implementation verification

Unit test の実態:
```rust
assert!(out.contains("x.unwrap_or(0.0)"))
```
これは **私が書いた実装が私が書いた実装通りに output を生成する** ことを検証。
TS→Rust の意味論保存は検証していない。

結果: Test passing は「実装が期待通りに動いた」ことの証拠にはなるが、「変換が
正しい」ことの証拠にならない。C-2 (closure test が silent compile error を
lock-in している疑い) はこの帰結。

### 原因 4: Review が specification-finding process

Review の実態: 実装を見て「この case は spec に入ってなかった」「この emission は
TS 意味論と乖離してる」と発見する作業。つまり **review 自身が spec の不足分を
discover** している。

結果:
- spec が不完全なまま implementation が進む。
- review の insight 次第で後から spec が拡張される。
- reviewer の「気づき」の深さが収束 criterion になり、objective な完了条件が
  不在。

---

## 3. 原因 → PRD 導出

Root cause 1〜4 を解消するために、PRD が提供すべき artifact:

| 原因 | 必要な artifact | 種別 |
|---|---|---|
| 1. ideal が declaration | 外部 oracle (tsc) で observed behavior を記録する workflow | Process + tooling |
| 2. bottom-up enumeration | Grammar-derived matrix template (SWC AST variant catalog, RustType variant catalog, Emission context catalog) | Reference doc + tooling |
| 3. test = implementation check | Per-cell E2E fixture harness (tsc stdout vs cargo stdout 比較) | Test infrastructure |
| 4. review = spec finding | Spec-stage adversarial review workflow (implementation 未着手で spec review) | Process + rule |

これら 4 つは **同時に揃わないと効果を発揮しない**:
- Tooling 単体: process がなければ使われない。
- Rule 単体: tooling がなければ実行 friction が高く守られない。
- Pilot 単体: 上記なしには spec-first になり得ない。

よって、これらを **単一の methodology shift** として扱う。

---

## 4. 計画 v1 — 初期案

Root cause × artifact で 5 PRD に分割 (bottom-up 推論):

### PRD A: External Oracle Infrastructure
- tsx-based TS 実行 harness (fixture stdout 取得)
- Per-cell E2E 比較 runner
- 既存 E2E を新 harness に移行
- **Deliverable**: `tests/e2e/oracle/`, parametric runner

### PRD B: Grammar-Derived Matrix Tooling
- SWC AST 定義 parse → variant 列挙 tool (`scripts/derive-matrix.py`)
- RustType enum parse → variant 列挙
- Emission context 列挙 (transformer 呼出 grep)
- **Deliverable**: `scripts/derive-matrix.py`, `.claude/rules/matrix-derivation.md`

### PRD C: Spec-First PRD Workflow Rule
- `.claude/rules/spec-first-prd.md` 新規
- `/prd-template` skill 更新
- `/check_job` skill 更新 (spec 準拠 check に転換)
- **Deliverable**: rule + skill 更新

### PRD D: Pilot Retrofit of I-142
- 新 workflow で I-142 を再検証
- 残 defect (Step 4 の C-1〜C-9 + D-1) が pilot で検出・解消されるか検証
- **Deliverable**: `backlog/I-142-spec.md` + per-cell E2E fixtures + 新 defect 報告

### PRD E: Matrix Completeness CI Gate
- `scripts/check-matrix-coverage.sh` — 各 matrix-driven PRD の cell coverage check
- CI integration
- **Deliverable**: CI gate + gate 違反時の message

### 依存関係 v1
```
A ──┐
B ──┼──> D ──> E
C ──┘
```
A/B/C parallel → D (pilot) → E (automation)。

---

## 5. 計画 v1 の批判的レビュー

### 問題 1-1: PRD A が over-engineered

**問題**: 既存 E2E framework (`tests/e2e/scripts/`, `tests/e2e_test.rs`) は既に
tsx vs cargo 比較を行っている。PRD A が求めるのは「per-cell granularity」と
「parametric runner」だけ。ゼロから harness を構築するのは over-engineering。

**refinement**: PRD A を「E2E harness per-cell 拡張」に縮小。新規構築ではなく
**既存 framework の extension**。

### 問題 1-2: PRD B は自動化が脆弱

**問題**: SWC AST 定義の parse は crate version に依存 (21.0.0 の enum 定義)。
上流変更で script が壊れる。RustType enum parse は自プロジェクトなので制御可能
だが、emission context 列挙は transformer code 全体の grep に依存 (refactor で
壊れやすい)。

**refinement**: 自動化を諦め **reference document (静的)** として作成。
`doc/grammar/ast-variants.md` 等。PRD 作成時に手動で参照し、variant 漏れを防ぐ
gate として使う。上流変更は手動 sync (SWC upgrade 時)。

### 問題 1-3: PRD C のみでは enforcement がない

**問題**: rule を書いただけでは守られない。PRD 作成者 (= LLM session) が rule を
load して遵守するには、rule 自身に「spec 段階で review を強制する checkpoint」
が埋め込まれている必要がある。

**refinement**: rule に **機械的 checkpoint** を組み込む:
- PRD template に「Grammar derivation section (必須)」「tsc observation section
  (必須)」を入れる。
- 空欄 / 「T.B.D.」の状態での review は block。
- `/check_job` skill に「spec 段階なら spec review、実装段階なら spec 準拠 check」
  の dispatch を入れる。

### 問題 1-4: PRD D の pilot 対象が不適

**問題**: I-142 を retrofit するのは cost が高く、かつ既に defect が documented
済なので validation signal が弱い。「既知の defect を再発見できた」だけでは
process が機能した証拠にならない (reviewer が bias で同じ defect を見つける)。

**refinement**: Pilot 対象を **新規 PRD に変更** — I-050 umbrella の最初の sub-PRD
(I-050-a)、または他の未着手 matrix-driven PRD。Fresh application で新 process の
defect 発見能力を validate。

### 問題 1-5: PRD E は validation 前には作れない

**問題**: Pilot が成功するまで何を gate すべきか不明。Pilot 段階で gate を作る
のは順序逆転。

**refinement**: PRD E を **Pilot 完了後の follow-up** に延期。初期 PRD 群から
除外し、「Phase 4 Rollout」内で定義。

### 問題 1-6: 5 PRD 並列は独立性の幻想

**問題**: A/B/C を独立 PRD にすると「A は A の scope で完成、B は B の scope
で完成」を目指すが、D (pilot) で統合する時に noise が発生する。例: A の harness
が B の reference doc を読む必要があると発覚しても、既に A は closed。

**refinement**: A/B/C/D を **1 つの PRD (複数 phase)** に統合。methodology shift
は atomic に実施しないと、phase 間の impedance mismatch で中途半端に終わる。

### 問題 1-7: 計画 v1 自身が bottom-up enumeration

**問題**: 私が「思いつく deliverable」を列挙している。これは新 framework が批判
している手法そのもの。計画自身を spec-first で構築すべき。

**refinement**: Root cause 1〜4 を **derivation の起点** として再出発する。何を
artifact として提供すれば root cause が解消するかを、直接 derive する。

---

## 6. 計画 v2 — 初回 refinement

### 単一 PRD 化 + root cause 駆動 derivation

**新 PRD: Spec-Driven Conversion Development Framework (以下 SDCDF)**

Root cause ごとに derivation:

#### Root cause 1 解消 → 外部 oracle workflow

- Artifact 1a: **Rule section** "tsc による観測を ideal 根拠とする"
- Artifact 1b: **PRD template update**: "tsc observation" section (必須、空欄
  不可)
- Artifact 1c: **Helper script**: `scripts/observe-tsc.sh <fixture.ts>` →
  `{stdout, exit_code, errors}` を JSON で出力

#### Root cause 2 解消 → grammar-derived matrix

- Artifact 2a: **Reference doc** `doc/grammar/ast-variants.md` (SWC AST 変換関連
  enum の variant catalog)
- Artifact 2b: **Reference doc** `doc/grammar/rust-type-variants.md` (IR
  `RustType` / `PrimitiveIntKind` / `StdCollectionKind` catalog)
- Artifact 2c: **Reference doc** `doc/grammar/emission-contexts.md` (transformer
  emission contexts catalog)
- Artifact 2d: **Rule section**: PRD の matrix は reference doc から直積計算に
  基づいて enumerate、NA justification は spec で行う (intuition 不可)

#### Root cause 3 解消 → per-cell E2E

- Artifact 3a: **Test infrastructure**: `tests/e2e/scripts/<prd-id>/<cell-id>.ts`
  layout、既存 E2E runner を parametric 化
- Artifact 3b: **Helper**: `scripts/record-cell-oracle.sh` — cell fixture から
  tsc output 自動観測し expected output として記録
- Artifact 3c: **Rule section**: 各 matrix cell に E2E fixture 必須、substring
  assertion は shortcut のみ (primary assertion は stdout 一致)

#### Root cause 4 解消 → spec-stage review

- Artifact 4a: **Rule section**: PRD lifecycle を "spec" / "implementation" の 2
  stage に分離、spec stage 完了で敵対レビュー実施、実装は spec stage approved 後
- Artifact 4b: **Skill update**: `/check_job` が spec stage / implementation stage
  で dispatch (spec stage: matrix 完全性 / tsc oracle 根拠 / NA justification
  review、implementation stage: spec 準拠 check)
- Artifact 4c: **Pilot application** に "spec stage review → implementation stage
  review" の 2 段階実施

### Phase 構成

**Phase 1: Foundation** (rule + reference doc + helper script)
**Phase 2: E2E harness extension** (既存拡張 + record helper)
**Phase 3: Pilot** (Fresh PRD — I-050 sub-PRD or similar)
**Phase 4: Rollout** (Pilot 成功時、一般 PRD に rule 適用)

---

## 7. 計画 v2 の批判的レビュー

### 問題 2-1: Reference doc の maintenance burden

**問題**: SWC AST variant catalog は 100+ variant を含む。SWC upgrade のたびに
手動 sync 必要。stale 化するとまた grammar-derived が机上の空論になる。

**refinement**: Reference doc に **version snapshot** を明記 (SWC version,
observation date)。更新 trigger を明示: SWC / IR 変更時は reference doc 更新を
commit 単位で同時に。できれば `scripts/check-grammar-doc.sh` で stale 検出
(SWC source の enum variant 数 vs doc の variant 数比較で簡易 gate)。

**補助**: SWC は variant 追加が低頻度なので burden は許容範囲。IR (RustType) は
高頻度変更だが自プロジェクト内で `#[non_exhaustive]` 等の構造で検出可能。
emission context は最も流動的だが、catalog は transformer call site refactor
(D-1 の `iter_block_with_reset_check` helper 的な統一) で一元化すれば自動 sync
できる可能性。

### 問題 2-2: E2E per-cell は slow

**問題**: 100 cells × cargo build & run = minutes 単位。開発中にこれを回すのは
friction 大。

**refinement**: E2E per-cell は **CI で実行、local では sub-suite のみ**。Unit
test (substring assertion) は primary development loop、E2E は correctness gate。
`cargo test --features e2e-cells` のような feature gate 化。

**注意**: unit test が primary loop だと結局 implementation verification が主に
なるリスク。対策: unit test を書くときに cell id コメントを必須化、spec との
trace が強制される。

### 問題 2-3: Pilot の成功指標が arbitrary

**問題**: 「defect 数 ≤ 2」とか決めても、評価者 (review を誰がやるか) で結果が
変わる。

**refinement**: 成功指標を **categorical** にする:
- Pilot 実装後の review で発見される defect を以下に分類:
  - **Grammar gap**: reference doc に記載されていない variant が関与 → grammar
    doc を補強
  - **Oracle gap**: tsc observation が不十分 → observation を補完
  - **Spec gap**: matrix に未 enumerate cell が存在 → 上記 2 つのどちらかに
    trace
  - **Implementation gap**: spec 通りでない実装 → 実装修正
  - **Review insight**: spec も実装も正当、レビュアーの新たな気づき → 新 sub-PRD
- **Pilot 成功条件**: 発見 defect がすべて "Grammar gap" / "Oracle gap" /
  "Implementation gap" に trace でき、"Spec gap (reference doc / oracle から
  derivable だったが enumerate 漏れ)" が **0**。
- 「Review insight」は上記とは独立に発生 OK。ただし insight 起点の新 sub-PRD
  は spec-first workflow で処理。

### 問題 2-4: Spec-stage review の内容定義が弱い

**問題**: spec stage で review すべき項目が具体化されていない。review が ad-hoc
になる risk (root cause 4 と同じパターンを再生産)。

**refinement**: Spec-stage review の **checklist** を rule に embedding:
1. Matrix の全 cell に ideal output が記載されているか (空欄 / TBD は block)
2. Ideal output が tsc observation log と cross-reference されているか
3. NA justification は spec (syntax error 等) に trace できるか、「稀」「multi-prd」
   等の曖昧理由は排除されているか
4. Grammar doc に記載されていない variant が matrix に存在しないか (存在すれば
   grammar doc 更新 or matrix 訂正)
5. 各 cell に対応する E2E fixture が (red 状態で) 準備されているか

Checklist の全項目が [x] でない状態での implementation 開始は block。

### 問題 2-5: Pilot 対象の先 (Retrofit vs New) 判断未

**問題**: v1 で「新規 PRD に pilot 対象変更」と書いたが、具体候補が未選定。

**refinement**: **候補比較表**:

| 候補 | Pros | Cons |
|---|---|---|
| I-050-a (Any coercion, let-init+return × String) | 未着手、新 workflow 純粋適用、小 scope で iteration 早い | I-050 umbrella design が spec-first で整う前に sub-PRD を切る必要 |
| I-142-b (FieldAccess LHS `??=`) | I-142 の延長、defect pattern が既知 | 既知 defect を再発見するだけの risk (v1-4 問題の再燃) |
| I-144 (control-flow narrowing analyzer) | Large scope で framework の stress test、Hono 影響大 | Large scope で pilot 失敗時の cost 大 |

**判定**: **I-050-a** を推奨。理由:
- 未着手 (bias なし)
- Scope が small (一context + 一source type から開始可能)
- 成功時に I-050 全体への rollout が自然 (umbrella を spec-first で拡充)
- 失敗時の cost も I-144 より低い

ただし I-050 umbrella 自体の spec が spec-first で整うのが先決。I-050-a scope を
定義するには I-050 umbrella の grammar-derived matrix が必要。

### 問題 2-6: Bootstrap 順序の rigor

**問題**: 新 framework の rule / reference doc / harness を整備した後で pilot
を行うが、pilot を行う前に framework の完全性は証明できない。一方、framework を
完全に整備してから pilot というのは bootstrap paradox。

**refinement**: **Iterative bootstrap**:
1. **Alpha**: Phase 1 の reference doc を minimum viable で作成 (全 variant 列挙
   を目指すが、漏れは Pilot 中に補強前提)。Rule も draft。
2. **Pilot**: I-050-a spec 作成中に reference doc の不備が判明 → その場で修正 +
   log。
3. **Beta**: Pilot 完了後、reference doc と rule を修正結果で version up。
4. **Rollout**: Beta 版で他 PRD に適用。

この iterative path は「rule を spec-first で整備する」という requirement 自身
にも spec-first を適用する自己参照なので、Bootstrap paradox を避けられる。

---

## 8. 計画 v3 — 二回目 refinement

### SDCDF PRD の最終形 (pending final review)

#### 目的

TS→Rust conversion PRD の開発プロセスを、implementation-first から specification-first
に転換する。具体的には root cause 1〜4 を解消する artifact 群を提供し、pilot
application で validate、成功確認後に rollout する。

#### Scope

以下を含む:
- Rule: `.claude/rules/spec-first-prd.md`
- Reference docs: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- Helper script: `scripts/observe-tsc.sh`
- Helper script: `scripts/record-cell-oracle.sh`
- E2E harness extension: `tests/e2e/scripts/<prd-id>/<cell-id>.ts` layout,
  parametric runner
- Skill update: `/prd-template`, `/check_job`
- Pilot: I-050-a (spec + implementation + review)

以下を含まない (non-goals):
- 既存 PRD (I-142, I-022, I-138, I-040) の retrofit
- Matrix derivation の自動化 (静的 reference doc のみ)
- Non-matrix PRD (infra, refactor) への framework 適用
- CI gate 自動化 (Pilot 完了後の follow-up PRD)

#### Phase 1: Foundation (Alpha — iterative)

1.1. `.claude/rules/spec-first-prd.md` draft 作成:
  - PRD lifecycle (spec / implementation stage 分離)
  - Spec stage artifact 要件 (grammar derivation / tsc observation / matrix
    completeness / NA justification)
  - Spec-stage adversarial review checklist (5 項目、上記 問題 2-4 参照)
  - Implementation stage: spec 準拠 check のみ、ad-hoc 再 review 禁止

1.2. `doc/grammar/ast-variants.md` alpha 版:
  - SWC `swc_ecma_ast` の Expr / Stmt / Pat / AssignTarget / AssignOp /
    UpdateOp / BinOp / UnaryOp 等の variant 列挙
  - 各 variant の「??= で使われ得るか」「narrowing 影響有無」等の分類 column
  - Version snapshot: SWC crate version, observation date

1.3. `doc/grammar/rust-type-variants.md` alpha 版:
  - `RustType` 18 variants + `PrimitiveIntKind` 13 + `StdCollectionKind` 12
  - 各 variant の TS 由来型 (reverse mapping) + 典型用途
  - Version snapshot

1.4. `doc/grammar/emission-contexts.md` alpha 版:
  - Transformer の emission context 分類: let init / return / call arg / match
    arm / field assign / cond branch / arr elem / obj field / template / method
    recv / NC RHS / ??= RHS / etc.
  - 各 context の expected-type propagation 経路
  - Version snapshot

1.5. `scripts/observe-tsc.sh`:
  - 入力: `.ts` fixture path
  - 出力: `{stdout, stderr, type_errors, exit_code}` JSON
  - TypeScript 5.9.3 使用 (tools/extract-types の node_modules 共有)
  - tsx 経由 runtime stdout も取得

#### Phase 2: E2E harness extension

2.1. `tests/e2e/scripts/<prd-id>/<cell-id>.ts` layout 確立:
  - 既存 `tests/e2e/scripts/*.ts` は top-level。新規 PRD は subdir 必須。
  - cell-id は `<lhs>-<context>-<rhs-class>` 等の systematic 命名。

2.2. Parametric runner:
  - `tests/e2e_test.rs` に macro `cell_e2e_test!(<prd-id>, <cell-id>)` 追加
  - 1 cell = 1 test function、Cartesian 時は macro で展開
  - `cargo test --test e2e_test -- <prd-id>::` で PRD 単位実行可

2.3. `scripts/record-cell-oracle.sh`:
  - 入力: cell fixture path
  - Action: `observe-tsc.sh` 実行し結果を `<cell-id>.oracle.json` として記録
  - CI / review 時に oracle と実 Rust stdout を比較

#### Phase 3: Pilot (I-050-a)

3.1. I-050 umbrella の spec-first 再構築:
  - `backlog/I-050-any-coercion-umbrella.md` を spec stage で再起票
  - Matrix を grammar-derived 形式で列挙 (reference doc 使用)
  - Sub-PRD 分割基準を spec 内で確定 (context 軸 / source type 軸の選択)

3.2. I-050-a (最初の sub-PRD) 選定 + spec stage:
  - 最小 scope: `let-init + return` context × `String` source type ×
    `AST shape {Lit, Ident}`
  - Grammar 由来 cell 列挙
  - tsc で全 cell 観測
  - Spec-stage adversarial review (checklist 全項目 [x] 確認)

3.3. I-050-a implementation stage:
  - Per-cell E2E fixture 先行書き (red)
  - Spec 準拠で実装
  - E2E all green で implementation 完了

3.4. Post-implementation review:
  - 発見 defect を 5 category (Grammar gap / Oracle gap / Spec gap /
    Implementation gap / Review insight) に分類
  - 成功条件: **Spec gap (derivable from grammar+oracle but missed) = 0**

#### Phase 4: Rollout criterion

4.1. Pilot 成功時:
  - Rule を draft から正式 rule に昇格
  - `/prd-template` / `/check_job` skill 更新を正式 version
  - Alpha reference doc を Beta に昇格 (Pilot で判明した不備を反映)
  - 以降の matrix-driven PRD は spec-first 必須

4.2. Pilot 失敗時 (Spec gap > 0):
  - 失敗の root cause を分析:
    - Grammar doc に漏れ? → doc 拡充
    - Oracle observation 不足? → helper script 強化
    - Review checklist 不備? → rule 改訂
  - 修正版で pilot 再実施 (I-050-b 等)
  - 2 連続失敗時は framework 自体の再設計 (エスカレーション)

#### Non-goals (明示)

- **既存完了 PRD の retrofit**: I-142 の Step 4 defect は引継ぎドキュメントに
  記録済。新 framework で再着手するか、個別 sub-PRD で処理するかは rollout 後に
  決定。
- **Matrix derivation 自動化**: SWC / IR parse は fragility 大。静的 reference
  doc で管理。
- **Non-matrix PRD への適用**: infra / refactor PRD は matrix を持たないため
  本 framework の対象外。現行 `prd-template` で十分。
- **Full CI gate**: spec 段階の Mechanical gate は rule + checklist で実現、
  automated CI gate は Phase 4 以降の follow-up PRD。

#### 成功指標

Pilot application (I-050-a) の post-implementation review で:
- **Spec gap = 0**: 発見された defect は全て Grammar gap / Oracle gap /
  Implementation gap のいずれかに trace 可能 (spec 自身は complete だった)
- **Implementation gap = 0**: spec 通りでない実装は 0 (spec 準拠 check が有効)
- **Grammar gap / Oracle gap / Review insight**: 存在してよい (改善 signal として
  記録、reference doc / rule / harness の update に反映)

Baseline: I-142 Step 3 baseline = Tier 1-2 defect 10 個。
Target (stretch): Spec gap = 0 達成 + 他 gap 合計 ≤ 3。

---

## 9. 計画 v3 の批判的レビュー

### 問題 3-1: 「Spec gap = 0」の計測は本当に可能か

**問題**: Post-review で「これは Spec gap だ / 違う」の判定自体が主観になる。
reviewer が「grammar doc に書いてあった」と確認する作業が必要。

**refinement**: **Trace requirement を明示**:
- Review で defect を発見したら、reviewer は必ず以下を報告:
  - 「この defect は reference doc のどの entry から derivable か」
  - derivable なのに missed なら Spec gap
  - reference doc に entry がなければ Grammar gap
  - entry はあるが tsc observation が不足なら Oracle gap
- Trace 不可能 (どの category か分類できない) な defect は Review insight 扱い。

この trace 要件自身を rule の spec-stage checklist に追加。

### 問題 3-2: I-050-a scope が依然曖昧

**問題**: 「let-init + return × String × {Lit, Ident}」と書いたが、具体 cell
数が未計算。

**refinement**: **cell 数を具体化**:
- Context = 2 (let-init, return)
- Source type = 1 (String)
- AST shape = 2 (StringLit, Ident)
- Target context constraint (Value 必要性) = 2 (yes, no)

2 × 1 × 2 × 2 = 8 cells 程度。Small enough for fast iteration、ただし trivial
すぎず oracle / matrix derivation を practice できる。

### 問題 3-3: Phase 1 alpha reference doc の内容量

**問題**: SWC AST の Expr variant だけで 30+ variants、Stmt も 20+、Pat 10+、
Decl 5+。Emission context も 15+ category。網羅的に書くと doc が巨大化、review
困難。

**refinement**: **Tier 付け**:
- **Tier 1 (必須)**: 変換 pipeline で現行 emit される variant (`Expr::Assign`,
  `Expr::Call`, etc.)。全列挙。
- **Tier 2 (shortcut)**: 現行 `convert_expr` で `unsupported` 扱いの variant
  (`Expr::JSX*`, `Expr::MetaProp`, 等)。名前のみ列挙、semantic 記述は Pilot で
  必要になった時点で拡充。
- **Tier 3 (将来)**: TS parser が accept するが ts_to_rs が見ない variant
  (`Expr::Yield` async 外、等)。NA justify で除外、doc には entry のみ。

alpha 版は Tier 1 + Tier 2 name-only が minimum viable。Pilot で Tier 2 の一部
が必要になれば拡充。

### 問題 3-4: Rule の skill 統合が不明瞭

**問題**: `.claude/rules/spec-first-prd.md` と `/prd-template` / `/check_job`
skill の関係。skill が rule を自動 load するか、手動で呼ぶか。

**refinement**: 既存の rule-skill 統合パターンを踏襲:
- Rule は `.claude/rules/` に置き、`CLAUDE.md` の該当 section から link。
- Skill は rule を明示参照 (`参照: .claude/rules/spec-first-prd.md`)。
- `/prd-template` skill の step 0 に「本 PRD は matrix-driven か?」判定を入れ、
  yes なら spec-first-prd rule を load。
- `/check_job` skill に stage 判定 (「PRD の spec section が完成済か」) を入れ、
  stage 応じて checklist を変更。

### 問題 3-5: 「spec-first」が implementation 否定に誤解される risk

**問題**: 「spec-first」が「implementation は dogmatic に spec 通りに」と誤解され、
implementation 段階での legitimate な discovery (実装で spec の曖昧さが判明) を
抑圧する risk。

**refinement**: Rule に明示:
- Implementation stage で spec 曖昧点を発見したら、**必ず spec に戻る** (spec を
  更新 + review → implementation 再開)。implementation 内で spec を「解釈」する
  ad-hoc 修正は禁止。
- 「spec が書かれていないから勝手に決める」は implementation 段階では禁止、必ず
  spec update round を挟む。

### 問題 3-6: Pilot 失敗時の 2 連続失敗 escalation

**問題**: 「2 連続失敗時は framework 自体の再設計」だけではエスカレーション
基準として弱い。

**refinement**: 失敗の内訳を specific に:
- **連続失敗 1**: reference doc or rule の不備 → 修正再挑戦
- **連続失敗 2** (同種): framework のアプローチ自体の見直し → 以下選択肢:
  - (a) 外部 oracle を tsc 以外に拡充 (e.g., lint tools、formal TS spec)
  - (b) Spec-stage review を人間 (user) 必須に (LLM agent 単独 review を禁止)
  - (c) PRD 粒度を更に小さく (1 cell = 1 PRD レベル)
- 判定: user 合意のもとエスカレーション。LLM session 単独判断禁止。

### 問題 3-7: 本 plan.prd.md 自身が spec-first workflow に従っていない

**問題**: plan.prd.md は本 framework を **作る計画** だが、計画自身が grammar
derivation / oracle observation に従っていない。self-hosting 矛盾。

**refinement**: Plan.prd.md を新 framework の **「Meta-specification」** と位置
付ける:
- Plan は derivation ではなく「root cause → artifact」の causal chain で正当化。
- この causal chain が新 framework の中で "bootstrap 根拠" として参照される。
- 新 framework の rule に「本 rule は plan.prd.md の causal chain から derive
  された」と注記。
- Rule の改訂時は plan.prd.md も同時 update (trace 可能性維持)。

この自己参照は bootstrap paradox の解決策として成立。

---

## 10. 最終計画 (v4 — polished)

### SDCDF PRD

#### 1-liner

「TS→Rust 変換 PRD を **外部 oracle (tsc) と grammar-derived matrix** に grounding
する spec-first workflow を導入し、review-finds-10-defect pattern を構造的に解消
する」

#### 成果物 (artifacts)

| # | Artifact | 種別 | Phase |
|---|---|---|---|
| 1 | `.claude/rules/spec-first-prd.md` | Rule | 1 |
| 2 | `doc/grammar/ast-variants.md` | Reference doc | 1 |
| 3 | `doc/grammar/rust-type-variants.md` | Reference doc | 1 |
| 4 | `doc/grammar/emission-contexts.md` | Reference doc | 1 |
| 5 | `scripts/observe-tsc.sh` | Helper script | 1 |
| 6 | `tests/e2e/scripts/<prd>/<cell>.ts` layout + parametric runner | Test infra | 2 |
| 7 | `scripts/record-cell-oracle.sh` | Helper script | 2 |
| 8 | `/prd-template` skill update | Skill | 1 |
| 9 | `/check_job` skill update | Skill | 1 |
| 10 | I-050 umbrella spec-first 再構築 | PRD | 3 |
| 11 | I-050-a pilot (spec + implementation + review) | PRD | 3 |

#### Phase sequence

```
Phase 1: Foundation (alpha)
  ├── Artifact 1 (rule draft)
  ├── Artifact 2-4 (reference docs, Tier 1+Tier 2 name only)
  ├── Artifact 5 (observe-tsc helper)
  └── Artifact 8-9 (skill draft)
        ↓
Phase 2: Harness extension
  ├── Artifact 6 (E2E layout + runner)
  └── Artifact 7 (oracle record helper)
        ↓
Phase 3: Pilot
  ├── Artifact 10 (I-050 umbrella re-spec)
  ├── Spec stage: I-050-a spec + tsc oracle + matrix derivation + checklist
  ├── Spec-stage adversarial review
  ├── Implementation stage: E2E fixtures (red) → code → green
  └── Post-implementation review: category-label defects
        ↓
Phase 4: Rollout decision
  ├── Spec gap = 0 ? → 正式 rule 昇格 + rollout
  └── Spec gap > 0 ? → root cause 分析 + 再試行 or escalation
```

#### 成功指標

**Pilot (I-050-a) post-implementation review** で:
- **Spec gap = 0** (必須): 発見 defect が reference doc / oracle から derivable
  だったのに matrix に漏れていたものは 0。
- **Implementation gap = 0** (必須): spec と乖離した実装は 0。
- **Grammar gap / Oracle gap / Review insight**: 0 以上 OK。ただし trace 必須
  (いずれの category か明示)。

Baseline = I-142 Step 3 baseline Tier 1-2 defect 10。
Target = **Spec gap 0 + 他 category 合計 ≤ 3**。

#### Non-goals

- 既存完了 PRD (I-142, I-022, I-138, I-040) の retrofit。
- Matrix derivation の自動化 (静的 reference doc で manage)。
- Non-matrix PRD (infra, refactor, bug fix) への適用。
- Full CI gate 自動化 (Phase 4 以降の follow-up PRD として切り出し)。
- Grammar doc の completeness を SWC / IR upgrade に fully 対応させる (Tier 1
  確実に、Tier 2 best-effort、Tier 3 skip)。

#### 依存 + ordering constraints

- Phase 1 は Artifact 1〜5 + 8〜9 を並列で作成、全完了で Phase 2 移行。
- Phase 2 は Phase 1 完了後 (rule が runner の要件を定義するため)。
- Phase 3 は Phase 2 完了後 (pilot が E2E harness を使うため)。
- Phase 4 は Phase 3 完了後。

#### 推定 effort

| Phase | セッション数目安 | 備考 |
|---|---|---|
| 1 | 3-4 | reference doc 3 本 + rule が labor-intensive |
| 2 | 1-2 | 既存 E2E 拡張、比較的 mechanical |
| 3 | 4-6 | Pilot は iterative、spec stage 1-2 session + implementation 2-3 session + review 1 session |
| 4 | 1 (成功時) / 2-3 (失敗時) | 成功時は trivial、失敗時は root cause 分析 + retry |
| **合計** | **9-13** | I-142 3 cycle で 3 sessions 相当使用したことを思うと投資対効果高 |

---

## 11. Bootstrap 自己適用 note

### Meta-specification としての本 plan

本 plan.prd.md は新 framework の **bootstrap meta-specification** として機能
する:

- 計画は root cause → artifact の causal chain で derive されている。
- 計画自身が新 framework の grammar-derived 要件を満たしていない (framework が
  まだ存在しないため) が、それは Bootstrap の初期値として accept 可能。
- Phase 1 完了後、本 plan の artifact list を新 framework の rule で再 review し、
  漏れがあれば plan を update → 新 framework 自身が plan を補強する自己参照的
  改善 cycle を形成。

### 本 plan 自身の批判的 review (meta-level)

ここまで v1 → v2 → v3 → v4 の refinement を経た。各 cycle で以下が排除された:

- **v1 → v2**: 5-PRD fragmentation → 単一 PRD 統合 (root cause 4 が atomic な
  methodology shift を要求するため)
- **v2 → v3**: 成功指標の arbitrariness → categorical classification (Grammar
  gap / Oracle gap / Spec gap / Implementation gap / Review insight の 5 分類)
- **v3 → v4**: phase / non-goal / effort 見積の曖昧さ → 具体化

### 残 risk (v4 時点)

- Pilot 失敗 risk: 未知。Pilot 前に calibrate する方法がない。失敗時の graceful
  degradation path は 9 章 問題 3-6 で定義済。
- Reference doc stale 化 risk: maintenance burden。7 章 問題 2-1 で mitigation
  定義済 (simple gate + 低頻度 sync)。
- Rule の skill 自動適用 risk: skill が rule を load し忘れる可能性。9 章
  問題 3-4 で対策定義済。

---

## 12. 次のアクション

1. **ユーザーによる本計画の承認**
   - 本 plan.prd.md (v4 最終計画) が承認されたら step 2 へ。
   - 承認前の修正要求あれば v5 として追加 refinement round。

2. **新 PRD 作成** (承認後)
   - `backlog/I-SDCDF-spec-driven-framework.md` (仮称) として起票。
   - 本 plan の v4 最終計画を PRD の "Background + Scope + Deliverables" に展開。
   - PRD 自身は新 framework 定着前の作成のため、従来の `prd-template` に沿う
     (self-hosting のゴール達成は phase 3 の pilot で)。

3. **Phase 1 着手**
   - Artifact 1 (rule draft) から開始。draft は本 plan の内容に基づく。
   - Artifact 2-4 (reference doc) を parallel で着手。
   - 各 artifact は小 commit 単位 (draft → review → finalize)。

4. **Phase 2 着手** (Phase 1 完了後)
   - 既存 E2E framework 拡張。
   - harness の first test として trivial な cell fixture (e.g., `x: number | null;
     return x ?? 0;`) で harness 動作確認。

5. **Phase 3 Pilot 着手** (Phase 2 完了後)
   - I-050 umbrella を spec-first で再構築。
   - I-050-a sub-PRD の spec stage。
   - Spec-stage adversarial review → implementation → post-review。

6. **Phase 4 Rollout 判定** (Phase 3 完了後)
   - 成功指標に対する judgment。
   - 成功: rule を正式化、rollout。
   - 失敗: root cause 分析、v5 plan 策定、再試行。

---

## 13. 本計画の使い方

- 本 plan.prd.md は **「新 framework PRD 作成のための計画」**。PRD 自身ではない。
- 承認後、本計画を「背景 + scope + deliverables + phase」として PRD に展開する。
- PRD 本体では具体 implementation step と test design を詳細化する。
- Rule 改訂・framework 進化に伴い、本 plan も update する (meta-specification
  として trace 可能性を維持)。

### Inter-document links

- I-142 close 記録: [`backlog/I-142-nullish-assign-shadow-let.md`](backlog/I-142-nullish-assign-shadow-let.md)
- I-142 Step 4 handoff: [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md)
- 全体 plan: [`plan.md`](plan.md)
- 既存 rule: [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md),
  [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md),
  [`.claude/rules/prd-completion.md`](.claude/rules/prd-completion.md)
