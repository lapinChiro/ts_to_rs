# /check_job Review Layers

## When to Apply

`/check_job` 起動時、対象 PRD が matrix-driven (`spec-first-prd.md` 適用対象) の場合、
本 4 layer framework を **初回 invocation から全実施** する。Non-matrix-driven PRD の
場合は Layer 1 (Mechanical) + Layer 4 (Adversarial trade-off) のみ必須、Layer 2-3 は
optional。

`/check_job deep` / `/check_job deep deep` modifier は **廃止**。4 layer は初回から
default で全実施されるため、deep modifier による depth 制御は不要。

## Core Principle

> **review プロセスの「深度」を iteration 進行で深まる現状から脱却し、初回 invocation
> で全 4 layer を必須実施する framework により、defect detection を初回に front-load
> する。各 layer は独立次元の review を担い、Layer 1-4 を全通過しない限り review
> 完了とは認めない。**

empirical evidence (本 framework の lesson source):
- I-161 T7 で /check_job initial → deep → deep deep の 3 度 iteration で defect が
  1 件 / 1 件 / 2 件 と連続発見された pattern を構造的に解消する目的。
- 各 iteration で発見された defect は (a) Truthy 誤発火 / (b) INV-2 path 3 symmetric
  coverage 欠落 / (c) sub-case test 不完全 / (d) Scenario A regression であり、
  iteration を跨いで初めて見えた defect は「review 深度」が iteration で増えていた
  ことに起因 → 初回から 4 layer 全実施で前倒し検出が可能。

## Layer 1: Mechanical (静的解析中心)

### 責務

実装コード / test code / rule 適用状態を **静的解析のみ** で verify する。
コード実行 / probe / fixture validation は Layer 2 で行う。

### Verification Methodology

1. **Code review (PR diff scope)**: 全 diff を読み、以下を check:
   - 妥協した実装 (TODO / FIXME / `unimplemented!` / `panic!` / `unwrap()` の生 production code 残存)
   - エラーハンドリングの skip (`.ok()` で error 握りつぶし、`unwrap_or_default` で silent fallback)
   - 命名 / コメント / doc comment の正確性
   - file size violation (`./scripts/check-file-lines.sh`)
   - clippy / fmt 違反
2. **Test code review**:
   - test name が `test_<target>_<condition>_<expected>` 形式に準拠
   - assertion message の有無 (substring matching に頼らず exact match)
   - decision point ごとに test が存在 (C1 branch coverage)
   - bug-affirming test (誤った expected を assert) の有無
3. **Rule compliance**:
   - 該当 rule (testing.md / pipeline-integrity.md 等) への準拠
   - 鍛え忘れた `unwrap()` / `expect()` の production code 内残存

### 必要 Artifacts

- PR diff (全 file)
- file size report (`./scripts/check-file-lines.sh`)
- clippy / fmt 出力 (0 warning / 0 diff)
- test 実行結果 (cargo test 全 pass)

### Output Format

```markdown
### Layer 1 (Mechanical) Findings

| # | Location | Category | Severity | Action |
|---|----------|----------|----------|--------|
| 1 | foo.rs:42 | TODO 残存 | High | 即時 fix |
| 2 | bar_test.rs:100 | bug-affirming test | Critical | 即時 fix |
```

### Failure Mode (このレイヤーの check が失敗するとき)

`unwrap()` / `panic!` 残存、test 偽陽性、clippy warning など、**コード実行なしで
発見可能な defect が漏れている**状態。Layer 1 で fail した場合、Layer 2-4 に
進む前に修正必須。

## Layer 2: Empirical (probe / fixture validation)

### 責務

実装コードが **実際に実行されたとき** の挙動を probe / fixture で verify する。
Layer 1 の静的解析で漏れた runtime defect を捕捉する。

### Verification Methodology

1. **TS fixture probe**: PRD scope の TS input fixture を作成 (各 matrix cell に
   1 fixture)、`scripts/observe-tsc.sh` で tsc / tsx 出力を取得。
2. **Rust emission probe**: `cargo run -- <fixture.ts>` で生成 Rust code を取得、
   `cargo run` で実行して runtime stdout を tsc 出力と byte-exact 比較。
