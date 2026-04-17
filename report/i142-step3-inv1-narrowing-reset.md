# INV-Step3-1: narrowing-reset の TS 意味論調査

- **日付**: 2026-04-15
- **対象**: I-142 Step 3 D-1 (narrowing-reset 検出 pass) 実装前の TS 意味論確認
- **調査手段**: TypeScript 5.9.3 `tsc --strict --noEmit` による型エラー観察
- **調査ファイル**: `/tmp/inv-step3-1/01..06-*.ts`

## 調査目的

I-142 Cell #14 (`x ??= 0; x = null; return x;`) は現状 shadow-let (`let x = x.unwrap_or(0.0);`) を emit し、後続の `x = None;` が compile error を起こす。D-1 で narrowing-reset を検出して `UnsupportedSyntaxError` を surface する pass を追加するが、**どの shape の assignment を reset と扱うべきか** の TS 意味論が未確定。特に以下を確定する必要があった:

1. 線形の `x = null` 再代入は TS 側で x の narrow を解除するか
2. `if (cond) { x = null; }` のような inner block での再代入は outer scope の narrow を解除するか
3. closure / nested function body 内の再代入は narrow を解除するか
4. for/while loop body 内の再代入は解除するか
5. 非 null literal への再代入 (`x = 5`) は narrow を維持するか

## 観測結果

| Case | TS 挙動 | reset 扱い? |
|------|---------|------------|
| 01 線形 `x = null` | `x = null` 直後、x は `null` literal に narrow (`number` として使用不可) | ✅ reset |
| 02 if block 内 `x = null` | if block 後、outer scope で x は `number \| null` に再 widen | ✅ reset (再帰的) |
| 03 closure 内 `x = null` | closure 呼出し後、outer scope の narrow は維持 (`number`) | ❌ 不変 |
| 04 for-of loop 内 `x = v` (v: `number\|null`) | loop 後、outer scope で x は `number \| null` | ✅ reset |
| 05 nested fn 内 `x = null` | nested fn 呼出し後、outer scope の narrow は維持 | ❌ 不変 |
| 06a 線形 `x = null` | return 時 x は `number \| null` (PRD の元 fixture と一致) | ✅ reset |
| 06c `x = 5` (non-null literal) | narrow は維持 (`number`) | ❌ 不変 |
| 06d `x = y` (y: `number\|null`) | narrow は解除 (`number \| null`) | ✅ reset |

### tsc error output (抜粋)

```
01-linear-reset.ts(12,11): error TS2322: Type 'null' is not assignable to type 'number'.
02-conditional-inner.ts(18,11): error TS2322: Type 'number | null' is not assignable to type 'number'.
04-loop-inner.ts(14,11): error TS2322: Type 'number | null' is not assignable to type 'number'.
06-return-paths.ts(31,5): error TS2322: Type 'number | null' is not assignable to type 'number'.
```

03, 05 でエラー **ゼロ** → closure / nested fn 経由の mutation は TS の CFG 解析で tracking されない (narrow 維持)。

## 結論

### Narrowing-reset 検出 pass の設計要件

#### 1. scan 範囲 (再帰構造)

`??=` が emit した shadow-let 以降、**同一 block 内および以下の nested block を再帰的に scan**:

- `if (...) { ... }` の consequent / alternate
- `for (...)` / `for...of` / `for...in` / `while (...)` / `do...while` の body (loop header 自体 — `for (x of arr)` — の counter-assignment も含む)
- `switch` case body
- block statement `{ ... }`
- `try { ... } catch { ... } finally { ... }` の各 block

**scan 対象外 (narrow 維持)**:

- `ArrowExpr` / `FnExpr` / `FnDecl` (closure / nested function) の body
- `ClassDecl` / `ClassExpr` の method body (sub-scope の closure 相当)

#### 2. 検出対象の AST node

同一 identifier (`??=` LHS と同名) への以下の mutation:

| AST | 例 | 判定 |
|-----|-----|------|
| `AssignExpr { op: Assign, left: Ident(x), right: _ }` | `x = null`, `x = y`, `x = 5` | RHS 型判定必要 (下記) |
| `AssignExpr { op: NullishAssign, left: Ident(x), right: _ }` | `x ??= y` (nested) | ✅ reset (narrow 解除可能性あり) |
| `AssignExpr { op: OrAssign / AndAssign, ... }` | `x \|\|= y`, `x &&= y` | ✅ reset (短絡 assign も同)  |
| `UpdateExpr { arg: Ident(x), op: PlusPlus/MinusMinus }` | `x++`, `x--` | ❌ 不変 (TS では number narrow 維持、INV-Step3-1 の scope 外だが保守的に ❌) |
| `ForOf { left: Pat::Ident(x), ... }` | `for (x of arr)` | ✅ reset (RHS は element 型) |

#### 3. RHS 型判定の必要性 (精度 trade-off)

TS は RHS の型から narrow を再計算する:

- `x = null` → narrow が `null` に置換 → reset 扱い (Rust shadow-let と不整合)
- `x = 5` (x の元 union に `5` が含まれる) → narrow 維持 → reset 非該当
- `x = y` where y: `number | null` → narrow 解除 → reset 扱い

**初期実装の policy (D-1 scope)**: 保守的に「**shadow 済 ident への任意の `AssignExpr`/`UpdateExpr` = reset 扱い**」で surface。Reason:

1. false negative (reset を見逃し) = silent compile error (Tier 1 違反)、絶対禁止
2. false positive (reset 扱いで Unsupported) = 変換失敗だが silent bug なし (Tier 3)、許容
3. RHS 型ベースの精密判定は I-144 の CFG analyzer で実施 (narrow 再計算の一部)

#### 4. scan 発動条件

scan は `pick_strategy` の `ShadowLet` strategy が選択された場合のみ発動。他 strategy (`Identity`, `BlockedByI050`) は shadow-let を emit しないため narrowing-reset 問題は発生しない。

### D-1 の interim surface と I-144 structural fix の関係

- **D-1 (I-142 Step 3)**: 上記の保守的 scan で `UnsupportedSyntaxError("nullish-assign with narrowing-reset (I-144)")` を surface。silent compile error を Tier 3 (explicit unsupported) に格上げ
- **I-144 (structural)**: CFG analyzer で RHS 型ベースの精密判定を行い、reset 検出時は shadow-let を使わず `let mut x: Option<T> = x;` + `x.get_or_insert_with(...);` 経路に切替。本調査の case 01/02/04/06d は reset 経路、case 03/05/06c は shadow-let 維持経路

### I-142 Step 3 D-1 実装での必要ヘルパー

- `shadow_ident_is_reassigned_in_block(block: &[Stmt], shadow_ident: &str) -> bool`
  - block 内の全 Stmt を再帰的に scan
  - Closure/fn body は早期 skip
  - `AssignExpr`/`UpdateExpr` 検出時に LHS が `shadow_ident` と一致するか確認
  - for-of / for-in の pattern binding も scan

### 未解決 / 保留 (本 INV scope 外)

- `UpdateExpr` (`x++`) の扱いは I-144 で要精密化。D-1 では保守的に reset 扱い
- 三項演算子 / switch case fall-through の scan 要件は D-2 (RHS 次元 matrix) と重複 → D-2 で enumerate

## D-1 実装への影響

- scan は `try_convert_nullish_assign_stmt` に追加し、`ShadowLet` strategy 分岐の **前** に実行 (scan 結果が true なら `UnsupportedSyntaxError` を返し、shadow-let emit をスキップ)
- scan の入力は「現在の enclosing block の残り Stmt 列」。Transformer の現行 API では block 全体を渡す必要があり、`try_convert_nullish_assign_stmt` の signature に追加引数 `remaining_stmts: &[ast::Stmt]` を追加する必要がある
  - あるいは block 全体を処理する `convert_block_stmts` layer で pre-scan してマーキングする方式も可。設計判断は D-1 実装時に確定
