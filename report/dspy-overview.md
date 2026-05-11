# DSPy Overview — 本 project 適用性評価のための grounding doc

**Base commit**: `984ab19` (working tree に uncommitted changes 含む: TODO / backlog/I-D-pre-audit-mechanism-bootstrap.md / plan.md)
**作成日**: 2026-05-11
**作成目的**: DSPy (https://dspy.ai/, https://github.com/stanfordnlp/dspy) の paradigm / capability / constraint を精確に把握し、後続 Phase 2-4 (本 project への適用可能性評価) の方針確定に grounding を与える。

## 1. DSPy の paradigm — 一文 summary

> **DSPy は "Programming—not prompting—language models" を実現する Python framework。
> prompt を手書き string として書くのではなく、Signature (入出力契約) と Module (戦略)
> を declarative に記述、Optimizer がメトリクスに対して自動的に prompt / weights を
> 最適化する。**

開発元: Stanford NLP。GitHub stars 34.3k (2026-05 時点)。arxiv 2310.03714 が core paper。
arxiv 2312.13382 (DSPy Assertions) が後続論文。

DSPy のメンタルモデルは **PyTorch 対 HuggingFace Transformers** の関係に近い (FAQ 自身が宣言)
— LangChain / LlamaIndex が pre-built module 集合体だとすれば、DSPy は **program 自体が
data に合わせて学習する** ことを志向した meta-framework。

## 2. Core abstraction 3 階層

### 2.1 Signature (input/output contract)

「LM に何をさせるか」を **入出力契約として宣言** する。prompt 文面は記述しない。

**Inline string 形式**:
```python
"question -> answer"                                  # default = str
"sentence -> sentiment: bool"                          # 型注釈付き
"context: list[str], question: str -> answer: str"     # 複数 field
```

**Class 形式 (Pydantic-style)**:
```python
class Emotion(dspy.Signature):
    """Classify emotion."""
    sentence: str = dspy.InputField()
    sentiment: Literal['sadness', 'joy', 'anger'] = dspy.OutputField()
```

- docstring が **task intent の natural-language spec** として LM に渡る
- `desc` 引数で field 単位の説明追加可能
- 型 validation あり (`Optional[float]` / Pydantic model / `dspy.Image` 等の特殊型対応)
- 実行時に instruction 追加注入可能 (`dspy.Signature("comment -> toxic: bool", instructions="...")`)

### 2.2 Module (LM 呼び出し戦略)

Signature を **どう LM に解かせるか** の戦略。Built-in module 一覧:

| Module | 動作 | 追加構造化出力 | 典型用途 |
|--------|------|--------------|---------|
| `Predict` | 単純 1-shot 推論 | なし | 基本分類 / 抽出 |
| `ChainOfThought` | "think step-by-step" を強制 | `reasoning` field 自動追加 | 推論タスク |
| `ProgramOfThought` | LM に **コードを書かせ実行結果を答えとする** | コード + 実行結果 | 計算系 |
| `ReAct` | 外部 tool 呼び出しを含む agent loop | tool call trace | tool 使用 agent |
| `MultiChainComparison` | 複数の CoT 出力を比較 | 統合結果 | 信頼性向上 |
| `Refine` | self-critique で iterative 改善 | refinement 履歴 | 品質向上 |
| `BestOfN` | N 候補生成 → 最良選択 | scores 付き | 探索 |

合成は **通常の Python class** で書く:
```python
class CustomProgram(dspy.Module):
    def __init__(self):
        self.generate_query = dspy.ChainOfThought('claim -> query')
        self.score = dspy.Predict('query -> score: float')
```

### 2.3 Optimizer ("teleprompter") — DSPy の core 差別化点

> **Module を自動的に「コンパイル」する。compile 後の program は同じ Signature を
> 持つが、prompt 内に few-shot 例や最適化された指示文が埋め込まれた状態になる。**

| Optimizer | 何を最適化するか | 必要 data | 典型 cost |
|-----------|----------------|----------|----------|
| `BootstrapFewShot` | few-shot 例のみ | ~10 例 | $2-5 USD |
| `BootstrapFewShotWithRandomSearch` | few-shot 例 (random search) | 50+ 例 | $5-15 USD |
| `MIPROv2` | **instructions + few-shot 例の両方** (Bayesian Opt) | 50-200+ 例 | $10-50+ USD |
| `COPRO` | instructions のみ (coordinate ascent) | 中程度 | $5-20 USD |
| `BootstrapFinetune` | **model weights 自体** を distill | 多数 | $10-100+ USD |

**MIPROv2 の compilation 3 stage**:
1. **Bootstrapping**: program を反復実行、高 metric score の trace 収集
2. **Grounded proposal**: program 構造 + training data + trace から候補 instructions を生成
3. **Discrete search**: training 例を sampling、instructions 候補を組み合わせ、surrogate model で評価・精緻化

Reference: GPT-3.5-turbo + BootstrapFewShotWithRandomSearch で約 6 分 / 3200 API calls /
2.7M input tokens / 156K output tokens / **$3 USD** (FAQ 記載のベンチ)。

### 2.4 Metric — DSPy が最適化する目的関数

```python
def my_metric(example, pred, trace=None) -> float | int | bool:
    return pred.answer.lower() == example.answer.lower()
```

- deterministic (= exact match / substring) **でも** LLM-as-judge **でも** 可
- `trace` 引数: 最適化時 (`compile()` 中) は中間 LM 呼出の trace を inspect 可能、推論時は None
- bool 返値は bootstrap 用 demo 採否判定にも使用
- 同一 metric を train / eval 両方に流用可能

### 2.5 Assertions — computational constraints (arxiv 2312.13382)

LM 出力に対する **runtime constraint** を組み込む仕組み。retry + feedback 自動注入による
self-refinement を実現。

| 構文 | 失敗時 | 用途 |
|------|------|------|
| `dspy.Assert(cond, msg)` | retry → 上限超で **AssertionError raise (pipeline 停止)** | 必須制約 |
| `dspy.Suggest(cond, msg)` | retry → 上限超で **log のみ (pipeline 継続)** | 推奨制約 |

retry 中、失敗した module の signature に **過去の失敗出力 + error msg** が動的に追加され、
LM が「何を直すべきか」の指示を受けて再生成。default `max_backtracks=2`。

```python
class MyModule(dspy.Module):
    def forward(self, question):
        query = self.gen(question=question).query
        dspy.Suggest(len(query) <= 100,
                    "Query should be short and less than 100 characters",
                    target_module=self.gen)
```

**重要**: Assert/Suggest は LM 出力の確率性を完全排除するわけではない (= retry 後も失敗
する可能性は残る、ただ AssertionError として顕在化する)。byte-exact correctness 要求の
core 変換 path には **不十分** な保証である点に注意。

## 3. Inference 時の挙動 — 「compile したら deterministic」ではない

- compile 出力は `save()` / load 可能、再現性のための cache 機構あり
  (`DSP_CACHEDIR` / `DSPY_CACHEDIR` env var)
- ただし **inference 自体は LM API 呼び出し** であり、temperature > 0 / 異 model 切替 /
  prompt 経年変化等で **依然として確率的**
- AWS Lambda 等の prod 環境では cache 無効化推奨 → さらに **call 単位で別出力可能性**
- 「optimized program が deterministic か」は FAQ 上 "outdated, may not be accurate in
  DSPy 2.5/2.6" とされ **明示否定されていないが保証もされていない**

## 4. DSPy が想定する典型 use case

公式 tutorial を整理 (https://dspy.ai/tutorials/):

| カテゴリ | 例 | 特性 |
|---------|-----|------|
| Classification | sentiment / toxicity / 多クラス分類 | 出力が小さい label 集合、metric 単純 |
| RAG | 質問応答 / multi-hop / financial analysis | retrieve → synthesis pipeline |
| Agent | customer service / memory-enabled ReAct | tool 呼び出し + state 持ち |
| Code generation | **unknown library doc から code 生成** | 検証 mechanism は tutorial に **未実装** (compile/test 不在) |
| Structured extraction | entity / email info / GEPA enterprise | Pydantic 型に従う JSON 出力 |
| Optimization-focused | classification finetuning / GEPA reflective prompt | 既存 program の精度向上 |

**Tier 1 観点で注目すべき点**:
- code generation tutorial が **生成 code を compile / test で検証していない** =
  典型的に LLM 出力は「もっともらしさ」レベルで評価され、byte-exact 等価性は射程外
- agent / RAG は本質的に「中程度の出力品質で十分な user-facing 用途」が main target

## 5. DSPy の limitation — 本 project 評価で load-bearing な点

公式 doc + コミュニティ discussion から抽出:

### 5.1 LM 確率性は paradigm 上排除不能

- compile しても inference は LM 呼出し
- Assertions で retry 可能だが **必ず成功する保証なし**
- temperature=0 / cache 有効化で **「同一 input には同一 output」** を実現できるが、
  これは **入力分布が事前に固定** されている前提

→ **byte-exact correctness を要求する用途 (= 本 project の Tier 1) には根本的に不適合**。

### 5.2 学習 data + metric が必要

- BootstrapFewShot で最小 ~10 例、MIPROv2 で 50-200+ 例、最適化 overview で **300+ 例推奨**
- metric は事前定義必須 (= 「正解とは何か」を Python function で書ける必要)
- → 「正解の定義が困難な領域」「training data が無い領域」では optimizer の効果が限定的

### 5.3 cost / latency

- compile: $2-50+ USD per run
- inference: API call 単位課金 + LM latency (数百 ms-数秒)
- 本 project の cargo build / cargo test は ms-秒 order の deterministic 処理 →
  LM 呼出を絡める surface は performance characteristics が大きく変わる

### 5.4 paradigm-level boundary (`arxiv:2604.05150` "Compiled AI" 議論より)

> "Not all workflows reduce to deterministic code. Tasks requiring genuine creativity or
> adaptation to novel situations may require runtime inference. Compiled AI trades runtime
> flexibility for predictability, auditability, cost efficiency, and reduced security
> exposure."

→ **deterministic compilation との trade-off は paradigm 上 inherent**。
本 project は完全 deterministic 側に位置しており、DSPy の strength (runtime adaptation /
creative output / fuzzy matching) は本 project の core principle と直交または対立する。

## 6. Fit-line — 本 project ts_to_rs との paradigm 比較

| 軸 | ts_to_rs | DSPy |
|---|---------|------|
| 出力決定性 | byte-exact deterministic (Tier 1 silent semantic change 禁止が最上位原則) | 確率的 (Assertions で制約可能だが排除不能) |
| 正解定義 | tsc observation runtime stdout の byte-exact 一致 (`spec-first-prd.md`) | Python metric function (deterministic でも LLM judge でも可) |
| 開発単位 | matrix cell × ideal output の完全 enumerate (`problem-space-analysis.md`) | training example 集合 + metric (label が外部依存) |
| 失敗時挙動 | Tier 2 honest error / Tier 3 unsupported syntax error で **明示拒否** | retry → max 超え時 raise (Assert) or log (Suggest) |
| 言語 | Rust (swc_ecma_parser + IR + Generator) | Python framework |
| 検証 | E2E fixture + tsc oracle + Hono bench (deterministic verify) | Python metric (近似評価) |

### Hard incompatibility 領域

以下 surface は **paradigm-level に conflict** し veto 対象になりうる:
- AST → IR / IR → Rust source 変換 logic 本体 (Tier 1 silent semantic change 不可避)
- TypeResolver の型 inference (silent fallback による semantic change リスク)
- E2E test assertion (近似 metric ≠ byte-exact stdout)

### Soft compatibility 候補

以下 surface は paradigm-level に conflict せず、適用可能性あり:
- error message wording / unsupported syntax の user-facing 説明文生成
- PRD draft / matrix construction の补助 (output は人間 review 前提)
- TODO clustering / prioritization 提案
- Hono bench error JSON のカテゴリ精度向上 (現状 `scripts/inspect-errors.py` の rule-based 補完)
- doc/grammar/* update assist (SWC AST variant の新規 enumerate)

詳細評価および最終 verdict は [`dspy-vs-claudecode-tradeoff.md`](dspy-vs-claudecode-tradeoff.md) 参照。

## 7. 一次資料 references

| Resource | URL | 重要度 |
|----------|-----|------|
| 公式 site | https://dspy.ai/ | ★★★ |
| GitHub repo | https://github.com/stanfordnlp/dspy | ★★★ |
| Core paper (arxiv) | https://arxiv.org/abs/2310.03714 | ★★★ |
| Assertions paper | https://arxiv.org/abs/2312.13382 | ★★★ |
| Modules guide | https://dspy.ai/learn/programming/modules/ | ★★ |
| Optimizers guide | https://dspy.ai/learn/optimization/optimizers/ | ★★ |
| Signatures guide | https://dspy.ai/learn/programming/signatures/ | ★★ |
| Metrics guide | https://dspy.ai/learn/evaluation/metrics/ | ★★ |
| Assertions guide | https://dspy.ai/learn/programming/7-assertions/ | ★★ |
| Tutorials index | https://dspy.ai/tutorials/ | ★ |
| Code generation tutorial | https://dspy.ai/tutorials/sample_code_generation/ | ★★ (本 project と近接) |
| FAQ | https://dspy.ai/faqs/ | ★ |
| "Compiled AI" 対比論文 | https://arxiv.org/html/2604.05150v1 | ★ (paradigm boundary 議論) |

## 8. 本 doc 確立の load-bearing assumption

本 doc を将来参照する際の前提:

- A1: DSPy は **Python 専用** framework。Rust から呼ぶには Python 子 process / FFI 必要
- A2: DSPy 出力は **paradigm-level に確率的**。完全 determinism は cache + temperature=0 で
  近似可能だが保証不能
- A3: DSPy の **強みは training data + metric が揃った状況で task 用 prompt を最適化** できる点
- A4: DSPy Assertions は **constraint 違反検知 + retry** であり、必ず satisfying な出力を
  保証する mechanism **ではない**
- A5: 本 project の最上位原則 `ideal-implementation-primacy.md` (deterministic 変換 / Tier 1 禁止) は
  DSPy の確率的 inference 性質と **paradigm-level に conflict**。conversion core への適用は
  原則として **veto** 対象
- A6: workflow / spec / 文書生成 surface は paradigm-level に conflict せず、適用可能性あり。
  ただし「人間 review 前提」「失敗が rollback 可能」な surface に限定すべき