3. **E2E test execution**: `cargo test --test e2e_test` で全 fixture が green か確認。
   ignored fixture が新規発生していたら理由 annotation を verify。
4. **Hono benchmark (該当 PRD のみ)**: `./scripts/hono-bench.sh` で clean files /
   error count の pre/post 差分を確認、PRD scope 外への regression 0 を verify。
5. **Dual verdict (TS / Rust)**: tsc observation ✓ と Rust emission ✓ を
   独立に verify (`spec-first-prd.md` の Dual verdict framework 準拠)。

### 必要 Artifacts

- 各 matrix cell に対応する TS fixture (`tests/e2e/scripts/<prd-id>/<cell>.ts`)
- tsc observation log (`scripts/observe-tsc.sh` 出力)
- Rust emission probe (`cargo run -- <fixture>` の生成 Rust code)
- E2E test 結果 (cargo test --test e2e_test)
- (matrix-driven のみ) Hono bench pre/post 差分

### Output Format

```markdown
### Layer 2 (Empirical) Findings

| # | Cell | TS observation | Rust emission | Defect |
|---|------|----------------|---------------|--------|
| 1 | C-5 | ✓ stdout=`5` | ✗ stdout=`Some(5)` | Some-wrap 余分 |
| 2 | C-12 | ✓ stdout=`null` | ✓ stdout=`null` | (none) |
```

### Failure Mode

実装が compile pass / unit test pass しているが、runtime で TS と異なる挙動を示す
**silent semantic change** 状態。Layer 2 で fail した場合、`conversion-correctness-priority.md`
Tier 1 (silent semantic change) として最優先 fix。

## Layer 3: Structural cross-axis

### 責務

**自分の解決軸と直交する軸からの cross-check**。Layer 1-2 が「解決軸内の coverage」を
verify するのに対し、Layer 3 は「解決軸外の dimension で見える defect」を捕捉する。
`spec-stage-adversarial-checklist.md` Rule 10 (Cross-axis matrix completeness) の
implementation stage 側 symmetric。

### Verification Methodology

1. **Axis enumeration (post-implementation)**: PRD で確定した解決軸に対して、直交する
   dimension を 3 prompt で抽出:
   - **(I) 逆問題視点**: 解決軸の対立軸を試案化
     (例: 解決軸=cohesion → 反問軸=trade-off / 解決軸=symmetric-coverage →
     反問軸=asymmetric-coverage / 解決軸=preservation → 反問軸=erasure)
   - **(II) 実装 dispatch trace**: 実装の dispatch / branch / pattern-match が消費する
     dimension を全列挙
   - **(III) 影響伝搬 chain**: "X が変わると Y が変わるか?" を再帰適用し間接 dimension
     を抽出
2. **Cross-axis cell sampling**: 抽出した直交軸を matrix に追加し、各 cell に対応する
   probe / test を作成 (Layer 2 と統合)。各 cell が ideal output と一致するか verify。
3. **Spec gap detection**: 直交軸が PRD matrix で enumerate されていなかった場合、
   `post-implementation-defect-classification.md` の **Spec gap** category として記録
   (framework 失敗 signal)。

### 必要 Artifacts

- Cross-axis enumeration table (解決軸 + 抽出した直交軸の matrix)
- 各直交軸 cell の probe / test 結果
- Spec gap detection log (PRD matrix と enumerate された直交軸の diff)

### Output Format

```markdown
### Layer 3 (Structural cross-axis) Findings

#### 直交軸 enumeration
| 解決軸 | 抽出 prompt | 直交軸 |
|-------|-------------|-------|
| cohesion | (I) 逆問題視点 | trade-off |
| symmetric coverage | (II) 実装 dispatch trace | path 3 (negation) |

#### 直交軸 × 解決軸 matrix probe
| 直交軸 | 解決軸 cell | Probe result | Defect |
|-------|------------|--------------|--------|
| trade-off | C-5 narrow-T-shape | RED (E0308) | Scenario A regression |

#### Spec gap detection
- PRD matrix に「path 3 (`!== null` symmetric)」が未 enumerate → Spec gap
```

### Failure Mode

