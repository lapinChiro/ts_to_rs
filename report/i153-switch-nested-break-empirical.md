# I-153: switch case body 内 nested `break` silent semantic change 実測レポート

**Base commit**: `38dba52` (uncommitted: TODO/plan.md 修正)
**調査日**: 2026-04-19
**関連**: TODO I-153, `doc/handoff/I-142-step4-followup.md`

## 調査目的

TODO I-153 (switch case body 内 nested bare `break` が outer loop を誤 break する
silent semantic change) の empirical 確認 + 問題空間マトリクスの導出。

## 再現 TS

```ts
function f(x: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 5; i++) {
        switch (x) {
            case 1:
                if (cond) break;    // user intent: break switch, go to count+=10
                count = count + 100;
                break;
            default:
                count = count + 1;
                break;
        }
        count = count + 10;
    }
    return count;
}

console.log(f(1, true));    // TS: 50 (5 iter × 10)
console.log(f(1, false));   // TS: 550 (5 iter × (100+10))
console.log(f(2, false));   // TS: 55 (5 iter × (1+10))
```

## tsx runtime 出力 (oracle)

```
50
550
55
```

## ts_to_rs 変換結果

```rust
fn f(x: f64, cond: bool) -> f64 {
    let mut count = 0.0;
    for i in 0..5 {
        let i = i as f64;
        match x {
            _ if x == 1.0 => {
                if cond {
                    break;    // ← Rust: breaks OUTER LOOP (silent!)
                }
                count = count + 100.0;
            }
            _ => {
                count = count + 1.0;
            }
        }
        count = count + 10.0;
    }
    count
}
```

## cargo run runtime 出力

```
0
550
55
```

## Silent semantic change 確認

| 呼び出し | TS 期待 | Rust 実測 | 一致 |
|---------|--------|----------|------|
| `f(1, true)` | **50** | **0** | ✗ **silent divergence** |
| `f(1, false)` | 550 | 550 | ✓ |
| `f(2, false)` | 55 | 55 | ✓ |

**Tier 分類**: **Tier 1 silent semantic change** (`conversion-correctness-priority.md`)。
rustc は検知せず、runtime まで bug が passthrough。

**優先度**: **L1 Reliability Foundation** (`todo-prioritization.md`)。

## 根本原因

TS `switch` statement 内の bare `break` は **switch を break** する (switch-local scope)。
Rust `match` 式内の bare `break` は **innermost enclosing loop/while/for を break**
する (match/block は bare break target ではない)。

`ts_to_rs` の switch → match 変換は case body をそのまま match arm body に落とすため、
case body 内の nested bare `break` が Rust 側で target 変化を起こす:

- **Target 変化**: switch (TS) → outer loop (Rust)
- **AST 位置**: `src/transformer/statements/switch.rs`
- **Clean-match path** (上記 Rust 出力): guarded match。bare break remain
- **Fallthrough path** (`convert_switch_fallthrough`): labeled-block `'switch`
  を emit するが、bare break は labeled non-loop block target にならないため、
  やはり innermost loop へ流れる

## 既存コード分析

### clean-match path (`src/transformer/statements/switch.rs:420-584`)

`convert_switch_clean_match` が switch を `Stmt::Match { arms }` に変換。
arm body の `convert_switch_case_body` は **top-level** bare break のみ drop:

```rust
// switch.rs:89-103
for (i, stmt) in cons.iter().enumerate() {
    if matches!(stmt, ast::Stmt::Break(_)) { continue; }  // top-level break drop
    if drop_continue && matches!(stmt, ast::Stmt::Continue(_)) { continue; }
    self.pre_check_narrowing_reset(stmt, &cons[i + 1..])?;
    result.extend(self.convert_stmt(stmt, return_type)?);
}
```

**Nested break (inside `if`, nested `switch`, 等々) は convert_stmt でそのまま
`Stmt::Break { label: None }` に変換され、Rust で innermost loop target 化**。

### fallthrough path (`convert_switch_fallthrough`, switch.rs:587-657)

Labeled block `'switch:` を emit し、arm 末尾の明示 break を `Stmt::Break {
label: Some("switch") }` に rewrite する (line 629-633):

```rust
if ends_with_break {
    then_body.push(Stmt::Break {
        label: Some("switch".to_string()),
        value: None,
    });
}
```

しかし **nested bare break は walk されない** (case body の convert_switch_case_body
が top-level のみ処理)。結果として labeled `'switch:` 内の nested bare break は
Rust では:
- outer loop があれば break outer loop (silent)
- outer loop がなければ E0268 `break outside of loop` compile error (検知可能)

## 問題空間マトリクス (SDCDF spec stage の起点)

### 入力次元 1: break の AST 位置 (case body 内)

