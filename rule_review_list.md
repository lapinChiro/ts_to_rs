# Claude Code Rule Review Checklist

Claude Code project (`.claude/rules/*.md`) の rule file を review / 改善する際の汎用観点リスト。
8 group / 24 観点で構成。任意 rule file に適用可能。

## 目的

- Rule file の **structural integrity / content quality / generalizability** を多角的に check
- 第三者 review 時に **観点漏れを防止**
- Rule の long-term maintainability を確保

## 使い方

1. Review 対象 rule file を選定
2. 各観点を「Counter-pattern が file 内に存在するか?」「Test question に "Yes" と答えられるか?」で評価
3. Group A-C は **高優先** (rule の本質定義、long-term integrity に関わる)
4. Group D-F は **中優先** (品質改善)
5. Group G-H は **状況依存** (standard 要素 / large file 対応)

各観点は **独立 application 可能**。発見した violation は別々に fix できる粒度。

---

## Group A: 内容の規範性 (Normative content)

Rule body は normative (規範) content のみで構成されるべき。historical journal / changelog / derivation narrative は外部に委譲。

### A1. Rule body の normative purity
- **観点**: rule body は principle / instruction / constraint のみ含むか
- **Counter-pattern**: 履歴 journal、change log、derivation narrative の混入
- **Test**: 「この記述は rule application 時に load-bearing か? それとも historical context か?」

### A2. 履歴の external delegation
- **観点**: 累積する history は VCS / external archive に委譲されているか
- **Counter-pattern**: `## Versioning` section、`(REVISED vX.Y)` annotation、revision history list
- **Test**: 「この履歴は VCS / commit message / changelog file で recover 可能か?」

### A3. Lesson の pattern essence preservation
- **観点**: 過去事例の **transferable pattern** は保持、**specific incident citation** は除去されているか
- **Counter-pattern**: `Lesson source: <specific event> で <specific defect> 発覚` のみで pattern 抽出なし
- **Test**: 「rule 適用者が pattern を他 case に転用できるか? それとも歴史的事実 record か?」

---

## Group B: Project-agnostic 性 (Generalizability)

Rule は instance-agnostic な principle として記述されるべき。特定 project-internal entity への dependency を排除。

### B1. Instance reference 不在
- **観点**: 特定 project-internal entity (PRD/issue/PR/task/ticket/iteration ID) への参照がないか
- **Counter-pattern**: `I-205 で...`, `PRD 2.7 で...`, `Sprint 12 で...`, `ticket #1234 で...`
- **Test**: 「3 ヶ月後 / 他 project でも同 rule として通用するか?」

### B2. Temporal marker 不在
- **観点**: 日付、iteration version、finding ID 等の temporal information がないか
- **Counter-pattern**: `(確定 2026-04-28)`, `(draft v3 final)`, `F-deep-deep-1 fix`, `Phase 2 で...`
- **Test**: 「rule application 時に時刻情報が必須か? それとも単なる timestamp か?」

### B3. Concrete vs specific の区別
- **観点**: 教育的 concrete example は保持、本質的でない specific value は抽象化されているか
- **Counter-pattern**: `cells 24-28` (= 特定 matrix の cell 番号)、`line 145 の bug`
- **Test**: 「example の specific value は理解の助けになっているか? generic placeholder (N, M, X) で同等か?」

---

## Group C: 参照整合性 (Reference integrity)

Rule 内外の cross-reference が正確 / robust であるべき。rename / restructure に対する resilience も含む。

### C1. Cross-reference の semantic anchoring
- **観点**: rule 間 cross-reference が **意味** に基づくか **label** に基づくか
- **Counter-pattern**: `Rule X (Y-Z) 整合` のような pure label reference (sub-rule rename で break)
- **Test**: 「sub-rule の renaming / restructure で本 cross-reference は break するか?」

### C2. Forward reference の reality check
- **観点**: 未来 entity への reference target が現実存在 (or 確実 plan) か
- **Counter-pattern**: `別 PRD X-NNN で対応` (= 未起票 / cancel 済 / 削除済 entity)
- **Test**: 「reference target を grep で発見できるか? あるいは存在保証あるか?」

