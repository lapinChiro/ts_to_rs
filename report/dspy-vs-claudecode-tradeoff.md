# DSPy 適用可能性 — Trade-off 分析と最終 verdict

**Base commit**: `984ab19` (uncommitted: TODO / backlog/ / plan.md 等、本 evaluation とは無関係の他 PRD WIP)
**作成日**: 2026-05-11
**目的**: 本 project `ts_to_rs` (TypeScript → Rust 決定的変換器) への DSPy (https://dspy.ai/) 適用可能性を、現状 Claude Code skill/rule workflow との比較軸で評価し、最終 verdict + 借用可能な paradigm 知見を identify する。
**前提 doc**: [`dspy-overview.md`](dspy-overview.md) — DSPy paradigm grounding + 一次資料 references

## 1. Executive Summary

### 1.1 最終 verdict (一文)

> **本 project は既に Claude Code skill/rule という "LM 搭載 dev tool app 構築 framework" 上で
> 高度に運用されており、DSPy framework 採用は cost を justify しない (= Status quo 推奨)。
> ただし DSPy paradigm の "Metric 化 / Optimizer-inspired / Corpus integration" 知見は
> 借用価値が明確で、将来 workflow 体系化 task として TODO 記録 (`[I-DSPY-paradigm-learning]`)。**

### 1.2 評価の核心 framing (user 確定 2026-05-11)

1. **DSPy を transpiler 本体には適用しない** — `ideal-implementation-primacy.md` (deterministic 変換 + Tier 1 silent semantic change 禁止) と DSPy の確率的 LM inference paradigm が直接 conflict
2. **DSPy 適用検討対象 = transpiler 作成 workflow** — PRD / spec / review / test 生成 / 文書整備の prompt-able 部分
3. **真の比較は "DSPy vs nothing" ではなく "DSPy vs Claude Code skill/rule workflow (= 既存)"** — 既存 workflow も LM (Claude) を深く活用している
4. **"DSPy framework 採用 ≠ DSPy paradigm 借用"** — framework として導入しなくても paradigm から学ぶ価値はある

## 2. 構造的等価性 — Claude Code skill ≒ partial DSPy

### 2.1 概念対応

| 概念 | Claude Code (現状) | DSPy |
|------|------------------|------|
| **Task 仕様 (Signature)** | Skill `.md` の "Trigger / Actions / Verification" sections | `dspy.Signature` class / inline string |
| **戦略 (Module)** | Skill 内の手順 + Claude (本 session) の judgment + tool use | `Predict` / `ChainOfThought` / `ReAct` |
| **制約 (Assertions)** | Rule `.md` (例: `spec-stage-adversarial-checklist.md` 13-rule) | `dspy.Assert` / `dspy.Suggest` + retry |
| **検証 (Metric)** | Audit script + 人間 review + `/check_job` 4-layer | Metric function (deterministic or LLM-as-judge) |
| **改善 cycle** | **手動 rule rewrite** + `/rule-maintenance` skill | **自動** (MIPROv2 で metric から最適化) |
| **実行形態** | Interactive session、Claude が tool を駆使 | Python program、stateless invocation |
| **Tool use 能力** | Read / Write / Bash / Edit / Grep / WebFetch / WebSearch / Agent / TaskCreate / etc. (= 14 tool 駆使) | `dspy.ReAct` module 経由のみ、tool は事前定義 Python function |
| **Adaptive 判断** | **Claude が context を読み runtime adapt** | **Fixed Signature、runtime adapt 不可** |
| **再現性** | Conversation context 依存、**非再現** | `save()` した compile output は **完全再現** |
| **永続 artifact** | Skill `.md` (静的) | Compiled program `.json` (optimizer 結果) |

### 2.2 重なる領域と排他領域

```
┌─────────────────────────────────────────────────┐
│                Common ground                     │
│  ・ LM 呼出 を structured に行う                  │
│  ・ Constraint / metric で出力品質 enforce        │
│  ・ Workflow を再利用可能 unit に分解             │
└─────────────────────────────────────────────────┘
        ┌──────────────┴──────────────┐
        ▼                              ▼
 Claude Code 独自                  DSPy 独自
   ✓ Agentic intelligence           ✓ Automatic prompt optimization
   ✓ Multi-tool 駆使 (14 tool)      ✓ Reproducible compile artifact
   ✓ Runtime context adaptation     ✓ Unattended batch automation
   ✓ Interactive review chain       ✓ Quantifiable metric improvement
   ✓ Project-specific rule 大量蓄積  ✓ LM-agnostic portability
   ✓ 0 additional infra              ✓ Embedding in other apps
```

### 2.3 重要な insight

> **Claude Code は "agentic intelligence + adaptive tool use を備えた DSPy-like framework"**、
> **DSPy は "auto-optimization + 再現性を備えた programmatic LLM orchestration"**。
> **両者は競合ではなく overlap を持つ補完関係**。

→ 「**DSPy 適用 = Claude Code を捨てる**」ではない。
「**DSPy 適用 = Claude Code workflow に automation / 再現性 layer を追加**」が正確な framing。

## 3. 4 戦略 (S1/S2/S3/S4)

### S1. Status quo (DSPy 不導入)

**説明**: 現 Claude Code skill/rule workflow をそのまま継続、DSPy 不導入。

**Merit**:
- Infrastructure cost = 0 (既に運用中)
- Maintenance burden = 0
- Claude の **agentic intelligence + multi-tool 駆使** を fully 活用
- 既存 skill / rule / command の蓄積投資を最大化
- `audit-prd-rule10-compliance.py` 等 audit chain と naturally 統合済

**Demerit**:
- 自動 prompt 最適化 不可
- 再現性なし (session 跨ぎで微妙に異なる出力)
- Unattended automation 不可 (cron / CI で workflow 駆動できない)
- batch 処理苦手 (interactive 前提)

**Cost**: 0 incremental

**適合 scenario**: 現状の運用規模 (~月 1 PRD)、interactive 中心開発。

### S2. Full migration (Claude Code skill 全廃 → DSPy program 化)

**Verdict**: **明確に Reject**

**Demerit (致命的)**:
- **Agentic intelligence 完全喪失** = Claude の最大の value loss
- **Multi-tool 駆使 不可** (ReAct module 経由のみ、Read/Write/Bash 等の 14 tool 自作必要)
- **既存 skill / rule の蓄積投資 完全廃棄** (再構築 cost 莫大)
- `ideal-implementation-primacy.md` の human review 多重防御と相性悪い

### S3. Selective DSPy (automation 価値ある surface のみ DSPy 化)

**説明**: 大多数 surface は Claude Code skill 維持、batch automation 系のみ DSPy program 化。

**Merit**:
- 増分 merit (DSPy が真に勝つ surface のみ拾う)
- 現状破壊しない
- 失敗時 rollback 容易

**Demerit**:
- **Dual infrastructure** 維持 (Python toolchain + Claude Code 併存)
- 2 framework 間の **整合性管理コスト**
- Compile cost (~$30-50 / surface)、inference cost (Anthropic API 直接課金)

**Cost**: 中程度 (PoC 1-2 surface = ~2-3 week + ~$50 USD)

**適合 scenario**: batch 系 surface が bottleneck 化、人手 cost を automation で回収できる規模。

### S4. DSPy-behind-Skill (Hybrid: skill が DSPy program を invoke)

**説明**: skill が **DSPy program を tool として呼ぶ** hybrid architecture。DSPy が "core prompt" 部分を担い、Skill が "context 取得 + tool use + review chain" を担う。

**Merit**: 両 framework strength 結合
**Demerit**: Architecture 最複雑、interface 設計工数大、debug 困難
**Cost**: 高
**適合 scenario**: 特定 high-value surface で skill の adaptive 性 + DSPy の最適化 prompt 両方が必要なケース。

### 戦略間の比較 matrix

| 軸 | S1 Status quo | S2 Full | S3 Selective | S4 Hybrid |
|----|:------------:|:-------:|:------------:|:---------:|
| Infrastructure cost | **0** | 莫大 | 中 | 中-高 |
| 既存資産活用 | **最大** | 廃棄 | 維持 | 維持 |
| Agentic intelligence | **fully 活用** | 喪失 | 部分維持 | **fully 活用** |
| Automation 能力 | 低 | **最大** | 部分 | 部分 |
| 再現性 | 低 | **最大** | 部分 | 部分 |
| Risk | **最小** | 致命的 | 中 | 中-高 |
| Verdict | **default 推奨** | Reject | conditional | conditional |

## 4. DSPy が真に勝つ 3 条件 + 該当 4 surface

### 4.1 真の Apply 3 条件

DSPy が Claude Code skill より明確に優位になる条件 (**3 つ全てを満たす場合のみ**):

1. **(R-1) 同 task を繰り返し実施** = 訓練 data が蓄積される (MIPROv2 が有効)
2. **(R-2) Interactive 不可 or 非効率** = unattended automation 必要、または batch >10 件
3. **(R-3) Adaptive 判断不要** = Fixed Signature で十分、runtime context 読込み軽量

3 条件のうち 1 つでも欠ければ Claude Code skill が優位。

### 4.2 該当 surface (4 件)

本 project workflow から 3 条件全 pass する surface を抽出:

| Surface | 説明 | R-1 | R-2 | R-3 |
|---------|------|:--:|:--:|:--:|
| **E2E TS fixture batch 生成** | matrix cell に対応する minimal TS fixture を batch 生成 (1 PRD で数十 cell) | ✓ | ✓ | ✓ |
| **Hono bench error categorize** | `/tmp/hono-bench-errors.json` の `kind` field を意味 cluster 化 (周期実行 candidate) | ✓ | ✓ | ✓ |
| **Hono bench root cause cluster** | bench error list → root cause hypothesis cluster 提案 (cron candidate) | ✓ | ✓ | △ (Hybrid 推奨) |
| **Bench regression detection** | pre/post bench delta から regression cell を抽出 | ✓ | ✓ | ✓ |

**注**: 他の workflow surface (PRD draft / commit message / cross-axis enumeration / `/check_job` review 等) は **(R-2) または (R-3) で Claude Code skill 優位**。Claude の adaptive context 読込み + multi-tool 駆使が essential なため。

### 4.3 該当 4 surface の現状評価

**いずれも現時点では bottleneck 化していない**:
- E2E fixture: 1 PRD あたり数十 cell、interactive 内で処理可能 (`/tdd` skill workflow で十分)
- Hono bench: 周期実行ではなく ad-hoc trigger、`scripts/inspect-errors.py` + 人間判断で対応中
- Regression detection: 同 ad-hoc 処理で対応中

→ **S1 維持の実証的根拠** (= 4 True Apply surface も実用上は Claude Code workflow 内で済んでいる)。

## 5. Decision criteria

### 5.1 S1 vs S3 判定 flow

```
[Q1] 上記 4 True Apply surface のいずれかで
     "人手 cost が回収困難" な bottleneck を感じているか?
   ├─ No  → S1 (Status quo) 推奨、現状維持
   └─ Yes → [Q2] へ

[Q2] DSPy infra (Python toolchain + Anthropic API 課金)
     を許容できるか?
   ├─ No  → S1 推奨、bottleneck は skill 改善で対応試行
   └─ Yes → [Q3] へ

[Q3] PoC 1-2 surface で実測効果を verify する余裕があるか?
   ├─ No  → S1 推奨、判断保留
   └─ Yes → S3 (Selective DSPy) 着手
```

### 5.2 現時点の判断

> **S1 (Status quo) 推奨**。Rationale:
> - 4 true Apply surface は **現状 bottleneck 化していない**
> - DSPy infra 導入 cost は **明確な ROI** を持たない (~$50/PoC + maintenance、現規模では net negative)
> - 既存 Claude Code skill workflow は **十分に well-functioning**

### 5.3 S1 → S3 transition trigger

以下のいずれかが発生したら S3 を re-evaluate (= 4 trigger):

1. **Hono bench を CI / cron で周期実行する PRD が起票される** (= bench error categorize / root cause cluster が automation 必要に)
2. **PRD あたりの cell 数が大幅増 (現 30-60 → 200+)** (= E2E fixture batch 生成が interactive で不可能に)
3. **第三者 review (人間 reviewer) が必要になる** (= Claude 出力を再現可能 artifact として渡す必要、= 再現性 merit が立つ)
4. **Multi-LM 比較が必要になる** (= LM-agnostic portability が merit になる、現状の Claude lock-in が制約に)

これらの trigger が観測されない限り **S1 維持が ideal**。

### 5.4 S3 着手時の PoC 候補

S3 着手判断が future trigger で発動した場合:

| Wave | Surface | 着手理由 |
|:---:|---------|---------|
| **1-a** | **E2E TS fixture batch 生成** | 3 条件全 pass、training data 293 fixture、metric 明確 (tsc pass + cell dimension match) |
| **1-b** | **Hono bench error categorize** | 3 条件全 pass、`scripts/inspect-errors.py` と役割分担 |
| 2 | Bench regression detection | Wave 1 path established 後 |
| 2 | Hono bench root cause cluster | Hybrid (S4) 推奨、context 一部必要 |

## 6. DSPy paradigm 借用 cluster (S1 内での体系化 candidate)

**framework 採用 ≠ paradigm 借用**。S1 維持の枠内で、DSPy paradigm の知見を本 project workflow に **構造的に組み込む** 候補。これが本 evaluation の **forward-looking 主要 deliverable**。

### 6.1 Metric 化 cluster (★ 高 value)

**現状の問題**:
- `audit-prd-rule10-compliance.py` は exit code 0/1 = binary 判定のみ
- `/check_job` の finding count は **plan.md に numeric 記録あり** (v3=17, v5=9, v17=9 等の Iteration trajectory) = 既に **無意識に metric 化** している
- ただし systematic な **「skill 1 適用の品質 score」** は計測していない

**DSPy paradigm から借りる knowledge**:
- Metric は **continuous function** (binary でなく float)
- **Trace 中の中間品質** も metric (= 最終出力だけでなく途中 decision の質)
- **Metric が良くなる方向** が改善 vector として明示化される

**本 project への applicability**:
- **(M-1)** 既存 audit script に **numeric score** 出力追加: 「13-rule のうち何個 pass」「Spec gap finding 何個」「Iteration 何回で convergence」
- **(M-2)** PRD 完成度を 0-100 score 化: pass rule 数 / 全 rule 数 × 100、`audit-prd-rule10-compliance.py` extension
- **(M-3)** Skill 適用結果の quality log: 「`/prd-template` 適用後、`/check_job` で finding 平均 N 件、Iteration 平均 M 回」を `report/` に蓄積
- **(M-4)** Trajectory 分析: 全 PRD 横断で改善 / 悪化 trend を observable に

**着手 candidate**:
- `scripts/audit-prd-rule10-compliance.py` に `--score` flag 追加
- `report/prd-quality-trajectory.md` 新設

### 6.2 Optimizer-inspired cluster (★ 高 value)

**現状の問題**:
- Skill 改善は **ad-hoc** (failure 経験から `/rule-maintenance` で手動)
- 「Skill X を改修してから改善したか?」が **直接観測できない** (= metric 不在の影響)
- 改善 / 悪化 の判定が **subjective sense** に依存

**DSPy paradigm から借りる knowledge**:
- **Compile cycle**: training set + metric → 自動 prompt search
- **A/B test**: 異なる prompt version の metric 比較
- **Bootstrap**: 過去の good example を新 prompt に embed

**本 project への applicability (DSPy framework 不導入で実現)**:
- **(O-1)** Skill 改修 PRD は **Before/After metric** を含める要求化
- **(O-2)** Quarterly **skill rewrite trigger** 制度: 全 skill の最近 3 PRD での適用 metric を集計、metric 悪化 / plateau している skill を **改修 candidate** として list
- **(O-3)** **Bootstrap 借用**: PRD 起草時に過去 5 PRD の "良い PRD" を `/prd-template` skill が **明示参照**、現状の "memory 依存" から脱却

**着手 candidate**:
- `.claude/rules/skill-metric-tracking.md` 新規起案
- `/refresh_todo_and_plan` skill に "skill metric review" step 追加

### 6.3 Corpus integration cluster (★ 高 value)

**現状の問題**:
- 既存 PRD 内の `## Oracle Observations` / `## Invariants` / `## Spec Review Iteration Log` 等は **個別 PRD 内に閉じている**
- `/prd-template` skill 適用時に「過去事例を参照」を **memory 依存**
- = 同様 task で **過去 learning が systematic に活きない**

**DSPy paradigm から借りる knowledge**:
- **Compile artifact** = 学習結果の凍結 + 共有可能化
- Compile artifact = **明示化された prompt + 採用された few-shot examples**
- 新 task は compile artifact を **load して同 quality を再現**

**本 project への applicability**:
- **(C-1)** **Cross-PRD training corpus 整備**: 全 backlog/ から `## Oracle Observations` / `## Invariants` / `## Spec Review Iteration Log` を抽出、`report/corpus/` 配下に **構造化 archive**
- **(C-2)** Skill が参照する **golden examples set** を明示: `/prd-template` skill 内に「過去の高 quality PRD top 3 へのリンク」を hard-code
- **(C-3)** **Cross-axis enumeration の prompt I/II/III 適用結果** を corpus 化
- **(C-4)** **Failure case archive**: Spec gap が発見された case を `report/failures/` に整理

**着手 candidate**:
- `report/corpus/` directory 新設
- `scripts/extract-prd-corpus.py` 新設

### 6.4 3 cluster の優先順位

| Cluster | ROI | 着手難易度 | Dependency |
|---------|----|-----------|-----------|
| Metric 化 | ★★★ | 低 (audit script 拡張) | なし、最初に着手すべき |
| Corpus integration | ★★ | 中 (extraction script 新設) | なし、並列着手可 |
| Optimizer-inspired | ★★★ | 中 (rule 改修 + 周期 trigger) | **Metric 化 cluster に依存** (Before/After metric が前提) |

## 7. Risk

### 7.1 S1 維持 (= 本 report の推奨) の risk

| Risk | 説明 | Mitigation |
|------|------|-----------|
| **Future bottleneck の見逃し** | 4 True Apply surface が将来 bottleneck 化した時に発見遅延 | Section 5.3 の 4 trigger を周期 monitor (quarterly review skill 候補) |
| **Skill rewrite 限界** | Skill の手動改善は労力大、metric 駆動の自動改善が将来必要に | Trigger 観測時に S3 着手 ready、または Section 6 の Metric/Optimizer cluster で部分代替 |
| **Multi-LM portability 不在** | Claude lock-in が constraint になる将来 risk | 問題化したら S3 で対応 |

### 7.2 paradigm 借用 cluster 着手時の risk

| Risk | 説明 | Mitigation |
|------|------|-----------|
| **Metric 化が subjective sense 抑制** | numeric score を盲信して人間判断 軽視 | metric は補助、最終判定は人間 review (`/check_job` 4-layer 維持) |
| **Corpus 整備の維持コスト** | extraction script 維持、archive 同期 | Cron 化、または PRD 完了時の自動更新 hook |
| **Optimizer-inspired の循環依存** | Metric cluster が前提、未整備のまま O cluster 着手 → metric 不在で改善観測不能 | 優先順位 (M → C → O) を遵守 |

## 8. 最終 verdict + 次の action

### 8.1 最終 verdict

> **本 project における DSPy 適用は、現時点では Status quo (S1) が最適。**
> **理由: 4 True Apply surface (E2E fixture batch / bench error categorize / root cause cluster / regression detection) が現在 bottleneck 化しておらず、DSPy infra 導入の incremental merit が cost を justify しない。**
>
> **将来 Section 5.3 の 4 trigger のいずれかが発生したら S3 (Selective DSPy) を re-trigger。**
>
> **並行して Section 6 の paradigm 借用 cluster (Metric 化 / Optimizer-inspired / Corpus integration) は S1 維持の枠内で workflow 体系化に着手可能、TODO `[I-DSPY-paradigm-learning]` に記録済。**

### 8.2 next action

- **(現時点)** 着手なし。本 evaluation の verdict を archive。
- **(将来 trigger 発生時)** Section 5.3 の 4 trigger を観測し、該当時に S3 着手の PRD 起票
- **(future task 余裕時)** Section 6 の paradigm 借用 cluster (Metric / Corpus → Optimizer の順) を順次着手、別 PRD として起票

### 8.3 内包 INV (未解消の調査債務、TODO entry 内に記録済)

- **INV-1**: DSPy v2.5/2.6 production 安定性 (paper / release note / Discord 確認)
- **INV-2**: 既存 PRD の training data 適合性 (S3 着手時の sample 量 / quality 均一性確認)
- **INV-3**: `audit-prd-rule10-compliance.py` の metric 化適合性 (Metric cluster 着手前提)
- **INV-4**: DSPy compile cost 実測値 (S3 PoC で初測)
- **INV-5**: S1→S3 trigger monitoring 機構 (quarterly review skill 候補)

## 9. 将来 framework 比較への汎用 framing

本 evaluation の framing (= 「既存 LM-aware framework との比較で incremental merit を絞る」) は、将来他 framework (LangChain Expression Language / Marvin / Outlines / Instructor / Guidance 等) との比較でも適用可能。共通の検討軸:

1. **Agentic intelligence vs Fixed pipeline** trade-off
2. **Multi-tool 駆使 vs Single-purpose program** trade-off
3. **Adaptive context vs Reproducible artifact** trade-off
4. **Manual rule rewrite vs Auto-optimization** trade-off

DSPy / Claude Code 以外を将来評価する場合も、上記 4 軸での positioning を最初に行うことを推奨。