| ID | Position | 例 | 現状対応 |
|----|---------|-----|---------|
| P-1 | Top-level stmt | `case 1: break;` | ✓ drop |
| P-2 | 末尾 break (fallthrough path) | `case 1: doWork(); break;` | ✓ rewrite to `break 'switch` |
| P-3 | Inside `if` consequent | `case 1: if (c) break;` | ✗ **silent bug** |
| P-4 | Inside `if` alternate | `case 1: if (c) {} else break;` | ✗ silent |
| P-5 | Inside nested `if-else` ladder | `if (c1) break; else if (c2) break;` | ✗ silent |
| P-6 | Inside block stmt | `case 1: { break; }` | ✗ silent |
| P-7 | Inside nested switch | `case 1: switch (y) { case 2: break; }` | TS: inner switch break → outer case body の続き。未検証 |
| P-8 | Inside nested loop (for/while/do-while) | `case 1: for (;;) { break; }` | TS/Rust 一致 (両方 inner loop break) |
| P-9 | Inside labeled block (user) | `case 1: L: { break L; }` | user label 前提、検証要 |
| P-10 | Inside try block | `case 1: try { break; } catch {}` | ✗ silent |
| P-11 | Inside try finally | `case 1: try {} finally { break; }` | ✗ silent |
| P-12 | Inside arrow/fn body | `case 1: () => { break; };` | TS syntax error (function 境界) |
| P-13 | Inside ternary (via IIFE) | 稀 | 省略 |

### 入力次元 2: switch の outer context

| ID | Outer | 例 | 現状影響 |
|----|-------|-----|---------|
| O-1 | 関数本体直下 | `function f() { switch ... }` | nested break → 無 loop → Rust compile error E0268 |
| O-2 | loop 内 | `for (;;) { switch ... }` | nested break → outer loop break (silent) |
| O-3 | labeled loop 内 | `L: for (;;) { switch ... }` | nested break → L break (silent) |
| O-4 | 別 switch 内 | `switch { case 1: switch { ... } }` | nested break → outer switch break? (TS 仕様要確認) |
| O-5 | try block 内 | `try { switch ... } catch {}` | nested break → 無 loop → compile error |
| O-6 | labeled block (user) 内 | `L: { switch ... }` | nested break → L break (silent if user intends switch-only) |

### 入力次元 3: break の label 有無

| ID | Label | 動作 |
|----|-------|------|
| L-1 | bare `break` | P-3〜P-6 で問題発生 |
| L-2 | `break L;` (user label) | Rust でも同名 label があれば正常 |

### マトリクス (P × O × L) 判定 (主要 cell)

| Cell | TS 意味論 | Rust 現状 | 一致? | 修正要否 |
|------|----------|----------|------|--------|
| P-3 × O-2 × L-1 | break switch → 次の case 下行 | break outer loop | ✗ silent | **必須** (empirical 確認済) |
| P-3 × O-1 × L-1 | break switch → switch の後 | compile error E0268 | Tier 2 | **必須** (変換結果 compile 不可) |
| P-6 × O-2 × L-1 | 同 P-3 | 同 P-3 | ✗ silent | **必須** |
| P-7 × O-any × L-1 | inner switch break | TS: inner switch のみ break / Rust: depends | 要検証 | 調査必要 |
| P-8 × O-any × L-1 | inner loop break | 両方 inner loop | ✓ | 不要 |
| P-2 × O-any × L-1 | 同 P-2 (fallthrough path) | rewrite 済 | ✓ | 不要 |
| P-1 × O-any × L-1 | drop | drop | ✓ | 不要 |
| P-3 × O-3 × L-1 | switch break | outer labeled loop break (silent) | ✗ silent | **必須** |
| P-10 × O-any × L-1 | switch break, try 通過 | ? (TryBodyRewrite が絡む) | 要検証 | 調査必要 |

## 修正方針 (structural)

### Option A: Case body の全 break を pre-rewrite (推奨)

**実装**: `convert_switch_case_body` で case body を recursive AST walker で走り、
以下条件を満たす `Stmt::Break { label: None }` を `Stmt::Break { label: Some("switch") }`
に pre-rewrite:

- **Descent 対象**: `Stmt::If.cons/alt`, `Stmt::Block.stmts`, `Stmt::Labeled.body` (user label
  優先)、`Stmt::Try.{block,handler,finalizer}`
- **Descent 非対象** (深いスコープなので target 変化せず): `Stmt::For`, `Stmt::While`,
  `Stmt::DoWhile`, `Stmt::ForIn`, `Stmt::ForOf`, `Stmt::Switch` (nested)、Fn/Arrow body
- **Labeled break の扱い**: `break L;` で L が user-declared なら rewrite 不要 (内部で
  hygiene 保つ)