### C3. Orphan prefix / dead label namespace の除去
- **観点**: rename history の vestigial 痕跡 (orphan letter prefix 等) が残っていないか
- **Counter-pattern**: parent `(a)(b)(c)` が無いのに `(d-1)(d-2)..` が突然開始する label
- **Test**: 「label の prefix は現行 file structure から explain 可能か?」

---

## Group D: 構造設計 (Structural design)

Sub-rule の構造 / 命名 / 配置が一貫していて navigable であるべき。

### D1. Sub-rule 命名 scheme の uniformity
- **観点**: 同 file / 同 project 内で sub-rule 命名 pattern が統一されているか
- **Counter-pattern**: 同 file 内に `(1-1)`, `(a)(b)`, `(d-N)`, `(e-N)` 混在
- **Test**: 「任意 sub-rule label を見て、所属 rule を一意に同定できるか?」

### D2. Hierarchy depth の justifiability
- **観点**: 各 nesting layer が独立 content を保持し、単なる grouping ではないか
- **Counter-pattern**: `(X) > (X-a) > (X-a-1)` で `(X-a)` が `(X-a-1)(X-a-2)` の grouping label のみ
- **Test**: 「中間 layer を除去すると content が失われるか? それとも flat 化できるか?」

### D3. Single architectural concern per rule
- **観点**: 1 rule が 1 つの architectural concern に集中しているか
- **Counter-pattern**: rule title と乖離した sub-rule (e.g., dispatch rule に numbering convention sub-rule)
- **Test**: 「rule title が全 sub-rule の責務を cover するか? 異質な concern を mix していないか?」

### D4. Related block の structural placement
- **観点**: parent sub-rule の補足 / rationale / example は parent 近接に配置されているか
- **Counter-pattern**: rule-wide closing statement の **後** に sub-rule 特化補足が配置
- **Test**: 「top-down 読みで block ownership が natural に判明するか?」

---

## Group E: Content 品質 (Content quality)

Wording / grammar / readability の基本品質。

### E1. Grammar / syntactic correctness
- **観点**: 助詞・冠詞・単複の正確性、構文破綻なし
- **Counter-pattern**: `Rule N M axis enumerate` (助詞欠落), `Rule N (a)(b)(c)(d) 等` (主語不明)
- **Test**: 「first read で意味が一意に取れるか?」

### E2. Abstract placeholder の semantic clarity
- **観点**: abstract example の variable (X, N, M, Foo 等) の指示先が明確か
- **Counter-pattern**: `"X reclassify"` で X が「reclassify 種別」か「cell 名」か不明
- **Test**: 「placeholder を読み手が一意に instantiate 可能か?」

### E3. Readability anti-pattern の除去
- **観点**: 多重 equivalence chain (`= ... = ... = ...`)、二重否定、過剰 parenthesis 等の anti-pattern なし
- **Counter-pattern**: `禁止 (= compromise = framework integrity 損失 = ...)` の連鎖
- **Test**: 「cause-effect / condition-result の論理関係が明示されているか?」

---

## Group F: Self-consistency (file 内一貫性)

同 file 内での terminology / style / inline reference の一貫性。

### F1. Terminology uniformity
- **観点**: 同概念に同 wording / label
- **Counter-pattern**: `Lesson source` / `Lesson` / `Lesson:` / `**Lesson source**:` の style 混在、同義 wording の表記揺れ
- **Test**: 「同 concept を grep すると 1 種類の表記で hit するか?」

### F2. Inline reference accuracy
- **観点**: file 内 sub-rule への inline reference (`下記 (X-N) 参照`) が actual target を指すか
- **Counter-pattern**: `(13-4) との overlap allow` が semantic に不適切 (= (13-2) が正しい等)
- **Test**: 「inline reference の target を読むと文脈が natural に flow するか?」

### F3. Style consistency (bold / italic / parenthetical)
- **観点**: 同類 information が同 visual style で記述
- **Counter-pattern**: 一部 `**Lesson source**:` bold、他は `Lesson source:` non-bold
- **Test**: 「visual style から information category が一意に判別できるか?」

---

## Group G: Rule structural completeness (補完要素)

Rule として備えるべき standard 構成要素の有無を check。

