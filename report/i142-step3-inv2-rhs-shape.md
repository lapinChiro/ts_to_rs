# INV-Step3-2: `x ??= <RHS>` の RHS shape 調査

- **日付**: 2026-04-15
- **対象**: I-142 Step 3 D-2 (Problem Space matrix の RHS 次元 enumerate) 実装前の AST 境界確定
- **調査手段**: SWC AST 定義確認 (swc_ecma_ast-21.0.0 `expr.rs`, `operators.rs`) + TS 仕様

## 調査目的

D-2 では「3 LHS × 2 Context × 14 RHS shape = 84 cell」と PRD 記載したが、実際に parser に入り得る RHS shape を確定する必要あり。

## SWC AST 構造

```rust
pub struct AssignExpr {
    pub span: Span,
    pub op: AssignOp,         // NullishAssign for `??=`
    pub left: AssignTarget,   // Ident / Member / SuperProp (本 PRD scope は Ident)
    pub right: Box<Expr>,     // 全 Expr variant を受理
}
```

`right: Box<Expr>` は syntactic には **全 Expr variant** が parser を通過可能。SWC は文法的な制約のみ適用し、TS 意味論違反 (例: generator 外の `yield`) は後段で検出される。

## TS 仕様レベルでの accept 判定

| RHS shape | TS 仕様 | 備考 |
|-----------|---------|------|
| `Lit` (Number/Bool/String/Null/BigInt/Regex) | ✅ | 最頻出 |
| `Ident` | ✅ | 最頻出 |
| `This` / `Super` | ✅ | this / super (Super は SuperProp 経由) |
| `Array` (`[a, b]`) | ✅ | - |
| `Object` (`{ a: 1 }`) | ✅ | 要括弧 `x ??= ({a:1})` (block 曖昧性回避) |
| `Fn` (function expression) | ✅ | - |
| `Arrow` (arrow function) | ✅ | - |
| `Class` (class expression) | ✅ | - |
| `Call` (function call) | ✅ | 最頻出 |
| `New` | ✅ | `new T(...)` |
| `TaggedTpl` (tagged template) | ✅ | - |
| `Member` (`a.b` / `a[b]`) | ✅ | - |
| `SuperProp` (`super.x`) | ✅ | 稀 |
| `Unary` (`!x`, `typeof x`, `-x`) | ✅ | - |
| `Update` (`x++`, `--x`) | ✅ | side effect あり |
| `Bin` (二項演算) | ✅ | NC chain `y ?? def` 等を含む |
| `Assign` (nested `x = y`) | ✅ | `x ??= (y = z)` |
| `Seq` (`,` 演算子) | ✅ | 要括弧 `x ??= (a, b)` |
| `Cond` (三項 `a ? b : c`) | ✅ | - |
| `Tpl` (template literal `` `${x}` ``) | ✅ | - |
| `TaggedTpl` | ✅ | - |
| `Paren` (括弧) | ✅ | transparent wrapper |
| `Await` | ✅ only in async | - |
| `Yield` | ✅ only in generator | `x ??= yield v` は assignment precedence で parse 可能 |
| `OptChain` (`a?.b`) | ✅ | - |
| `TsAs` (`x as T`) | ✅ TS only | - |
| `TsTypeAssertion` (`<T>x`) | ✅ TS only (JSX と曖昧) | - |
| `TsNonNull` (`x!`) | ✅ TS only | - |
| `TsSatisfies` (`x satisfies T`) | ✅ TS 4.9+ | - |
| `TsConstAssertion` (`x as const`) | ✅ TS only | - |
| `TsInstantiation` (`f<T>`) | ✅ TS 4.7+ | - |
| `MetaProp` (`new.target` / `import.meta`) | ✅ 稀 | - |
| `Invalid` | ❌ parse error marker | production では発生しない |
| `JSXElement` 等 | ❌ TSX only | 本 PRD scope 外 |
| **`Spread` (`...x`)** | ❌ syntax error | expression として単独不可 |
| **Throw expression (`throw err`)** | ❌ stage 2 | tsc が reject、scope 外 |

## 正規化 (Cell 設計)

全 variant 列挙では matrix が冗長。semantic 分岐に影響する高レベル次元で **4 クラス** に正規化:

### クラス A: Side-effect-free Copy literal

- `NumberLit`, `BoolLit`, `BigIntLit`, `Null` (= Option の None emission)
- ideal emission: `.unwrap_or(lit)` (eager arg, closure 不要)
- 分類の意義: Rust の `Option::unwrap_or` は eager 引数を取り、その値は `Copy + cheap`。これらリテラルは該当

### クラス B: Side-effect-free non-Copy literal

- `StringLit` (`.to_string()` 生成)、RegexLit (Regex::new)、static `Tpl` (interpolation なし)
- ideal emission: `.unwrap_or_else(|| "...".to_string())` (closure wrap で eager .to_string() 回避)

### クラス C: Side-effect / non-idempotent expression