PRD 解決軸内では正しく動作するが、直交軸で defect が発生している状態。
`Cross-axis matrix completeness` rule (spec-stage-adversarial-checklist Rule 10)
violation の implementation 側顕在化。Layer 3 で fail した場合、Spec stage に戻り
matrix を更新する (`spec-first-prd.md` の「Spec への逆戻り」手順発動)。

## Layer 4: Adversarial trade-off

### 責務

fix の trade-off を批判的に評価。「**何を犠牲にして何を得たか**」を明示化し、
pre-fix / post-fix の比較 matrix で failure mode が増えたか減ったかを verify。
fix が trade-off を導入する場合、犠牲 cell が PRD scope 内か scope 外かを判断する。

### Verification Methodology

1. **Pre/post matrix construction**: 全 matrix cell に対して以下を記録:
   - **Pre-fix verdict**: pre-PRD 実装での cell 状態 (✓ / ✗ / NA)
   - **Post-fix verdict**: post-PRD 実装での cell 状態 (✓ / ✗ / NA)
   - **Delta**: ✓ → ✗ (regression) / ✗ → ✓ (fix) / ✓ → ✓ (preserved) / ✗ → ✗ (unfixed)
2. **Trade-off identification**: regression cell (✓ → ✗) を全列挙。各 regression について:
   - **Trade-off statement**: 「<解決軸 A> を fix するために <犠牲 cell B> を regress
     させた」を 1 文で記述
   - **Scope decision**: 犠牲 cell が PRD scope 内 (= fix の延長で追加修正) か scope 外
     (= 別 PRD 起票) かを判定
3. **Patch vs Structural fix evaluation**: `ideal-implementation-primacy.md` の
   patch / structural fix 区分に従い、本 fix が patch (interim) か structural fix か
   を分類。patch の場合は interim 条件 4 件を満たすか verify (条件未充足なら commit 禁止)。
4. **Architectural rabbit hole detection**: fix が deep iteration で発見された場合
   (Layer 1-3 で見つからず Layer 4 で初めて見える)、それは architectural defect の
   patch 化を試みている signal の可能性 → structural fix を別 PRD で起票検討。

### 必要 Artifacts

- Pre/post matrix (全 cell の delta 表)
- Trade-off statement list (各 regression cell の 1 文 statement)
- Scope decision log (PRD scope 内 / scope 外 の判定)
- (patch の場合) interim 条件 4 件の充足記録

### Output Format

```markdown
### Layer 4 (Adversarial trade-off) Findings

#### Pre/post matrix
| Cell | Pre-fix | Post-fix | Delta |
|------|---------|----------|-------|
| C-1 | ✗ | ✓ | fix |
| C-5 | ✓ | ✗ | regression (trade-off!) |

#### Trade-off statements
1. C-5 regression: 「path 2 symmetric coverage を加えるために path 3 (`!== null`)
   側を犠牲にした」 — Scope decision: PRD scope 内、本 PRD で追加 fix 必要

#### Patch vs Structural fix
- 本 fix は **patch** (root cause = TypeResolver suppression scope のため):
  - Interim 条件 (1) Structural fix PRD: I-177-D ✓
  - Interim 条件 (2) `// INTERIM: I-177-D` コメント: 未記載 ✗ → 即時記載必要
  - Interim 条件 (3) silent semantic change なし: ✓
  - Interim 条件 (4) `session-todos.md` 削除基準: 未記載 ✗ → 即時記載必要
- 条件 (2)(4) 未充足 → patch commit 禁止、修正後に再 review
```

### Failure Mode

fix が trade-off を介して別 cell を regression させているのに気付かず commit する
状態、または architectural defect を patch で済ませて root cause を放置する状態。
Layer 4 で fail した場合、`ideal-implementation-primacy.md` 違反として最優先 fix。

## Stage Dispatch (matrix-driven PRD で Spec / Implementation 切替)

`/check_job` 起動時、対象 PRD の現在の stage に応じて review 内容を切り替える:

### Spec stage (Implementation 未着手)

- `spec-stage-adversarial-checklist.md` の **10 項目を全 verification**
  (Layer 1-4 ではなく checklist による review)
- Matrix の各セルに対して「この ideal output は正しいか」を adversarial に検証
- Reference doc (`doc/grammar/`) との cross-check
- **実装コードは review 対象外** (存在しないため Layer 1-4 は skip)

### Implementation stage (Spec approved 後)

- 上記 Layer 1-4 を **初回 invocation で全実施** (deep modifier 不要)
- 追加: 各セルの実装出力が spec の ideal output と一致するかを Layer 2 で verify
- Post-implementation defect classification:
  `post-implementation-defect-classification.md` の 5 category を適用

## Defect Classification 5 Category

各 layer で発見された defect は `post-implementation-defect-classification.md` の
5 category (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight)
に分類する。分類は **trace** に基づく (主観判断ではない)。

特に **Spec gap** (reference doc + oracle から derivable だったが matrix に漏れ) は
**framework 失敗 signal** であり、framework 自体の改善が必要な可能性を示唆する。

## Output Format (全 layer 統合)

```markdown
## /check_job Review Result