### G1. `When to Apply` clarity
- **観点**: rule applicability の trigger condition が明示されているか
- **Counter-pattern**: rule body 冒頭で「when」/「scope」が不明、適用 context が implicit
- **Test**: 「rule 適用者が「now is the right time to apply this rule」を judge できるか?」

### G2. Verification mechanism の有無
- **観点**: rule compliance を check する具体 method (audit script, manual checklist, peer review 等) が明示されているか
- **Counter-pattern**: 「禁止」「必須」のみ列挙、check 方法 unspecified
- **Test**: 「rule violation を detect する具体 procedure があるか?」

### G3. Anti-pattern enumeration (`Prohibited` section)
- **観点**: rule violation pattern が enumerate されているか
- **Counter-pattern**: positive 規範のみ、negative example 不在
- **Test**: 「rule 適用者が「これは違反」を判定できる counter-example list があるか?」

### G4. Related rules cross-link
- **観点**: 関連 rule への cross-reference table が存在・最新か
- **Counter-pattern**: 関連 rule への mention 不在、stale link
- **Test**: 「本 rule に関連する他 rule が discoverable か?」

---

## Group H: Loading & resource (Claude Code 固有)

Conversation context への load impact 管理 (large rule のみ要 check)。

### H1. File size eager-load impact
- **観点**: rule file size が conversation context window に与える impact を評価しているか
- **Counter-pattern**: 30KB+ rule が `paths` frontmatter なしで session 起動時 eager-load
- **Test**: 「rule が常に必要か? 特定 file 編集時のみ必要か?」

### H2. `paths` frontmatter による conditional load
- **観点**: 限定 context (特定 file 編集時) でのみ必要な rule に `paths` frontmatter で trigger 設定
- **Pattern**: 
  ```yaml
  ---
  paths:
    - "backlog/**/*.md"
    - ".claude/rules/**/*.md"
  ---
  ```
- **Test**: 「rule の必要性は file path-based trigger で表現可能か?」

### H3. Rule vs Skill 選択
- **観点**: 「常時参照 normative content」= rule、「task-specific reference material」= skill としての分離
- **Counter-pattern**: 30KB の reference material が rule として常時 load (skill 化候補)
- **Test**: 「content の load timing は session-wide か、task-triggered か?」

---

## Review プロセス (推奨運用)

### Step 1: 個別観点 evaluation
各 group 内 観点を順次 check。Counter-pattern hit 時は finding として記録。

### Step 2: Severity 分類
- **Critical**: A1 / A2 / C1 / C2 / D3 violation (rule の integrity 直接損傷)
- **High**: B1 / B2 / C3 / D1 / D2 (long-term maintainability impact)
- **Medium**: D4 / E1 / E2 / E3 / F1 / F2 / F3 (readability / consistency)
- **Low**: A3 / B3 / G1〜G4 / H1〜H3 (完備性 / performance)

### Step 3: Fix 戦略
- **独立 fix 可能** な finding は parallel 実施
- **依存関係あり** の finding (例: rename → cross-ref update) は順序付け
- **設計判断要** の finding (例: rule split vs merge) は user 承認後 implement

### Step 4: Post-fix verification
- 各 group の Test question を再度 evaluate
- Counter-pattern が file 内に残存しないことを grep / audit で confirm
- 外部 file への coordinated change が完了していることを cross-check

---

## 補足: 観点間の関係

```
A (規範性) -- depends on --> B (Project-agnostic)
                            \
B (Project-agnostic) ------> C (参照整合) -- enables --> D (構造設計)
                                          \
D (構造設計) -- enables --> E (Content 品質) <- enables -- F (Self-consistency)

G (Completeness): rule 単体の独立 check
H (Loading): rule operational deployment の check
```

- **A** が確立すると **B** の violations が見えやすい (規範外 content = 削除候補)
- **B** が clean だと **C** の reference が長期 stable
- **D** の structural design clean なら **E/F** の品質 review が容易
- **G** は他 group と独立、rule 完成度の baseline check
- **H** は **A-F** とは別軸の operational concern

---

## 観点リストの maintenance

本 list 自体も rule の 1 形態。periodic review を推奨:

- 新 rule pattern が発見されたら新観点として追加
- 観点が project-specific になっていないか定期 check (本 list の Group B 自己適用)
- 観点間の overlap / redundancy を merge

