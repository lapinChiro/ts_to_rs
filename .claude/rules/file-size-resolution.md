---
paths:
  - "src/**"
  - "tests/**"
---

# File-Size Threshold Resolution

## When to Apply

`./scripts/check-file-lines.sh` が 1000 行超過を報告したとき。または、編集中にファイルが
1000 行に近づき分割を検討するとき。

## Core Principle

> **ファイルサイズ超過は「機械的な切り出し」ではなく「凝集度向上の機会」として扱う。
> 行数を減らす最短経路を選ぶのではなく、関連実装も含めて再構成し、DRY 違反 +
> 低凝集 + 結合過多 を同時に解消する設計を求める。**

行数超過自体は「設計が肥大化している signal」であり、症状の単純な隠蔽 (= 該当
ファイル末尾を別ファイルに切り出す) は本質的解消にならない。むしろ、その signal を
契機に、周辺の関連実装も含めた大局的な構造を見直す。

## Resolution Procedure

行数超過を発見したとき、以下を順に検討する。**Step 1 / 2 を skip して Step 3 に
直行することは禁止** (= 機械的切り出しのみで済ませる anti-pattern)。

### Step 1: 周辺関連ファイルの調査

超過ファイル単体ではなく、その周辺の関連ファイル群を調査する:

- 同じ module 配下の sibling files (同 directory 内の `*.rs`)
- 超過ファイルが import / re-export している types / functions の定義場所
- 超過ファイルから参照されている関連 logic の所在
- 超過ファイル内の関数 / 型を import している外部 module

これらのファイル群を一つの「設計ユニット」として把握する。

### Step 2: DRY + 凝集度の問題点 enumerate

設計ユニット全体に対して以下を自問する:

1. **DRY 違反**: 同じ知識 (= 変換ルール / dispatch table / 判定 logic 等) が
   複数箇所に重複していないか? 重複があれば、それを構造的に解消する設計 (=
   constructor / helper / shared module) を検討する。
2. **凝集度低下**: 1 file 内に責務が混在していないか? 関連する type と detector が
   別 file にあって認知コストが高くないか? (= `UserMainKind` が `mod.rs` に、その
   detection 関数が `user_main.rs` にある状態は **低凝集 anti-pattern**)
3. **結合過多**: 切り出し候補が、他 module への依存を増やしていないか? 切り出した
   後の依存方向が pipeline integrity (parser → transformer → generator) と整合する
   か?

### Step 3: 再構成 plan の選択

Step 1, 2 の調査結果を踏まえ、以下の選択肢から **最も凝集度を高める** ものを選ぶ:

- **選択 A — 関連 type と detector を同一 file に移動**: 例: `UserMainKind` enum
  を `mod.rs` から、それを生成する `detect_user_main` がある `user_main.rs` に
  移動。両者を同じファイルに置くことで認知コストが下がる
- **選択 B — DRY 解消用の constructor / helper を新設**: 例: `(exec_mode,
  user_main_kind) → UserMainSubstitution` の dispatch table を 2 箇所で重複した
  `match` で書く代わりに、`UserMainSubstitution::from_dispatch(exec_mode, kind)`
  constructor を 1 箇所定義し、両 call site から呼ぶ
- **選択 C — sub-module への分割 (cohesive group)**: 1 つの「責務単位」が大きく
  なっている場合、その責務単位を sub-module 化する。**単純な末尾切り出し** ではなく
  **意味的に独立した group** を sub-module にする
- **選択 D — 既存 sub-module への合流**: 切り出し候補が既存の sub-module の責務と
  cohesive な場合、新ファイルを作らず既存 sub-module に合流させる

**機械的な末尾切り出しは禁止**。Step 1 で関連ファイルを把握せず、ただ末尾を別 file
に move する選択は anti-pattern (= 設計問題の隠蔽)。

### Step 4: 検証

再構成後、以下を verify:

- `./scripts/check-file-lines.sh`: 全 file < 1000 行
- `cargo check --tests`: compile error 0
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings
- `cargo test --lib` + 関連 integration test: 全 pass
- 設計 cohesion review: 「再構成後、type と detector が同じ場所にあるか?
  duplicated knowledge は単一 source of truth 化されたか? 結合度は上がっていないか?」を
  自問

## Recurring problem rationale

新規 enum / struct 追加で existing module が 1000 行を僅か数行超過する pattern では、
**機械的末尾切り出し** (新型のみを新 file に move) は anti-pattern として再発しやすい。
typical な構造は: (a) 隣接 module 群に DRY 違反 (同一 dispatch table の複数 site 重複) と
低凝集 (型定義と detector 関数の別居) が pre-existing で存在、(b) 新型は既存型から
派生する dispatch state なので単純切り出しでは凝集度をさらに下げる、(c) 真の理想解は
"選択 A (型と detector の同居化) + 選択 B (DRY 解消用 constructor 新設)" の組合せで、
line overshoot 解消と structural improvement を同時達成する。**Step 1-2 を skip して
Step 3 の選択を行うと、この組合せ解は見えない** ため、procedure の structural enforcement が
prerequisite。

## Prohibited

- 行数超過の解消手段として、**周辺ファイルを調査せず** ファイル末尾を機械的に
  別 file に切り出すこと (Step 1, 2 を skip)。
- DRY 違反 / 低凝集 / 結合過多 が周辺に存在するのに、行数超過のみを fix して
  これらを放置すること (= 設計問題の隠蔽)。
- **「速く済ませるため」「scope 拡大を避けるため」を理由に Step 1-3 を簡略化** すること
  (`feedback_no_dev_cost_judgment.md` 違反)。
- 再構成後の検証 (Step 4) を skip すること。

## Related Rules

| Rule | Relation |
|------|----------|
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則。機械的切り出し禁止 = ideal 達成の手段の 1 つ |
| [design-integrity.md](design-integrity.md) | 凝集度 / DRY / 結合度 review の 4 観点 (本ルールと相補) |
| [prd-design-review.md](prd-design-review.md) | PRD 設計 review の 3 観点 (cohesion / 責務分離 / DRY) |
| [bulk-edit-safety.md](bulk-edit-safety.md) | 再構成時の bulk replace は本ルールの dry run 原則を併用 |
| [pipeline-integrity.md](pipeline-integrity.md) | 切り出し後の依存方向は pipeline integrity と整合 |
| [large-scale-refactor](../skills/large-scale-refactor/SKILL.md) (skill) | 1 PRD 内に閉じない構造 refactor を伴う場合は本 skill を併用 |