- `Ident`, `Call`, `New`, `Member`, `Update`, `Unary`, `Bin`, `Cond`, `Assign` (nested), `Await`, `Yield`, `Array`, `Object`, `Fn`, `Arrow`, `Class`, `OptChain`, `SuperProp`, `This`, `Super`, `TaggedTpl`, `Tpl` (with interpolation), `Seq`
- ideal emission: `.unwrap_or_else(|| <rhs>)` (lazy eval、LHS が Some のとき RHS を評価しない TS 意味論を保持)

### クラス D: Transparent wrapper (peek-through)

- `Paren`, `TsAs`, `TsTypeAssertion`, `TsSatisfies`, `TsConstAssertion`, `TsInstantiation`, `TsNonNull`
- inner Expr に再帰し、上記 A/B/C で再判定
- I-143-a (`(x as T) ?? d`) と同一問題空間 — NC / ??= 両方で TsAs peek-through 必要

## D-2 Matrix の確定 cell 数

- LHS 型: {Option<T>, non-null T, Any, unresolved} = **4**
- Context: {Stmt, Expr} = **2**
- RHS class: {A, B, C, D} = **4**
- 合計: 4 × 2 × 4 = **32 cell** (個別 variant まで展開する代わりに class 代表でテスト)

各 class は **parameterized test** で代表 variant 2-3 個を測定。

### NA justification (matrix の非該当 cell)

| Cell | 判定 | 理由 |
|------|------|------|
| Any × any class × any context | ⏸ I-050 blocked | pick_strategy で BlockedByI050 surface、class 無関係 |
| unresolved × any × any | ✗→✓ UnsupportedSyntaxError | 型が未解決のため strategy 選択不能、class 無関係 |
| non-null T × class A/B/C/D × Stmt | ✓ 空 emit | Identity strategy は stmt で emit なし、class 無関係 |
| non-null T × class A/B/C/D × Expr | ✓ Identity emit | `target` 単独 (Copy) / `target.clone()` (!Copy)、RHS 未 emit → class 無関係 (D-3 で RHS convert skip) |

**実質 ideal 出力定義が必要な cell = Option<T> × 2 Context × 4 Class = 8 cell**。このうちいくつかは現行実装で既に正しく、いくつかは未網羅。

### 優先確認対象

D-2 実装で各 Option<T> × Context × Class cell の現状確認:

| Class | Stmt | Expr |
|-------|------|------|
| A (Copy lit) | `let x = x.unwrap_or(0);` (Cell #1/#2 既存) | `*x.get_or_insert_with(\|\| 0.0)` (Cell #7 既存) |
| B (non-Copy lit) | `let x = x.unwrap_or_else(\|\| "d".to_string());` (Cell #3 既存) | `x.get_or_insert_with(\|\| "d".to_string()).clone()` (Cell #8 既存) |
| C (side-effect expr) | `let x = x.unwrap_or_else(\|\| y);` (Cell #2 既存) | **未確認**: `x.get_or_insert_with(\|\| y)` の shape |
| D (TsAs peek-through) | **未確認** | **未確認** — I-143-a 並の silent bug の可能性 |

特に **Class D (transparent wrapper) × Option<T>** は現行実装で peek-through されているか要確認 (D-2 scope)。未対応なら D-1 と同等の silent bug の可能性あり。

## Seq (comma operator) RHS の扱い

`x ??= (a, b)` は現行 `convert_expr` の Seq arm で `UnsupportedSyntaxError` surface。

- 本 PRD (I-142 Step 3) での選択: **unsupported surface を lock-in 化**。ideal 変換 (block expr `{ a; b }`) は I-114 (SeqExpr 一般変換) に委譲
- D-2 matrix: Class C の `Seq` variant → `UnsupportedSyntaxError` assertion を lock-in test 化
- I-114 完了時に本 lock-in test を ideal 出力 assertion に書き換え

## Yield / Await RHS の扱い

- `Await` は async 関数内で ok、同期関数内では `syntax error` (tsc reject)
- `Yield` は generator 内で ok、非 generator では invalid
- Hono bench には generator 関数出現なし (要 grep 確認) → `Yield` RHS は NA justify + minimal lock-in test
- `Await` は async 関数内の `??=` RHS として現実的 → Class C に含めて ideal emission (`.unwrap_or_else(|| <await>)`) を測定

## Throw / Spread RHS

- TS syntax error で parser が reject → 本 PRD の matrix 対象外
- D-2 matrix で **NA justify** として明示記載 (問題空間から除外した理由を残す)

## INV-Step3-2 結論

D-2 matrix の確定 cell 数: **32 cell (4 LHS × 2 Context × 4 RHS class)** + NA justification = 完全 enumerate。

実装優先度:
1. Class D (transparent wrapper peek-through) が現行実装で正しく動くか確認 — 🔴 silent bug 懸念
2. Seq / Yield を UnsupportedSyntaxError で surface 化 — lock-in test
3. Class A/B の現行 cell を D-2 matrix 上で parameterized 化 — regression 強化
4. Class C の代表 variant (Ident / Call / Bin / Cond / Await) を網羅 — parameterized 化