**Emission 前提**: clean-match path も labeled block `'switch:` を emit するよう変更
(または arm body を closure 化)。現状 clean-match は raw match なので target label が
存在しない。

**Option A-1 (最小変更)**: clean-match + fallthrough の両 path で case body 内
nested bare break を削除 (drop) + 末尾に `continue` 等の fall-through を emit しない
ので、IR 上 switch break の意味論を `continue-to-post-switch` で表現。この場合は
labeled block 不要だが、case body の emission 自体を `{ ... }` block + tail expr で
終端する必要あり (unit type).

**Option A-2 (一貫性あり)**: clean-match + fallthrough の両方で labeled block `'switch:`
emission 化。nested bare break を全て `break 'switch;` に rewrite。

### Option B: Always emit switch as labeled block (全統一)

現行の fallthrough emission path (labeled block) を clean-match にも適用。全 switch
を `'switch: { match { ... } }` にラップ。bare break は AST walk で全て labeled break に
rewrite。Rust label 存在が保証される。

### Option C: Case body を closure 化 (extreme)

各 case body を `(|| { ... })()` にラップして break を prevent。TS の return 意味論が
閉じ込められる問題あり (return は closure 外に効かない)。**不採用**。

**判断**: Option A-2 を採用。理由:
- 統一 emission で mental model 単純
- case body recursive walker は Phase A Step 4 で既に `TryBodyRewrite::rewrite` に
  類例あり (Stmt 再帰 walk pattern 確立済)
- labeled block emission cost 小 (fallthrough 以外でも `'switch:` emit)
- Option A-1 は switch post-break の unit-type handling が case 毎に complex

## I-154 との batching 検討

**I-154** (`'try_block` 固定 label が user labeled block と衝突し得る hygiene 欠落、
`__ts_try_block` 等に変更) は:
- scope: `src/transformer/statements/error_handling.rs::convert_try_stmt`
- 修正: label 名を `"try_block"` → `"__ts_try_block"` に変更 (hygiene 保障)

**Common theme**: ts_to_rs emission が Rust label namespace に固定名を導入する
際の **hygiene 欠落**。I-153 と I-154 は「emission label が user label と衝突しない
ことの保証」という共通責務を持つ。

**Batch 候補**: Label Hygiene & Break Target 複合 PRD
- I-153 (switch case nested break rewrite + label hygiene)
- I-154 (try_block label rename)
- 共通 helper: `fn uniquify_internal_label(stem: &str, user_labels: &[&str]) -> String`
  が user labels と衝突しない internal label を生成

**批判 (反論)**:
- I-153 は AST walker + emission 変更、I-154 は文字列 rename のみ
- 実装規模の非対称性
- Batching による scope 肥大化 risk

**判断**: I-153 先行 (structural core)、I-154 は同一 PRD 内の **sub-step** として取り込む
(2 行変更で user label 衝突を解消)。単一 PRD で 2 issue 解消。

## I-155 との関係

**I-155** (TryBodyRewrite expression walker for defense-in-depth) は try body 内の
Block/Match/If expression の再帰 throw 検出。現時点 reachability なし (plan.md 引継ぎ)。

**I-153 との関係なし**: I-153 は switch 内 break、I-155 は try 内 throw。直交。
batching しない。

## 前提 / 依存

- I-142 Step 4 follow-up に依存しない (独立 scope)
- I-144 に依存しない (control flow emission 責務、type narrowing と直交)
- I-023 / I-021 (Phase A Step 4) 完了が前提 (already closed)

## 推奨 PRD scope

**PRD 名**: I-153 switch case body 内 nested break の outer loop silent redirect 解消

**含める**:
- Option A-2 実装 (clean-match + fallthrough 両 path で labeled block 統一 + case body 内
  nested bare break の AST walker rewrite)
- I-154 (try_block label rename、user label hygiene)
- 問題空間マトリクス cell (上記 P × O × L) の全判定 + per-cell E2E fixture

**含めない**:
- I-155 (独立、defense-in-depth)
- switch の pattern 判定変更 (is_literal_match_pattern 等、別 concern)

## 成果

- I-153 の silent semantic change を empirical に確認 (TSX 50 vs Rust 0)
- 問題空間マトリクス原案を導出
- I-154 との batching 判断 (同一 PRD に取り込む)
- PRD 起票の準備完了

## 参考ファイル

- `src/transformer/statements/switch.rs:83-104` (`convert_switch_case_body`)
- `src/transformer/statements/switch.rs:420-584` (`convert_switch_clean_match`)
- `src/transformer/statements/switch.rs:587-657` (`convert_switch_fallthrough`)
- `src/transformer/statements/error_handling.rs::convert_try_stmt` (I-154 関連)