### Stage: <Spec / Implementation>
### Target PRD: <PRD ID>

### Layer 1 (Mechanical) Findings
<table>

### Layer 2 (Empirical) Findings
<table>

### Layer 3 (Structural cross-axis) Findings
- 直交軸 enumeration table
- Spec gap detection log

### Layer 4 (Adversarial trade-off) Findings
- Pre/post matrix
- Trade-off statements
- Patch vs Structural fix evaluation

### Defect Classification Summary
- Grammar gap: N
- Oracle gap: N
- Spec gap: N (framework 失敗 signal)
- Implementation gap: N
- Review insight: N

### Action Items
| # | Layer | Action | Severity | Scope |
|---|-------|--------|----------|-------|
| 1 | Layer 1 | Fix unwrap() in foo.rs:42 | High | 本 PRD |
| 2 | Layer 4 | Architectural fix needed | Critical | 別 PRD (I-NNN) |
```

## Prohibited

- Layer 1 のみで review を完了させること (`/check_job 浅い` 状態の構造化)
- Layer 4 trade-off matrix なしに fix を commit すること
- `/check_job deep` / `deep deep` modifier の復活 (4 layer は初回 default 実施)
- Layer 1-3 で発見された defect を Layer 4 trade-off 評価なしに「fix」として commit
  (trade-off が別 cell を regress させている可能性を verify せず)
- patch を「動いているから良い」として commit する (`ideal-implementation-primacy.md`
  違反、interim 条件 4 件の verify が必須)
- `Spec gap` category 発見時に framework rule (`spec-stage-adversarial-checklist.md`)
  の改善を検討せずに fix のみで済ますこと

## Related Rules

| Rule | Relation |
|------|----------|
| [spec-stage-adversarial-checklist.md](spec-stage-adversarial-checklist.md) | 本 framework の symmetric counterpart (Spec stage 側 review)。Layer 3 ↔ Rule 10 (Cross-axis matrix completeness) は同 lesson の review/spec 両面 |
| [post-implementation-defect-classification.md](post-implementation-defect-classification.md) | Defect 5 category の trace 方法。各 layer で発見された defect の分類で参照 |
| [spec-first-prd.md](spec-first-prd.md) | matrix-driven PRD lifecycle workflow。Stage Dispatch logic の base |
| [problem-space-analysis.md](problem-space-analysis.md) | Layer 3 (Structural cross-axis) の理論的根拠 (matrix 完全 enumerate) |
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | Layer 4 (Adversarial trade-off) の patch / structural fix 区分の base 原則 |
| [conversion-correctness-priority.md](conversion-correctness-priority.md) | Layer 2 (Empirical) で発見された silent semantic change の Tier 1 分類 |

## Versioning

- **v1.0** (2026-04-25): I-183 で 4-layer framework として新設。`/check_job` Stage Dispatch を `spec-first-prd.md` から本 file に統合。`/check_job deep` / `deep deep` modifier 廃止。
  - Layer 1 (Mechanical): 静的解析、TODO/FIXME/unwrap 残存等の verify
  - Layer 2 (Empirical): probe / fixture validation、silent semantic change 捕捉
  - Layer 3 (Structural cross-axis): 直交軸 enumeration、Spec gap detection
  - Layer 4 (Adversarial trade-off): pre/post matrix、patch vs structural fix evaluation
  - Lesson source: I-161 T7 三度の `/check_job` iteration での defect re-detection pattern
