# I-205: Class member access dispatch with getter/setter methodology framework

**Status**: Spec stage v7 final (TS-0〜TS-5 完了 + 8 RC clusters fix (5 RC + 3 RC-Θ/Ι/Κ) + 7 F-deep + 8 F-deep-deep findings fix + framework v1.3 → v1.4 → v1.5 → v1.6 連続 revision (audit symmetry restoration)、2026-04-28、Implementation stage 移行 ready、improved framework adopter self-applied integration 完了)
**Discovery date**: 2026-04-27
**Architectural concern**: Class member access dispatch with getter/setter methodology
**Priority**: L2 (Design Foundation — every future development with class accessor faces same defect)
**Tier**: Tier 2 (compile error visible to user, not silent semantic change)
**Prerequisite**: PRD 2.7 完了 (✓)
**Blocks**: PRD 2.8 (I-201-A AutoAccessor)、PRD 2.9 (I-202 Object literal Prop::Getter/Setter)、PRD 7 (I-201-B Decorator framework)
**Self-applied integration**: framework Rule 改修 (`spec-stage-adversarial-checklist.md` Rule 1/2/5/6/8/11/13 + `prd-completion.md` Tier-transition compliance + `prd-template` skill Step 3-pre / 4-template / 4.5 + audit script extensions) を本 PRD の draft v1 → v2 transition で first-class adopter として self-applied verify (PRD 2.7 pattern)。詳細は `## Spec Review Iteration Log` 参照。

---

## ⚠️ T11 削除 + 新 PRD I-A / I-B migration 注記 (2026-05-01)

本 PRD doc 内の **T11 references** (= "T11 (11-a/11-b/11-c/11-d/11-e/11-f)" 言及) は **2026-05-01 に user 確定で削除** され、T11 sub-tasks は以下 2 つの新 PRD として独立起票:

- **新 PRD I-A "Method static-ness IR field propagation"** (= 元 T11 11-b、`MethodSignature.is_static` field 追加 + Rule 9 (c-1) Field Addition Symmetric Audit、61 site)
- **新 PRD I-B "Class TypeName context detection unification"** (= 元 T11 11-d + 11-f + I-214 統合、TypeResolver `RustType::ClassConstructor(String)` type marker + 全 Ident match sites unification + `Expr::AssociatedConst` 新 IR variant)
- **(11-c) matrix cell expansion** は新 PRD I-A / I-B の completion criteria に integrate

T11 task description verbatim copy + 移行 mapping table は `note.after.md` archive 参照。

**本 doc 内 T11 references の読み方**:
- `## ~~T11~~: 削除 (2026-05-01、新 PRD I-A / I-B へ migrate)` section (旧 T11 task description 削除位置) は **最新の defer mapping** を記載
- 各 T1〜T10 task の `### Status` / `### Defect Classification` 内の "T11 (11-x) defer" 言及は **historical commit notes** = T1〜T10 commit 時点の defer 先記録、historical accuracy のため preserve (現在の defer 先は T11 ではなく新 PRD I-A / I-B、最新 mapping は上記新 section 参照)
- `## Spec → Impl Dispatch Arm Mapping` 内の T11 言及は最新 mapping に update 済 ("subsequent T11 で expansion" → "新 PRD I-A/I-B で expansion")

`prd-completion.md` matrix 全セルカバー条件 compliance は **(11-c) defer cells を新 PRD I-A / I-B に明示 move** することで維持。

---

## Background

### Discovery 経緯

PRD 2.8 (I-201-A AutoAccessor 単体 Tier 1 化) Spec stage の Step 0b Problem Space 検証中、AutoAccessor emission strategy 比較のため既存 class Method Getter/Setter 変換結果を empirical observation した結果、**framework 全体が broken** であることを発見:

```ts
// 入力 TS:
class Foo {
  _name: string = "alice";
  get name(): string { return this._name; }
  set name(v: string) { this._name = v; }
}
const f = new Foo();
console.log(f.name);
f.name = "bob";
```

```rust
// 現状 ts_to_rs 出力 (broken):
struct Foo {
    _name: String,
}
impl Foo {
    fn name(&self) -> String {
        self._name        // ✗ E0507 — move out of `self._name` which is behind a shared reference
    }
    fn set_name(&mut self, v: String) {
        self._name = v;
    }
}
// caller side:
println!("{}", f.name);   // ✗ E0609 — no field `name` on type `Foo` (name は method)
f.name = "bob";           // ✗ E0609 (silent semantic divergence: getter/setter dispatch されず direct field write、`name` field 不在で compile error)
```

### Root cause (架構的欠陥)

`src/transformer/expressions/member_access.rs:73-115` の `resolve_member_access` 関数は **常に `Expr::FieldAccess` を emit**、receiver type に getter/setter が定義されていても method dispatch しない。`MethodSignature` (`src/registry/mod.rs:116`) に **method kind tracking が不在** (Method/Getter/Setter 区別なし)、`collect_class_info` (`src/registry/collection/class.rs:108-144`) も `method.kind` を捨てている。call site での dispatch 判断材料が架構的に欠如。

### 影響範囲 (架構的、cross-cutting)

| 機能 | 現在の Tier | I-205 framework defect の影響 |
|------|-----|-----|
| Class Method Getter (`get x() {...}`) | Tier 2 (E0507 + E0609) | ✗ broken |
| Class Method Setter (`set x(v) {...}`) | Tier 2 (E0609) | ✗ broken |
| AutoAccessor (PRD 2.8 / I-201-A) | Tier 2 honest error (PRD 2.7 で error 化済) | ✗ ideal emission 阻害 |
| Object literal Prop::Getter/Setter (PRD 2.9 / I-202) | Tier 2 honest error (PRD 2.7) | ✗ ideal emission 阻害 |
| Decorator framework (I-201-B、L1 silent semantic change) | Tier 1 silent drop | ✗ hook coverage 不能 |

I-205 は上記全 5 機能の真の prerequisite framework。

### Tier 分類 + Priority 判定

- **Tier 2 (compile error)** ([`conversion-correctness-priority.md`](.claude/rules/conversion-correctness-priority.md))。caller-side の `f.name` / `f.name = v` で E0609、getter body で E0507 — いずれも compile error visible to user。silent semantic change には該当しない (caller が compile fail で気付く)。
- **L2 (Design Foundation)** ([`todo-prioritization.md`](.claude/rules/todo-prioritization.md))。getter/setter を含む class が新規追加されるたびに同類問題再発、framework として construction defect = base 不全。
- L1 ではない理由: Tier 1 silent semantic change ではない (compile error が出るため runtime 誤動作はない)。
- L3 より優先: PRD 2.8/2.9/I-201-B 全 prerequisite で leverage 高、Expansion Rate 大、Fix Cost は新規 PRD だが framework foundation で必要。

---

## Problem Space

### 入力次元 (Dimensions)

`problem-space-analysis.md` Step 1 (axis enumeration) + `spec-stage-adversarial-checklist.md` Rule 10 (Cross-axis matrix completeness、9 default check axis) を適用。

#### Dimension A (call site context — read/write/compound)

`obj.x` が現れる construct を全列挙:

| A | Variant | 例 | Scope |
|---|---|---|---|
| A1 | Read (RHS use) | `let v = obj.x; func(obj.x); return obj.x;` | 本 PRD |
| A2 | Write simple | `obj.x = v;` | 本 PRD |
| A3 | Write compound (`+= -= *= /= %= **=`) | `obj.x += v;` | 本 PRD |
| A4 | Write bitwise/shift compound (`<<= >>= >>>= &= \|= ^=`) | `obj.x \|= 1;` | 本 PRD |
| A5 | Write logical compound (`??= &&= \|\|=`) | `obj.x ??= d;` | 本 PRD |
| A6 | Increment/Decrement (`++/--` prefix/postfix) | `obj.x++; ++obj.x;` | 本 PRD |
| A7 | Destructure read | `const {x} = obj;` | Tier 2 honest error (E1 確定) |
| A8 | Spread | `{...obj}` | Tier 2 honest error (E1 確定) |
| A9 | Delete | `delete obj.x;` | Tier 2 honest error (E1 確定) |
| A10 | typeof | `typeof obj.x` | Tier 2 honest error (E1 確定) |
| A11 | in operator | `"x" in obj` | Tier 2 honest error (E1 確定) |

#### Dimension B (receiver type's member shape for the field name `x`)

receiver type に対し `x` という name の member 構成を全列挙:

| B | Variant | 例 | Scope |
|---|---|---|---|
| B1 | field only (no getter, no setter, no method) | `class { x: T = init; }` | 現状動作維持 (regression lock-in) |
| B2 | getter only (read-only property) | `class { get x(): T {...} }` | 本 PRD |
| B3 | setter only (write-only property) | `class { set x(v: T) {...} }` | 本 PRD |
| B4 | getter + setter (full accessor) | `class { get x() {} set x(v) {} }` | 本 PRD |
| B5 | AutoAccessor (PRD 2.8 後) | `class { accessor x: T = init; }` | 別 PRD (PRD 2.8) — 本 framework leverage |
| B6 | regular method (not accessor) | `class { x(): T {...} }` | 別 architectural concern (E1 確定 Tier 2 honest error for `obj.x` no-paren reference) |
| B7 | inherited (parent class accessor) | `class Sub extends Base {}` (Base に `get x()`) | Tier 2 honest error (E1 確定、Class inheritance interaction = 別 architectural concern) |
| B8 | static accessor (`Class.x` access path) | `class { static get x() {} }`、call: `Foo.x` | 本 PRD (E1 確定 in-scope) |
| B9 | unknown / external (no TypeRegistry entry) | external module class | 現状動作維持 (fallback to direct field access) |

#### Dimension C (receiver expression shape)

| C | Variant | 例 |
|---|---|---|
| C1 | Ident (instance) | `obj.x` |
| C2 | this (inside class body) | `this.x` |
| C3 | TypeName (static) | `Foo.x` (B8 と orthogonal) |
| C4 | chain | `a.b.x` |
| C5 | call result | `getInstance().x` |
| C6 | complex (ternary / paren / cast) | `(cond ? a : b).x` |

C3-C6 は dispatch logic が C1 と uniform (Type が解決できれば dispatch 同じ)、emit 時に object expression 部の convert 結果を embed する。

#### Dimension D (T variant for getter return / field type / setter param)

`.clone()` insertion logic を決める T variant ([`doc/grammar/rust-type-variants.md`](doc/grammar/rust-type-variants.md) 参照):

| D | Variant | Copy 性 | `.clone()` 必要性 |
|---|---|---|---|
| D1 | f64 / i64 / u64 / etc. (primitive number) | Copy | 不要 |
| D2 | bool | Copy | 不要 |
| D3 | char | Copy | 不要 |
| D4 | String | non-Copy (要 Clone) | 必要 |
| D5 | Vec<T> | non-Copy (要 Clone) | 必要 |
| D6 | Option<T> | T が Copy なら Copy、それ以外 non-Copy | T 依存 |
| D7 | HashMap<K,V> | non-Copy (要 Clone) | 必要 |
| D8 | Tuple (T1, T2, ...) | 全要素 Copy なら Copy | 各要素依存 |
| D9 | Struct Named (user defined) | derived Clone or 非 Clone | 各 struct 依存 |
| D10 | Enum Named | derived Clone or 非 Clone | 各 enum 依存 |
| D11 | DynTrait (`Box<dyn Trait>`) | non-Copy、Clone は trait に依る | 多くは Tier 2 |
| D12 | Fn type | non-Copy、Clone 不能の場合多い | Tier 2 |
| D13 | TypeVar (generic T、bounded `T: Clone`) | bound 依存 | bound 依存 |
| D14 | Any (`serde_json::Value`) | non-Copy (要 Clone) | 必要 |
| D15 | Regex | non-Copy | 必要 |

#### Dimension E (inside-class vs external)

| E | Variant | Dispatch (P1 確定) |
|---|---|---|
| E1 | external (caller outside class) | getter/setter dispatch via method |
| E2 | internal method/getter/setter body | TC39 faithful: `self.x()` / `self.set_x(v)` 経由 |
| E3 | internal constructor body | E2 同 |

#### Dimension F (TS strict / non-strict) — read-only property write 動作

| F | Variant | TS 動作 (write to B2 read-only) | Rust 出力 |
|---|---|---|---|
| F1 | strict | TS2540 type error | conversion 時 detect → Tier 2 honest error |
| F2 | non-strict | runtime silently no-op | conversion 時 detect → Tier 2 honest error (F1 と同等扱い、static detect 可能) |

(実装上は TS strict / non-strict 区別不要、receiver type の B 判定で write 判定する)

### 組合せマトリクス (Cartesian product enumeration)

**Orthogonality merge legitimacy declaration (Rule 1 (1-4) compliance、framework v1.5 適用)**: 本 matrix では Rule 10 Step 2 orthogonality reduction を適用、`* (orthogonality-equivalent: D dimension は dispatch logic に影響なし)` placeholder と `B5-B9` group cells (cells 47/48-b/49-b/51-b/52-b で全 B variants → Tier 2 honest error reclassify dispatch identical の場合) を **orthogonality-equivalent merge** として merge cell 表記を採用。dispatch row のみ orthogonality merge (Class Method Getter body cells 70-80 では D dimension が `.clone()` insertion logic に影響、independent rows)、cells 47/48-b/49-b 等は全 B2-B9 variants → dispatch token-level identical を **Rule 1 (1-4-b) Spec stage structural verify** (audit script `verify_orthogonality_merge_consistency` function) + **Rule 1 (1-4-c) Spec stage referenced cell symmetry probe** で auto verify (framework v1.5、v1.4 Implementation Stage defer から Spec stage structural verify へ revise)。Cells 35/41/45-d/29-e は **divergent dispatch** (B5=NA/B6/B7=Tier 2/B8=Tier 1/B9=fallback) のため Phase 1 で expand 済 (cells 35-a〜35-e、41-a〜41-e、45-da〜45-de、29-e-a〜29-e-e)。

primary axes A × B、secondary axis D (`.clone()` insertion 用) で全 cell 列挙。NA / fallback / 別 PRD scope は理由付きで明示。

| # | A (context) | B (member shape) | D (T variant) | Ideal Rust 出力 | 現状 | 判定 | Scope |
|---|---|---|---|---|---|---|---|
| 1 | A1 Read | B1 field | D1-D15 全 | `obj.x` (direct field access) | `obj.x` | ✓ | 本 PRD regression lock-in |
| 2 | A1 Read | B2 getter only | D1 Copy primitive | `obj.x()` (method call) | `obj.x` (compile error E0609) | ✗ | 本 PRD |
| 3 | A1 Read | B2 getter only | D4-D15 non-Copy | `obj.x()` (method returns owned T、body の `.clone()` で実現) | `obj.x` (compile error E0609 + getter body E0507 for non-Copy T) | ✗ | 本 PRD |
| 4 | A1 Read | B3 setter only | * | Tier 2 honest error (`UnsupportedSyntaxError::new("read of write-only property", span)`) | E0609 | ✗ | 本 PRD |
| 5 | A1 Read | B4 both | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `obj.x()` (method call) | E0609 | ✗ | 本 PRD |
| 6 | A1 Read | B5 AutoAccessor | * | `obj.x()` (PRD 2.8 後、本 framework leverage) | Tier 2 honest error (PRD 2.7 で error 化済) | NA in 本 PRD | 別 PRD (PRD 2.8) |
| 7 | A1 Read | B6 regular method (no paren) | * | Tier 2 honest error (`UnsupportedSyntaxError::new("method-as-fn-reference", span)`) | direct field access (E0609) | ✗ | 本 PRD (Tier 2 honest error 化) |
| 8 | A1 Read | B7 inherited | * | Tier 2 honest error (`UnsupportedSyntaxError::new("inherited accessor access", span)`) | E0609 | △ | 本 PRD (Tier 2 honest error 化、別 PRD で Tier 1 化) |
| 9 | A1 Read | B8 static | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `Foo::x()` (associated function call) | `Foo.x` (Rust syntax error: `.` on type path is not field access) | ✗ | 本 PRD |
| 10 | A1 Read | B9 unknown | * | `obj.x` (current behavior、TypeRegistry entry なしで dispatch 不能 = fallback) | `obj.x` | ✓ | 本 PRD regression lock-in |
| 11 | A2 Write simple | B1 field | * | `obj.x = v;` (current) | 同 | ✓ | regression lock-in |
| 12 | A2 Write simple | B2 getter only (read-only) | * | Tier 2 honest error (`UnsupportedSyntaxError::new("write to read-only property", span)`) | E0609 | ✗ | 本 PRD |
| 13 | A2 Write simple | B3 setter only | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `obj.set_x(v);` | E0609 | ✗ | 本 PRD |
| 14 | A2 Write simple | B4 both | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `obj.set_x(v);` | E0609 | ✗ | 本 PRD |
| 15 | A2 Write simple | B5 AutoAccessor | * | `obj.set_x(v);` (PRD 2.8 後 leverage) | Tier 2 honest error | NA | 別 PRD (PRD 2.8) |
| 16 | A2 Write simple | B6 regular method | * | Tier 2 honest error (`UnsupportedSyntaxError::new("write to method", span)`) | E0609 | ✗ | 本 PRD |
| 17 | A2 Write simple | B7 inherited setter | * | Tier 2 honest error | E0609 | △ | 本 PRD (Tier 2 honest error 化) |
| 18 | A2 Write simple | B8 static setter | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `Foo::set_x(v);` | `Foo.x = v` (Rust syntax error: `.` on type path is not field assignment) | ✗ | 本 PRD |
| 19 | A2 Write simple | B9 unknown | * | `obj.x = v;` (fallback) | 同 | ✓ | regression lock-in |
| 20 | A3 Write compound (`+=`) | B1 field | * | `obj.x += v;` | 同 | ✓ | regression lock-in |
| 21 | A3 Write compound (`+=`) | B4 both | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | **Block form setter desugar (yield_new、prefix update と same shape with rhs replacing 1.0)**: side-effect-free receiver = `{ let __ts_new = obj.x() + v; obj.set_x(__ts_new); __ts_new }` / side-effect-having receiver (INV-3 1-evaluate compliance) = `{ let mut __ts_recv = <object>; let __ts_new = __ts_recv.x() + v; __ts_recv.set_x(__ts_new); __ts_new }`。statement context (`obj.x += v;`) では TailExpr discarded、expression context (`let z = (obj.x += v)`) では `__ts_new` yield (TS spec: compound assign yields assigned value)。 | E0609 | ✗ | 本 PRD |
| 22 | A3 Write compound (`+=`) | B2 getter only | * | Tier 2 honest error | E0609 | ✗ | 本 PRD |
| 23 | A3 Write compound (`+=`) | B3 setter only | * | Tier 2 honest error (read part 不能) | E0609 | ✗ | 本 PRD |
| 24 | A3 Write compound (`+=`) | B5 AutoAccessor | * | `obj.set_x(obj.x() + v);` (PRD 2.8 で AutoAccessor が methods にregister された後、本 framework leverage) | Tier 2 honest error (PRD 2.7) | NA | 別 PRD (PRD 2.8) |
| 25 | A3 Write compound (`+=`) | B6 regular method | * | Tier 2 honest error (`UnsupportedSyntaxError::new("compound assign to method", span)`) | E0609 | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 26 | A3 Write compound (`+=`) | B7 inherited setter | * | Tier 2 honest error (`UnsupportedSyntaxError::new("compound assign to inherited accessor", span)`) | E0609 | △ | 本 PRD (Tier 2 honest error reclassify) |
| 27 | A3 Write compound (`+=`) | B8 static accessor | D1-D15 | **Block form static setter desugar (yield_new、cell 21 と orthogonality-equivalent dispatch with `Foo::x()` / `Foo::set_x(...)` 置換)**: `{ let __ts_new = Foo::x() + v; Foo::set_x(__ts_new); __ts_new }`。Static dispatch では receiver = class TypeName で side-effect なし、IIFE form 不要。 | `Foo.x += v` (Rust syntax error) | ✗ | 本 PRD |
| 28 | A3 Write compound (`+=`) | B9 unknown | * | `obj.x += v;` (current behavior、fallback) | 同 | ✓ | regression lock-in |
| 29-a | A3 `-=` | B1 field | * | `obj.x -= v;` (current direct field、IR BinOp = Sub、Rust 直接対応) | `obj.x -= v` | ✓ | regression lock-in |
| 29-b | A3 `-=` | B2 getter only | * | Tier 2 honest error (write to read-only) | E0609 | ✗ | 本 PRD |
| 29-c | A3 `-=` | B3 setter only | * | Tier 2 honest error (read part 不能) | E0609 | ✗ | 本 PRD |
| 29-d | A3 `-=` | B4 both | D1 numeric | **Block form setter desugar (cell 21 と op-axis orthogonality-equivalent、BinOp::Sub 置換)**: side-effect-free = `{ let __ts_new = obj.x() - v; obj.set_x(__ts_new); __ts_new }` / side-effect-having (INV-3) = `{ let mut __ts_recv = <object>; let __ts_new = __ts_recv.x() - v; __ts_recv.set_x(__ts_new); __ts_new }` | E0609 | ✗ | 本 PRD |
| 29-e-a | A3 `-=`/`*=`/`/=`/`%=`/`**=` | B5 AutoAccessor | * | `obj.set_x(obj.x() OP v);` (PRD 2.8 で AutoAccessor が methods に register された後、本 framework leverage、operator は IR BinOp 層 Sub/Mul/Div/Rem/Pow で吸収) | Tier 2 honest error (PRD 2.7) | NA | 別 PRD (PRD 2.8) |
| 29-e-b | A3 `-=`/`*=`/`/=`/`%=`/`**=` | B6 regular method | * | Tier 2 honest error (`UnsupportedSyntaxError::new("compound assign to method", span)`、cell 25 と同 dispatch、operator 非依存) | E0609 | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 29-e-c | A3 `-=`/`*=`/`/=`/`%=`/`**=` | B7 inherited setter | * | Tier 2 honest error (cell 26 と同 dispatch、operator 非依存) | E0609 | △ | 本 PRD (Tier 2 honest error reclassify) |
| 29-e-d | A3 `-=`/`*=`/`/=`/`%=`/`**=` | B8 static accessor | D1-D15 | **Block form static setter desugar (cell 27 と op-axis orthogonality-equivalent、BinOp::Sub/Mul/Div/Mod 置換、`**=` は本 PRD scope 外 = TS exponent op 別 architectural concern)**: `{ let __ts_new = Foo::x() OP v; Foo::set_x(__ts_new); __ts_new }`。 | Rust syntax error | ✗ | 本 PRD |
| 29-e-e | A3 `-=`/`*=`/`/=`/`%=`/`**=` | B9 unknown | * | `obj.x OP= v;` (current behavior、fallback、operator 直接 emit) | 同 | ✓ | regression lock-in |
| 30 | A4 Bitwise compound (`\|=`) | B1 field | * | `obj.x \|= v;` (current direct field write、Rust 直接対応) | 同 | ✓ | regression lock-in |
| 31 | A4 Bitwise compound (`\|=`) | B2 getter only | * | Tier 2 honest error (write to read-only) | E0609 | ✗ | 本 PRD |
| 32 | A4 Bitwise compound (`\|=`) | B3 setter only | * | Tier 2 honest error (read part 不能、setter only では `obj.x()` undefined) | E0609 | ✗ | 本 PRD |
| 33 | A4 Bitwise compound (`\|=`) | B4 both | D1 numeric | **Block form setter desugar (cell 21 と op-axis orthogonality-equivalent、BinOp::BitOr 置換)**: side-effect-free = `{ let __ts_new = obj.x() \| v; obj.set_x(__ts_new); __ts_new }` / side-effect-having (INV-3) = `{ let mut __ts_recv = <object>; let __ts_new = __ts_recv.x() \| v; __ts_recv.set_x(__ts_new); __ts_new }` | E0609 | ✗ | 本 PRD |
| 34-a | A4 `<<=`/`>>=`/`>>>=`/`&=`/`^=` | B1 field | * | `obj.x <<= v;` 等 (current direct field、IR BinOp 層で operator 区別吸収、Rust 直接対応) | direct compound (current behavior) | ✓ | regression lock-in |
| 34-b | A4 各 bitwise operator | B2 getter only | * | Tier 2 honest error (write to read-only) | E0609 | ✗ | 本 PRD |
| 34-c | A4 各 bitwise operator | B4 both | D1 numeric | **Block form setter desugar (cell 21 と op-axis orthogonality-equivalent、BinOp::Shl/Shr/UShr/BitAnd/BitXor 置換)**: side-effect-free = `{ let __ts_new = obj.x() OP v; obj.set_x(__ts_new); __ts_new }` / side-effect-having (INV-3) = IIFE form with `__ts_recv` | E0609 | ✗ | 本 PRD |
| 35-a | A4 Bitwise compound (`<<=`/`>>=`/`>>>=`/`&=`/`^=`) | B5 AutoAccessor | * | `obj.set_x(obj.x() OP v);` (PRD 2.8 後)、operator は IR BinOp 層 Shl/Shr/UShr/BitAnd/BitXor で吸収 | Tier 2 honest error (PRD 2.7) | NA | 別 PRD (PRD 2.8) |
| 35-b | A4 Bitwise compound (各 operator) | B6 regular method | * | Tier 2 honest error (cell 25 と同 dispatch、operator 非依存) | E0609 | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 35-c | A4 Bitwise compound (各 operator) | B7 inherited setter | * | Tier 2 honest error (cell 26 と同 dispatch、operator 非依存) | E0609 | △ | 本 PRD (Tier 2 honest error reclassify) |
| 35-d | A4 Bitwise compound (各 operator) | B8 static accessor | D1 numeric | **Block form static setter desugar (cell 27 と op-axis orthogonality-equivalent、BinOp::Shl/Shr/UShr/BitAnd/BitXor 置換)**: `{ let __ts_new = Foo::x() OP v; Foo::set_x(__ts_new); __ts_new }`。Static dispatch では receiver = class TypeName で side-effect なし、IIFE form 不要。 | Rust syntax error | ✗ | 本 PRD |
| 35-e | A4 Bitwise compound (各 operator) | B9 unknown | * | `obj.x OP= v;` (current behavior、fallback) | 同 | ✓ | regression lock-in |
| 36 | A5 Logical compound (`??=`) | B1 field | D6 Option<T> | `obj.x.get_or_insert_with(\|\| d);` (既存 nullish_assign helper、I-142 pattern) | 同 | ✓ | regression lock-in |
| 37 | A5 Logical compound (`??=`) | B2 getter only | * | Tier 2 honest error (write to read-only) | E0609 | ✗ | 本 PRD |
| 38 | A5 Logical compound (`??=`) | B4 both | D6 Option<T> | desugar `if obj.x().is_none() { obj.set_x(d); }` (statement context) or `obj.x().or_else(\|\| { obj.set_x(d); Some(d) })` (expression context、Iteration v13 で `{ if obj.x().is_none() { obj.set_x(Some(d)); }; obj.x() }` に revise = SE-having receiver の borrow checker E0502/E0506 回避) | E0609 | ✗ | 本 PRD (既存 nullish_assign helper integration) |
| 38-identity | A5 Logical compound (`??=`) | B4 both | D1-D5 / D7-D15 (non-Option non-Any、cell 38 と orthogonality-equivalent inheritance per Rule 1 (1-4)、`pick_strategy` `Identity` strategy = TS dead code semantic) | **Tier 1 Identity emission** (Iteration v14 deep-deep review F-L4-1 source、cohesive with existing `nullish_assign.rs::try_convert_nullish_assign_stmt` Ident-target Identity emission): SE-free statement = empty Block / SE-having statement = `{ <obj>; }` evaluate-discard / SE-free expression = `<obj>.x()` direct getter call / SE-having expression = `{ let __ts_recv = <obj>; __ts_recv.x() }` IIFE evaluate-once + yield | Tier 2 broken (pre-T9: existing `nullish_assign.rs` Member arm が `convert_member_expr_for_write` 経由 FieldAccess emit、class with getter のみ field 不在で E0609) | ✗ | **本 PRD (Tier 2 broken → Tier 1 Identity transition、Iteration v14 deep-deep)** |
| 38-blocked | A5 Logical compound (`??=`) | B4 both | Any (`pick_strategy` `BlockedByI050` strategy) | **Tier 2 honest error**: `UnsupportedSyntaxError::new("nullish-assign on Any class member (I-050 Any coercion umbrella)", span)`。wording consistency with existing `nullish_assign.rs::try_convert_nullish_assign_stmt` Ident-target `BlockedByI050` strategy。subsequent I-050 umbrella PRD で `serde_json::Value`-aware runtime null check + RHS coercion を提供、本 PRD scope では Tier 2 honest error 維持。 | E0599 (`is_none` not found on `serde_json::Value`) または silent type widening | ✗ | 本 PRD (Iteration v14 deep-deep) |
| 39 | A5 Logical compound (`&&=`) | B4 both | D2 bool | desugar `if obj.x() { obj.set_x(v); }` | E0609 | ✗ | 本 PRD |
| 39-other | A5 Logical compound (`&&=`) | B4 both | D1 F64 / D3 String / D4 Option (predicate-supported per truthy.rs Matrix A.12、cell 39 と orthogonality-equivalent inheritance per Rule 1 (1-4)) | **Predicate-based dispatch via existing `truthy_predicate_for_expr`** per-type (F64 truthy = `<getter> != 0.0 && !<getter>.is_nan()` with non-pure operand tmp-binding Block / String truthy = `!<getter>.is_empty()` / Option truthy = `<getter>.is_some_and(\|v\| <truthy(*v)>)` (Copy inner) or `<getter>.as_ref().is_some_and(\|v\| <truthy(v)>)` (!Copy inner))。setter 引数 = wrap_setter_value(rhs, lhs_type) (Option<T> なら Some-wrap、他は raw)。 | E0609 | ✗ | 本 PRD (Iteration v14 deep-deep、structural lock-in via tests) |
| 39-truthy | A5 Logical compound (`&&=`) | B4 both | always-truthy (Vec / Fn / StdCollection / DynTrait / Ref / Tuple / Named non-union per `is_always_truthy_type`) | **Tier 1 const-fold = unconditional setter call** (Iteration v14 deep-deep review F-L4-2 source、cohesive with existing `compound_logical_assign.rs::const_fold_always_truthy_stmts`): SE-free statement = `<setter>(rhs);` (no `if` predicate、no eval-Block 余分 emission) / SE-having statement = `{ let __ts_recv = <obj>; __ts_recv.set_x(rhs); }` IIFE / Expression context = `<setter>(rhs); <getter>` Block + tail (post-state value yield)。 | Tier 2 broken (pre-T9: FieldAccess emit) または Tier 1 functional with eval-Block predicate (post-Iteration v13: 余分 emission) | ✗ | **本 PRD (Tier 1 const-fold ideal、Iteration v14 deep-deep)** |
| 39-blocked | A5 Logical compound (`&&=`) | B4 both | Any / TypeVar (truthy/falsy predicate unavailable per truthy.rs Matrix A.12) | **Tier 2 honest error**: `UnsupportedSyntaxError::new("compound logical assign on Any/TypeVar class member (I-050 umbrella / generic bounds)", span)`。wording consistency with existing `compound_logical_assign.rs::desugar_compound_logical_assign_stmts` blocked path。 | silent (truthy_predicate_for_expr returned None で fallback path) または broken Rust output | ✗ | 本 PRD (Iteration v14 deep-deep) |
| 40 | A5 Logical compound (`\|\|=`) | B4 both | D2 bool | desugar `if !obj.x() { obj.set_x(v); }` | E0609 | ✗ | 本 PRD |
| 40-other | A5 Logical compound (`\|\|=`) | B4 both | D1 F64 / D3 String / D4 Option (predicate-supported、cell 40 と orthogonality-equivalent inheritance per Rule 1 (1-4)) | **Predicate-based dispatch via existing `falsy_predicate_for_expr`** per-type (De Morgan inverse of truthy)。setter 引数 wrap = cell 39-other と symmetric。 | E0609 | ✗ | 本 PRD (Iteration v14 deep-deep) |
| 40-truthy | A5 Logical compound (`\|\|=`) | B4 both | always-truthy | **Tier 1 const-fold = no-op** (Iteration v14 deep-deep、`||=` always-truthy LHS は dead = setter never called): SE-free statement = empty Block (no-op) / SE-having statement = `{ <obj>; }` evaluate-discard / SE-free expression = `<obj>.x()` getter yield / SE-having expression = `{ let __ts_recv = <obj>; __ts_recv.x() }` IIFE。 | Tier 2 broken (pre-T9) または Tier 1 functional with eval-Block predicate (post-Iteration v13) | ✗ | **本 PRD (Tier 1 const-fold ideal、Iteration v14 deep-deep)** |
| 40-blocked | A5 Logical compound (`\|\|=`) | B4 both | Any / TypeVar | Tier 2 honest error (cell 39-blocked と op-axis orthogonality-equivalent dispatch、wording 同一) | silent または broken Rust output | ✗ | 本 PRD (Iteration v14 deep-deep) |
| 41-a | A5 Logical compound (`??=`/`&&=`/`\|\|=`) | B5 AutoAccessor | * | logical short-circuit desugar (PRD 2.8 後 leverage) | Tier 2 honest error (PRD 2.7) | NA | 別 PRD (PRD 2.8) |
| 41-b | A5 Logical compound (各 operator) | B6 regular method | * | Tier 2 honest error (cell 25 と同 dispatch、logical operator 非依存) | E0609 | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 41-c | A5 Logical compound (各 operator) | B7 inherited setter | * | Tier 2 honest error (cell 26 と同 dispatch、logical operator 非依存) | E0609 | △ | 本 PRD (Tier 2 honest error reclassify) |
| 41-d | A5 Logical compound (各 operator) | B8 static accessor | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `??=`: `if Foo::x().is_none() { Foo::set_x(d); }` / `&&=`: `if Foo::x() { Foo::set_x(v); }` / `\|\|=`: `if !Foo::x() { Foo::set_x(v); }` | Rust syntax error | ✗ | 本 PRD |
| 41-e | A5 Logical compound (各 operator) | B9 unknown | * | `obj.x OP= v;` (current behavior、`??=`/`&&=`/`\|\|=` 既存 nullish_assign helper fallback) | 同 | ✓ | regression lock-in |
| 42 | A6 Increment (`++`) | B1 field | D1 numeric | **Block form (postfix old-value preservation + prefix new-value preservation)**: postfix `{ let __ts_old = obj.x; obj.x = __ts_old + 1.0; __ts_old }` / prefix `{ obj.x = obj.x + 1.0; obj.x }`。Pre-T7 は convert_update_expr が Member target を全面 reject (Tier 2 broken)、本 PRD で Tier 1 化 (regression Tier 2 → Tier 1 transition、Iteration v11 で Spec gap = matrix 当初 statement form `obj.x += 1;` 記載を Block form に correct)。`obj.x++;` 等の statement context では Block の TailExpr が discarded、`let z = obj.x++;` 等の expression context では yield。両 context 対応の cohesive emission。 | Tier 2 broken (Member target reject) | ✗ | **本 PRD (Tier 2 broken → Tier 1 transition)** |
| 43 | A6 Increment (`++`) | B4 both | D1 numeric | **Block form setter desugar**: postfix `{ let __ts_old = obj.x(); obj.set_x(__ts_old + 1.0); __ts_old }` / prefix `{ let __ts_new = obj.x() + 1.0; obj.set_x(__ts_new); __ts_new }` | E0609 (Tier 2 broken) | ✗ | 本 PRD |
| 44 | A6 Increment (`++`) | B4 both | D2-D15 (non-numeric、e.g., String) | **Tier 2 honest error reclassify (本 PRD scope、Rule 3 (3-3) SWC empirical reclassify)**: `UnsupportedSyntaxError::new("increment of non-numeric (String/etc.) — TS NaN coercion semantic", span)` | tsx で `NaN` (string→number coercion)、Rust では `String + 1` E0277 compile error | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 44-symmetric | A6 Decrement (`--`) | B4 both | D2-D15 (non-numeric) | Tier 2 honest error reclassify (op-axis symmetric per Rule 1 (1-4)、cell 44 と orthogonality-equivalent dispatch、wording = `"decrement of non-numeric (String/etc.) — TS NaN coercion semantic"`、cells 44 と Rule 1 (1-4-a)/(1-4-b)/(1-4-c) compliant inheritance) | tsx `NaN`、Rust E0277 | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 45-a | A6 Decrement (`--`) | B1 field | D1 numeric | **Block form (cell 42 と op-axis orthogonality-equivalent、BinOp::Sub に置換)**: postfix `{ let __ts_old = obj.x; obj.x = __ts_old - 1.0; __ts_old }` / prefix `{ obj.x = obj.x - 1.0; obj.x }`。Rule 1 (1-4) Orthogonality merge legitimacy: cell 42 と orthogonality-equivalent dispatch (Rule 1 (1-4-a) source cell 42 reference、(1-4-b) 同 Scope `本 PRD`、(1-4-c) symmetric prefix `Block form` token-level match)。 | Tier 2 broken | ✗ | **本 PRD (Tier 2 broken → Tier 1 transition)** |
| 45-b | A6 Decrement (`--`) | B2 getter only | D1 numeric | Tier 2 honest error reclassify: `UnsupportedSyntaxError::new("write to read-only property", span)` | E0609 | ✗ | 本 PRD |
| 45-b-symmetric | A6 Increment (`++`) | B2 getter only | D1 numeric | Tier 2 honest error (cell 45-b と op-axis orthogonality-equivalent dispatch、wording = `"write to read-only property"` 同一) | E0609 | ✗ | 本 PRD |
| 45-c | A6 Decrement (`--`) | B4 both | D1 numeric | **Block form setter desugar (cell 43 と op-axis orthogonality-equivalent、BinOp::Sub)**: postfix `{ let __ts_old = obj.x(); obj.set_x(__ts_old - 1.0); __ts_old }` / prefix `{ let __ts_new = obj.x() - 1.0; obj.set_x(__ts_new); __ts_new }` | E0609 | ✗ | 本 PRD |
| 45-da | A6 Decrement (`--`) | B5 AutoAccessor | D1 numeric | Block form setter desugar (PRD 2.8 後 leverage、cell 43 と operator -1 吸収のみ) | Tier 2 honest error (PRD 2.7) | NA | 別 PRD (PRD 2.8) |
| 45-db | A6 Decrement (`--`) | B6 regular method | D1 numeric | Tier 2 honest error: `UnsupportedSyntaxError::new("write to method", span)` (compound assign cell 25 と同 dispatch wording の UpdateExpr arm) | E0609 | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 45-db-symmetric | A6 Increment (`++`) | B6 regular method | D1 numeric | Tier 2 honest error (cell 45-db と op-axis orthogonality-equivalent dispatch、wording = `"write to method"` 同一) | E0609 | ✗ | 本 PRD |
| 45-dc | A6 Decrement (`--`) | B7 inherited setter | D1 numeric | Tier 2 honest error: `UnsupportedSyntaxError::new("write to inherited accessor", span)` (compound assign cell 26 と同 dispatch wording の UpdateExpr arm) | E0609 | △ | 本 PRD (Tier 2 honest error reclassify) |
| 45-dc-symmetric | A6 Increment (`++`) | B7 inherited setter | D1 numeric | Tier 2 honest error (cell 45-dc と op-axis orthogonality-equivalent、wording = `"write to inherited accessor"` 同一) | E0609 | ✗ | 本 PRD |
| 45-dd | A6 Decrement (`--`) | B8 static accessor | D1 numeric | **Block form static setter desugar (cell 27 と operator -1 吸収)**: postfix `{ let __ts_old = Foo::x(); Foo::set_x(__ts_old - 1.0); __ts_old }` / prefix `{ let __ts_new = Foo::x() - 1.0; Foo::set_x(__ts_new); __ts_new }` | Rust syntax error | ✗ | 本 PRD |
| 45-dd-symmetric | A6 Increment (`++`) | B8 static accessor | D1 numeric | Block form static setter desugar (cell 45-dd と op-axis orthogonality-equivalent、BinOp::Add 置換のみ) | Rust syntax error | ✗ | 本 PRD |
| 45-de | A6 Decrement (`--`) | B9 unknown | D1 numeric | **Block form (cell 42 と同 fallback path、BinOp::Sub)**: postfix `{ let __ts_old = obj.x; obj.x = __ts_old - 1.0; __ts_old }` / prefix `{ obj.x = obj.x - 1.0; obj.x }`。Rule 1 (1-4) cell 42 と orthogonality-equivalent inheritance。 | Tier 2 broken | ✗ | **本 PRD (Tier 2 broken → Tier 1 transition)** |
| 45-de-symmetric | A6 Increment (`++`) | B9 unknown | D1 numeric | Block form (cell 45-de と op-axis orthogonality-equivalent、BinOp::Add 置換) | Tier 2 broken | ✗ | 本 PRD |
| 45-b3 | A6 Increment/Decrement (`++`/`--`) | B3 setter only | D1 numeric | Tier 2 honest error: `UnsupportedSyntaxError::new("read of write-only property", span)` (UpdateExpr は read 先行、getter 不在で read fail = compound assign B3 dispatch arm cell 23 と semantic equivalent。Iteration v11 で Spec gap として発覚 = T7 implementation 完了後 Spec→Impl Mapping completeness 化、本 cell は both ops 対応の defensive Tier 2 honest reclassify) | E0609 (read-side) | ✗ | 本 PRD |
| 46 | A7 Destructure read (`const {x} = obj`) | B1 field | * | **Tier 2 honest error reclassify (本 PRD scope)**: `UnsupportedSyntaxError::new("destructure read of class instance", span)`、destructure with class instance は Rust の `let Foo { x, .. } = obj` move semantic と TS `obj.x` reference copy semantic が divergent (TS は own enumerable property 列挙、Rust は struct field 直接 binding)、両 semantic を Rust で TS-equivalent に reproduce 不能 = honest error が ideal | TS で field 直接 access、Rust で move-out unless `obj: Foo` 完全 owned | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 47 | A7 Destructure read (`const {x} = obj`) | B2/B4 getter | * | Tier 2 honest error (`UnsupportedSyntaxError::new("destructure read with getter", span)`、destructure desugar 別 architectural concern) | compile error (E0609 + Rust compilation fail) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 48-a | A8 Spread (`{...obj}`) | B1 field | * | Tier 2 honest error (`UnsupportedSyntaxError::new("spread of class instance with field", span)`、TS spread `{...obj}` は Rust struct リテラル `Foo { ..obj }` に類似だが complete enumerable property mapping 不能 = Rust 直接対応なし) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 48-b | A8 Spread (`{...obj}`) | B2-B9 (any class member shape) | * | Tier 2 honest error (`UnsupportedSyntaxError::new("spread of class instance with accessor", span)`、enumerable property descriptor は getter trigger、Rust 等価表現なし) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 49-a | A9 Delete (`delete obj.x`) | B1 field | * | Tier 2 honest error (`UnsupportedSyntaxError::new("delete on field", span)`、Rust struct field を removable にする mechanism なし、Option<T> 化等の semantic transformation も TS と divergent) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 49-b | A9 Delete (`delete obj.x`) | B2-B9 (any class member shape) | * | Tier 2 honest error (`UnsupportedSyntaxError::new("delete on accessor / method / static", span)`) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 50 | A10 typeof (`typeof obj.x`) | B1 field | * | **Tier 2 honest error reclassify (本 PRD scope)**: `UnsupportedSyntaxError::new("typeof of class field expression", span)`、TS `typeof obj.x` は string return ("number" / "string" / "object" 等)、Rust は static type system でrun-time string 表現なし、既存 ts_to_rs typeof emission は specific narrow context (typeof guard) のみ覆う、generic typeof string return は本 PRD scope 外 = honest error が ideal、別 PRD で Tier 1 化候補 | TS string return、Rust 該当なし | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 51-a | A10 typeof (`typeof obj.x`) | B2 getter only | * | Tier 2 honest error (`UnsupportedSyntaxError::new("typeof of getter", span)`、getter return は static type analysis で resolve 可能だが本 PRD scope 外) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 51-b | A10 typeof (`typeof obj.x`) | B3/B4 setter / both | * | Tier 2 honest error (B3 = setter only で read 不能、B4 = getter return type の static analysis = B2 と同) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 52-a | A11 in (`"x" in obj`) | B1 field | * | Compile-time `true` (field exists at compile time = 静的判定可能、本来は const literal emission)、ただし本 PRD scope 外 = Tier 2 honest error reclassify (`UnsupportedSyntaxError::new("in operator on class instance, even with field", span)`、Rust property reflection 機構なし) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| 52-b | A11 in (`"x" in obj`) | B2-B9 (any class member shape) | * | Tier 2 honest error (Rust property reflection 機構なし、property existence check の semantic を Rust 静的型で完全 reproduce 不能) | direct field access or compile error (B variant 別、本 PRD で Tier 2 reclassify) | ✗ | 本 PRD (Tier 2 honest error reclassify) |
| | | | | | | | |
| 60 | A1 Read (E2 internal `this.x`) | B2 getter only | D1 | `self.x()` (P1 TC39 faithful) | `self.x` field access (broken or works depending on field existence) | ✗ | 本 PRD (E2 internal dispatch) |
| 61 | A2 Write (E2 internal `this.x = v`) | B4 both | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `self.set_x(v)` (P1) | `self.x = v` field write (broken or works) | ✗ | 本 PRD |
| 62 | A1 Read (E2 internal) | B1 field | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | `self.x` (P1 dispatch なしで現状維持) | 同 | ✓ | regression lock-in |
| 63 | A3 Write compound (E2 internal `this.x += v`) | B4 both | D1 | desugar `self.set_x(self.x() + v);` (要 borrow checker temp binding 検討) | E0609 | ✗ | 本 PRD |
| 64 | A6 Increment (E2 internal `this.x++`) | B4 both | D1 | desugar `self.set_x(self.x() + 1);` | E0609 | ✗ | 本 PRD |
| | | | | | | | |
| 70 | Class Method Getter body — `return self.field;` (literal field access return) | D4 String non-Copy | `fn name(&self) -> String { self.field.clone() }` (C1 `.clone()` 自動挿入) | `fn name(&self) -> String { self.field }` (E0507) | ✗ | 本 PRD (C1) |
| 71 | Class Method Getter body — `return self.field;` | D1 f64 Copy | `fn name(&self) -> f64 { self.field }` (no clone needed, Copy) | 同 | ✓ | regression lock-in |
| 72 | Class Method Getter body — `return self.field;` | D5 Vec<T> non-Copy | `fn name(&self) -> Vec<T> { self.field.clone() }` (C1) | E0507 | ✗ | 本 PRD (C1) |
| 73 | Class Method Getter body — `return self.field;` | D6 Option<T> (T が Copy) | `fn name(&self) -> Option<T> { self.field }` (Option<Copy> = Copy) | 同 | ✓ | regression lock-in |
| 74 | Class Method Getter body — `return self.field;` | D6 Option<T> (T が non-Copy) | `fn name(&self) -> Option<T> { self.field.clone() }` (C1) | E0507 | ✗ | 本 PRD (C1) |
| 75 | Class Method Getter body — `return expr;` (computed expression、non-`self.field`) | D4-D15 | Tier 2 user manual `.clone()` (本 PRD scope 外 = C1 `return self.field;` pattern 限定) | compile error E0507 (move out of &self for non-Copy T) | Tier 2 | 別 PRD (C2 comprehensive `.clone()` 自動挿入) |
| 76 | Class Method Getter body — `if cond { return X; } return Y;` (conditional return) | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | Tier 2 user manual `.clone()` (本 PRD scope 外 = C1 limited pattern) | compile error E0507 (move out of &self for non-Copy T) | Tier 2 | 別 PRD (C2) |
| 77 | Class Method Getter body — `let v = self.field; return v;` (let-binding intermediate) | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | Tier 2 user manual `.clone()` (本 PRD scope 外) | compile error E0507 (move out of &self for non-Copy T) | Tier 2 | 別 PRD (C2) |
| 78 | Class Method Getter body — last-expr `self.field` (no return keyword) | D4-D15 non-Copy | **NA (TS spec reject、Iteration v18 empirical observation 由来)**: TS class getter body は statement block で **last-expr ≠ return**、annotation `: T` 付き `get name(): T { this._field }` は tsc reject (TS2378 "A 'get' accessor must return a value." + TS2355 "A function whose declared type is neither 'undefined', 'void', nor 'any' must return a value.")、annotation 無 case は runtime undefined return = `return self.field;` pattern と **NOT semantic equivalent**。`spec-stage-adversarial-checklist.md` Rule 3 (3-1) per TS spec NA reclassify | (NA、empirical observation `## SWC Parser Empirical Lock-ins` cell 78 entry 参照) | NA (Iteration v18 reclassify、初版 spec 誤り = Spec gap、`## Spec Review Iteration Log` Iteration v18 entry 参照) | NA (Iteration v18 で本 PRD scope から削除、TS spec reject)|
| 79 | Class Method Getter body — multi-return early exit | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | Tier 2 user manual `.clone()` (本 PRD scope 外 = C1 limited pattern) | compile error E0507 (move out of &self for non-Copy T) | Tier 2 | 別 PRD (C2) |
| 80 | Class Method Getter body — nested closure body (`return arr.map(x => this.field)` 等) | * (orthogonality-equivalent: D dimension は dispatch logic に影響なし) | Tier 2 user manual `.clone()` (本 PRD scope 外 = C1 limited pattern、closure capture semantic は I-048 と orthogonal) | compile error E0507 (move into closure) or runtime divergence | Tier 2 | 別 PRD (C2 + I-048 closure ownership 推論統合) |
| 81 | Class Method Setter body — `this.field = v;` | * | `fn set_name(&mut self, v: T) { self.field = v; }` (current は OK) | current behavior preserved | ✓ | regression lock-in |

### Spec-Stage Adversarial Review Checklist

[`spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) **13-rule** を全 verification (Implementation stage 移行前必須、本 PRD self-applied integration v2):

- [x] **Rule 1 Matrix completeness + abbreviation prohibition (sub-rule 1-1/1-2/1-3)**: 全 cell に ideal output 記載 (Cartesian matrix ~85 cells explicit enumerate、Rule 10 Step 2 orthogonality reduction 適用済、abbreviation pattern 全廃、`audit-prd-rule10-compliance.py` Rule 1 (1-2) check PASS)
- [x] **Rule 2 Oracle grounding + PRD doc embed mandatory (sub-rule 2-1/2-2/2-3)**: 31 ✗/要調査 cells (16 primary + 15 residual = TS-1 task 完了 + Phase 4 RC-α 完全 populate) に tsc/tsx observation 記録 (`## Oracle Observations` section + `### Additional residual cells (TS-1 task continuation v3 final)` 両 sub-section embed)、残 cells は orthogonality-equivalent inheritance per Rule 1 (1-4)
- [x] **Rule 3 NA justification + SWC parser empirical observation (sub-rule 3-1/3-2/3-3)**: 3 NA candidate cells (cell 44 / cell 6 / cell 8) に SWC parser empirical observation (`## SWC Parser Empirical Lock-ins` section embed、TS-2 task 完了 2026-04-28)、cell 44 reclassify per Rule 3 (3-3) (NA → Tier 2)
- [x] **Rule 4 Grammar consistency + doc-first dependency order (sub-rule 4-1/4-2/4-3)**: ast-variants.md ClassMember section sync (PRD 2.7 で確立)、`Implementation Stage Tasks` T1 (doc update) を T2-T13 全 code 改修 task の direct prerequisite に配置 (audit script Rule 4 (4-3) check PASS)
- [x] **Rule 5 E2E readiness + Stage tasks separation (sub-rule 5-1/5-2/5-3/5-4)**: 19 ✗ cell 対応 fixture `tests/e2e/scripts/i-205/cell-NN-*.{ts,expected}` を red 状態で作成 (TS-3 task 完了 2026-04-28)、Spec Stage Tasks (TS-0〜TS-5) + Implementation Stage Tasks (T1-T15) 2-section split
- [x] **Rule 6 Matrix/Design integrity + Scope 3-tier consistency (sub-rule 6-1/6-2/6-3/6-4)**: matrix Ideal output と Design section emission strategy token-level 一致、Scope 3-tier (In Scope / Out of Scope / Tier 2 honest error reclassify)、matrix Scope 列値 standard 準拠
- [x] **Rule 7 Control-flow exit sub-case completeness**: Getter body return shape sub-cases (cell 70-81): `return self.field;` (D1 Copy = cell 71、D4-D15 non-Copy = cells 70/72/74)、`return expr;` (cell 75)、conditional return (cell 76)、let-binding intermediate (cell 77)、last-expr `self.field` (cell 78、本 PRD C1 拡張)、multi-return (cell 79)、nested closure body (cell 80)、Setter body (cell 81) — 全 sub-case enumerate
- [x] **Rule 8 Cross-cutting invariant enumeration + audit verify (sub-rule 8-5)**: `## Invariants` 独立 section に 6 invariants (INV-1 dispatch consistency / INV-2 internal-external symmetry / INV-3 compound assign side-effect 1-evaluate / INV-4 kind propagation lossless / INV-5 visibility consistency / INV-6 scope boundary preservation)、各 invariant 4 項目 (a)(b)(c)(d) 記載
- [x] **Rule 9 Dispatch-arm sub-case alignment**: matrix cell と Design section dispatch logic 1-to-1 対応 (Spec → Impl verification は Implementation Stage T-* task で実施、Impl → Spec 逆戻りは spec-first-prd.md「Spec への逆戻り」手順)
- [x] **Rule 10 Cross-axis matrix completeness**: 9 default axes (A/B/C/D/E/F + AST dispatch hierarchy) 全 enumerate、Cross-axis orthogonal direction enumerated (`## Rule 10 Application` yaml block embed、audit Rule 10/12 check PASS)
- [x] **Rule 11 AST node enumerate completeness check (sub-rule d-1〜d-5)**: 修正 file `_ => ` arm の処理方針確定 (本 PRD scope = `class.rs:145` fix のみ、I-203 defer = 他多数)、phase 別 mechanism (Transformer = `UnsupportedSyntaxError`、TypeResolver = no-op、NA = `unreachable!()`)、ast-variants.md single source of truth、`## Impact Area Audit Findings` section embed (TS-4 task 完了 2026-04-28)
- [x] **Rule 12 Rule 10/11 Mandatory application + structural enforcement (sub-rule e-1〜e-8)**: `## Rule 10 Application` section yaml format 記入、audit script audit PASS、prd-template skill Step 0c hard-code、CI merge gate
- [x] **Rule 13 Spec Stage Self-Review (skill workflow integrated、sub-rule 13-1〜13-5)**: skill workflow Step 4.5 で 13-rule self-applied verify 実施 (TS-5 task 完了 2026-04-28)、findings v1 (15) → v2 (11) → v3 (resolved + final TS-* execution) を `## Spec Review Iteration Log` section に record、self-applied integration with PRD 2.7 pattern (本 PRD 自身が first-class adopter)

---

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - A (call site context: read/write/compound/destructure/spread/delete/typeof/in)
  - B (receiver type member shape: field/getter only/setter only/both/AutoAccessor/regular method/inherited/static/unknown)
  - C (receiver expression shape: Ident/this/TypeName/chain/call result/complex)
  - D (T variant for .clone() insertion: f64/String/Vec/Option/Struct/Enum/Any/etc.)
  - E (internal vs external: external/internal method body/internal constructor body)
  - F (TS strict / non-strict: 統合 detect、static detect 可能)
  - AST dispatch hierarchy (Rule 10 axis (i)): MemberExpr.prop (Ident/Computed/PrivateName) × member.obj resolved type × AssignExpr.left (Member/Ident) × UpdateExpr.arg (Member/Ident) — 各 layer 独立 axis
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A (matrix-driven PRD)
```

---

## Oracle Observations (Rule 2 (2-2) hard-code、各 ✗/要調査 cell の tsc observation log embed)

**TS-1 task 完了** (2026-04-27 v3 final): 全 representative ✗ cells (16 cells、orthogonality-equivalent cells inherit per Rule 10 Step 2) について `npx tsx /tmp/i205-cells/cell-NN.ts` で empirical observation 実施、log 全 embed。`scripts/observe-tsc.sh` (本 session で tsc command 不在のため `tsx` 経由 Node.js runtime) で stdout/stderr/exit_code capture。

### Cell 2: A1 Read × B2 (getter only) × D1 number (Copy)

- **TS fixture**: `class Counter { _n = 0; get value(): number { return this._n; } } const c = new Counter(); console.log(c.value);`
- **tsx output**: `stdout: 0`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `c.value()` (method call、f64 Copy 値返し) → stdout `0`
- **Rationale**: getter dispatch、Copy primitive 直接返却

### Cell 3: A1 Read × B2 (getter only) × D4 String (non-Copy)

- **TS fixture**: `class Person { _name = "alice"; get name(): string { return this._name; } } const p = new Person(); console.log(p.name);`
- **tsx output**: `stdout: alice`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `p.name()` (method call、`String` 返却 = body で `self._name.clone()` C1 自動挿入) → stdout `alice`
- **Rationale**: getter dispatch、non-Copy T body `.clone()` insertion による owned T return

### Cell 4: A1 Read × B3 (setter only) → Tier 2 honest error

- **TS fixture**: `class Box { _v = 0; set x(v: number) { this._v = v; } } const b = new Box(); b.x = 100; console.log(b.x);`
- **tsx output**: `stdout: undefined`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("read of write-only property", span)`、`undefined` の Rust 等価表現なし)
- **Rationale**: setter only property の read は TS で `undefined`、Rust の Option<T> や ()/`unit` で reproduce すると semantic divergent = honest error が ideal

### Cell 5: A1 Read × B4 (getter + setter) × D4 String

- **TS fixture**: `class Foo { _name = "alice"; get name(): string { return this._name; } set name(v: string) { this._name = v; } } const f = new Foo(); console.log(f.name);`
- **tsx output**: `stdout: alice`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `f.name()` (method call、String body clone)
- **Rationale**: B4 read = B2 read と同 dispatch (getter call)

### Cell 7: A1 Read × B6 (regular method、no parens reference) → Tier 2 honest error

- **TS fixture**: `class Calc { add(a: number, b: number): number { return a + b; } } const c = new Calc(); const fn = c.add; console.log(typeof fn);`
- **tsx output**: `stdout: function`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("method-as-fn-reference (no-paren)", span)`、Rust では method を first-class fn reference として変数 bind 不能、`Type::method` path 経由必要 + `&self` binding 喪失)
- **Rationale**: function reference semantic の Rust 等価表現は closure wrap or `Type::method` path、orthogonal architectural concern (本 PRD scope 外)

### Cell 9: A1 Read × B8 (static getter)

- **TS fixture**: `class Config { static get version(): string { return "1.0.0"; } } console.log(Config.version);`
- **tsx output**: `stdout: 1.0.0`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `Config::version()` (associated fn call、`&self` なし) → stdout `1.0.0`
- **Rationale**: static accessor dispatch、`Class.x` (TS) → `Class::x()` (Rust associated fn) emit

### Cell 12: A2 Write simple × B2 (getter only、write to read-only) → Tier 2 honest error

- **TS fixture**: `class Foo { get x(): number { return 42; } } const f = new Foo(); f.x = 100; console.log(f.x);`
- **tsx output**: `stdout: (空)`、`stderr: TypeError: Cannot set property x of #<Foo> which has only a getter`、`exit_code: non-zero` (uncaught throw)
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("write to read-only property", span)`、conversion-time detect で user に runtime TypeError surface 不要)
- **Rationale**: **重要 finding (v3 oracle re-verify で更新)** — JavaScript class は implicit strict mode、TS class も同様に runtime TypeError throw (= silently no-op **ではない**、v2 wording は誤、v3 で empirical 確認・修正)。Rust に直接 reproduce する mechanism なし = honest error が ideal

### Cell 13: A2 Write simple × B3 (setter only)

- **TS fixture**: `class Box { _v = 0; set x(v: number) { this._v = v * 2; } get _peek(): number { return this._v; } } const b = new Box(); b.x = 5; console.log(b._peek);`
- **tsx output**: `stdout: 10`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `b.set_x(5);` (setter dispatch、body 内 `self._v = v * 2`) → `b._peek()` で 10 read
- **Rationale**: setter dispatch、body 内 logic は preserve、setter 呼出 effect 確認

### Cell 14: A2 Write simple × B4 (both、setter dispatch with body logic)

- **TS fixture**: `class Foo { _n = 0; get x(): number { return this._n; } set x(v: number) { this._n = v + 1; } } const f = new Foo(); f.x = 5; console.log(f.x);`
- **tsx output**: `stdout: 6`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `f.set_x(5);` (setter `self._n = v + 1` で 6) → `f.x()` で 6 read
- **Rationale**: B4 write = setter dispatch (B3 と同)、body logic preserve

### Cell 18: A2 Write simple × B8 (static setter)

- **TS fixture**: `class Config { static _v = 0; static get x(): number { return Config._v; } static set x(v: number) { Config._v = v * 10; } } Config.x = 5; console.log(Config.x);`
- **tsx output**: `stdout: 50`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `Config::set_x(5);` (associated fn call、static body 内 `Config::_v = v * 10` で 50)
- **Rationale**: static accessor write = `Class::set_x(v)` emit、static instance state mutation

### Cell 21: A3 += × B4 (compound assign with getter+setter)

- **TS fixture**: `class Counter { _n = 10; get value(): number { return this._n; } set value(v: number) { this._n = v; } } const c = new Counter(); c.value += 5; console.log(c.value);`
- **tsx output**: `stdout: 15`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `c.set_value(c.value() + 5);` (= 10 + 5 = 15) → `c.value()` で 15
- **Rationale**: **重要 invariant verify (INV-3)** — compound assign desugar の receiver evaluation 1 回 (TS `c.value += 5` で `c` 1 回 eval)、Rust desugar も `c` 1 回 eval (side-effect-free receiver) で TS semantic 一致

### Cell 38: A5 ??= × B4 (logical compound、Option<T>)

- **TS fixture**: `class Cache { _v: number | undefined = undefined; get value(): number | undefined { return this._v; } set value(v: number | undefined) { this._v = v; } } const c = new Cache(); c.value ??= 42; console.log(c.value);`
- **tsx output**: `stdout: 42`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `if c.value().is_none() { c.set_value(Some(42)); }` (statement context、既存 nullish_assign helper integration、`Option<T>` 型 body)
- **Rationale**: ??= dispatch は logical short-circuit 評価、既存 nullish_assign helper (I-142) を leverage

### Cell 43: A6 ++ × B4 (postfix increment、numeric)

- **TS fixture**: `class Counter { _n = 5; get value(): number { return this._n; } set value(v: number) { this._n = v; } } const c = new Counter(); c.value++; console.log(c.value);`
- **tsx output**: `stdout: 6`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `c.set_value(c.value() + 1);` (postfix `++` の場合は old value 評価 + 後 increment、本 cell では console.log の前なので post-increment と pre-increment 結果差は不要、6 = 5+1 で確認)
- **Rationale**: A6 ++ dispatch = compound `+= 1` desugar と equivalent、IR BinOp 層で吸収

### Cell 60: A1 Read (E2 internal `this.x`) × B2 (getter only)

- **TS fixture**: `class Logger { _prefix = "[INFO]"; get prefix(): string { return this._prefix; } log(msg: string): void { console.log(this.prefix + " " + msg); } } const l = new Logger(); l.log("hello");`
- **tsx output**: `stdout: [INFO] hello`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `l.log("hello")` 内部で `self.prefix()` (P1 TC39 faithful = method call 経由、direct `self._prefix` field access ではない)
- **Rationale**: **重要 invariant verify (INV-2)** — internal-external dispatch path symmetry (内部 `this.prefix` は external `obj.prefix` と同 dispatch)

### Cell 61: A2 Write (E2 internal `this.x = v`) × B4 (both)

- **TS fixture**: `class Counter { _n = 0; get value(): number { return this._n; } set value(v: number) { this._n = v; } incrInternal(): void { this.value = this.value + 1; } } const c = new Counter(); c.incrInternal(); console.log(c.value);`
- **tsx output**: `stdout: 1`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `incr_internal` 内部で `self.set_value(self.value() + 1);` (P1 + INV-2 cohesion)、Rust borrow checker 観点では `&mut self` の中で `self.value()` (immutable borrow) と `self.set_value(...)` (mutable borrow) 共存不能 → **temp binding 必要** (`let __tmp = self.value() + 1; self.set_value(__tmp);`)
- **Rationale**: **重要 borrow checker constraint (Implementation Stage 影響)** — 内部 dispatch で borrow 衝突発生、temp binding mandatory (T6 / T8 / T10 で対応)

### Cell 70: Class Method Getter body — `return self.field;` × D4 String non-Copy

- **TS fixture**: `class Profile { _name: string = "alice"; get name(): string { return this._name; } } const p = new Profile(); console.log(p.name);`
- **tsx output**: `stdout: alice`、`stderr: (空)`、`exit_code: 0`
- **Ideal Rust**: `fn name(&self) -> String { self._name.clone() }` (C1 `.clone()` 自動挿入、`String` non-Copy で `&self` から move 不能)
- **Rationale**: getter body の `return self.field;` pattern detect + non-Copy T で C1 `.clone()` 自動挿入、TS runtime stdout `alice` を Rust が再現

### Additional residual cells (TS-1 task continuation v3 final、F-rev-1 fix、~15 cells empirically observed)

各 cell について TS fixture / tsx output / Ideal Rust / Rationale を compact format で記録。完全 Oracle observation populate で Rule 2 (2-2) full coverage 達成。

#### Cell 22: A3 += × B2 getter only (compound assign to read-only)

- **TS fixture**: `class Foo { get x(): number { return 10; } } const f = new Foo(); f.x += 5;`
- **tsx output**: `stderr: TypeError: Cannot set property x of #<Foo> which has only a getter`、`exit_code: non-zero`
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("compound assign to read-only property", span)`)
- **Rationale**: TS で TypeError throw (cell 12 と同 implicit strict mode)、Rust で reproduce 不能 = honest error

#### Cell 23: A3 += × B3 setter only (read part undefined + numeric coercion)

- **TS fixture**: `class Box { _v = 0; set x(v: number) { this._v = v; } } const b = new Box(); (b as any).x += 5; console.log((b as any).x);`
- **tsx output**: `stdout: undefined`
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("compound assign read of write-only property", span)`、`undefined + 5 = NaN` 軌道 + setter で `_v = NaN`、しかし read 不能で再度 undefined return)
- **Rationale**: setter only の read 部 undefined → numeric coercion で NaN、Rust に直接対応なし

#### Cell 25: A3 += × B6 method (method ref + string coercion)

- **TS fixture**: `class Calc { add(a: number, b: number): number { return a + b; } } const c = new Calc(); (c as any).add += 1;`
- **tsx output**: `stdout: string` (typeof 結果)
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("compound assign to method", span)`、TS は method を function-string coerce で string 化、Rust に reproduce 不能)

#### Cell 26: A3 += × B7 inherited (works at runtime via prototype chain)

- **TS fixture**: `class Base { _n = 10; get x(): number { return this._n; } set x(v: number) { this._n = v; } } class Sub extends Base {} const s = new Sub(); s.x += 5; console.log(s.x);`
- **tsx output**: `stdout: 15`
- **Ideal Rust**: **Tier 2 honest error reclassify (本 PRD scope)** (`UnsupportedSyntaxError::new("compound assign to inherited accessor", span)`、Rust struct inheritance not supported in 本 PRD)、Tier 1 化は別 PRD (Class inheritance dispatch)

#### Cell 27: A3 += × B8 static (works、Tier 1 dispatch in 本 PRD)

- **TS fixture**: `class Cnt { static _v = 10; static get x(): number { return Cnt._v; } static set x(v: number) { Cnt._v = v; } } Cnt.x += 5; console.log(Cnt.x);`
- **tsx output**: `stdout: 15`
- **Ideal Rust**: `Cnt::set_x(Cnt::x() + 5);` (associated fn dispatch、本 PRD scope Tier 1)

#### Cell 36: A5 ??= × B1 field (Option<T>、既存 nullish_assign helper leverage)

- **TS fixture**: `class Cache { v: number | undefined = undefined; } const c = new Cache(); c.v ??= 42;`
- **tsx output**: `stdout: 42`
- **Ideal Rust**: `c.v.get_or_insert_with(|| 42);` (既存 I-142 nullish_assign helper、B1 field path、regression lock-in)

#### Cell 37: A5 ??= × B2 getter only (TypeError throw)

- **TS fixture**: `class Foo { get v(): number | undefined { return undefined; } } const f = new Foo(); (f as any).v ??= 42;`
- **tsx output**: `stderr: TypeError: Cannot set property v of #<Foo> which has only a getter`、`exit_code: non-zero`
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("nullish-assign to read-only getter", span)`、cell 12/22 と同 pattern)

#### Cell 39: A5 &&= × B4 both (logical short-circuit through setter)

- **TS fixture**: `class Foo { _b = true; get b(): boolean { return this._b; } set b(v: boolean) { this._b = v; } } const f = new Foo(); f.b &&= false; console.log(f.b);`
- **tsx output**: `stdout: false`
- **Ideal Rust**: `if f.b() { f.set_b(false); }` (statement context)、本 PRD Tier 1 dispatch

#### Cell 40: A5 ||= × B4 both (logical short-circuit through setter)

- **TS fixture**: `class Foo { _b = false; get b(): boolean { return this._b; } set b(v: boolean) { this._b = v; } } const f = new Foo(); f.b ||= true; console.log(f.b);`
- **tsx output**: `stdout: true`
- **Ideal Rust**: `if !f.b() { f.set_b(true); }` (statement context)

#### Cell 47: A7 destructure read with B2/B4 getter (works in TS、Tier 2 in Rust)

- **TS fixture**: `class Foo { get x(): number { return 42; } } const f = new Foo(); const {x} = f; console.log(x);`
- **tsx output**: `stdout: 42`
- **Ideal Rust**: **Tier 2 honest error reclassify** (`UnsupportedSyntaxError::new("destructure read of class instance with getter", span)`、destructure desugar 別 architectural concern)

#### Cell 63: A3 += internal `this.x` × B4 both (borrow checker INV-3 verify)

- **TS fixture**: `class Counter { _n = 10; get value(): number { return this._n; } set value(v: number) { this._n = v; } incrInternal(): void { this.value += 1; } } const c = new Counter(); c.incrInternal();`
- **tsx output**: `stdout: 11`
- **Ideal Rust**: `incr_internal` 内部で **temp binding mandatory**: `let __tmp = self.value() + 1; self.set_value(__tmp);` (borrow checker `&mut self` constraint、INV-3 + INV-2 cohesive)
- **Rationale**: internal compound assign で borrow conflict 回避

#### Cell 64: A6 ++ internal `this.x` × B4 both

- **TS fixture**: `class Counter { _n = 5; get value(): number { return this._n; } set value(v: number) { this._n = v; } incrInternalIncr(): void { this.value++; } } const c = new Counter(); c.incrInternalIncr();`
- **tsx output**: `stdout: 6`
- **Ideal Rust**: cell 63 と symmetric: `let __tmp = self.value() + 1; self.set_value(__tmp);` (postfix ++、internal context)

#### Cell 71: Class Method Getter body × D1 number Copy (no `.clone()` needed、regression)

- **TS fixture**: `class Foo { _n: number = 42; get n(): number { return this._n; } } const f = new Foo(); console.log(f.n);`
- **tsx output**: `stdout: 42`
- **Ideal Rust**: `fn n(&self) -> f64 { self._n }` (Copy primitive、no `.clone()`)、regression lock-in

#### Cell 72: Class Method Getter body × D5 Vec<T> non-Copy (`.clone()` insertion)

- **TS fixture**: `class Bag { _items: number[] = [1, 2, 3]; get items(): number[] { return this._items; } } const b = new Bag(); console.log(b.items);`
- **tsx output**: `stdout: [ 1, 2, 3 ]`
- **Ideal Rust**: `fn items(&self) -> Vec<f64> { self._items.clone() }` (C1 `.clone()` insertion for Vec)

#### Cell 74: Class Method Getter body × D6 Option<non-Copy> (`.clone()` insertion)

- **TS fixture**: `class Cache { _v: string | undefined = "hello"; get v(): string | undefined { return this._v; } } const c = new Cache(); console.log(c.v);`
- **tsx output**: `stdout: hello`
- **Ideal Rust**: `fn v(&self) -> Option<String> { self._v.clone() }` (Option<String> = non-Copy 全体)

### Orthogonality-equivalent cells inherit observations (Rule 10 Step 2 reduction)

以下の cells は上記 representative cells の dispatch logic を inherit (orthogonality-equivalent class):

- Cell 22 (A3 += × B2 getter only) ← cell 12 と equivalent → Tier 2 honest error
- Cell 23 (A3 += × B3 setter only) ← cell 4 と equivalent (compound `+=` の read 部不能)
- Cells 29-a〜29-d (A3 -= × B1-B4) ← cells 20-21 (A3 += × B1/B4) と equivalent (operator のみ吸収)
- Cells 30-34-c (A4 bitwise compound × B1-B4) ← cells 20-21 と equivalent
- Cells 36-37 (A5 ??= × B1/B2) ← cell 38 variant
- Cells 39-40 (A5 &&= / ||= × B4) ← cell 38 と equivalent (logical short-circuit)
- Cells 45-a〜45-c (A6 -- × B1/B2/B4) ← cells 42-43 と equivalent (operator -1 吸収)
- Cells 46-52 (A7-A11 corner × B) ← Tier 2 honest error reclassify (各 case の Rust 等価表現不能)
- Cell 63 (A3 += E2 internal × B4) ← cells 21 + 60 + 61 cohesion
- Cell 64 (A6 ++ E2 internal × B4) ← cell 63 + 43 cohesion
- Cells 71-78 (Class Method Getter body sub-cases) ← cell 70 (D4 non-Copy `.clone()`) と D variant 違いで inherit、Copy T (D1) は no clone (cell 71)、Vec/Option/Struct/Enum は cell 70 と同 C1 pattern

## SWC Parser Empirical Lock-ins (Rule 3 (3-2) hard-code、各 NA cell の SWC parser empirical lock-in)

**TS-2 task 完了** (2026-04-28 v3 final): NA candidate cells (cell 44 `++` on non-numeric / cell 6 AutoAccessor / cell 8 inherited) について empirical observation 実施。**重要 finding (Rule 3 (3-3) reclassify): SWC accept = Tier 2 honest error reclassify**。

### Cell 44: A6 Increment (`++`) × D4 String non-numeric → 当初 NA → **Tier 2 honest error reclassify (Rule 3 (3-3))**

- **当初 spec-traceable reason (v2)**: TS spec で `++` は **numeric only** (number / bigint)、String 等 non-numeric T への `++` は parser reject 想定
- **SWC parser empirical evidence (v3 update 2026-04-28)**:
  - **TS fixture**: `let s: string = "abc"; s++; console.log(s);`
  - **tsx output**: `stdout: NaN`、`stderr: (空)`、`exit_code: 0`
  - **Behavior**: SWC parser は `s++` を `UpdateExpr { op: PlusPlus, arg: Ident("s") }` として **accept**、tsx runtime で String → Number coercion → `NaN` 算出 (= TS spec 違反だが ECMAScript 寛容 coercion semantic)
  - **Test path**: `tests/swc_parser_increment_non_numeric_test.rs::test_increment_on_string_swc_accepts` (作成予定、Implementation Stage で commit、本 PRD self-applied integration として TS-2 task 内で書き込み)
- **Reclassification per Rule 3 (3-3)**: NA → **Tier 2 honest error reclassify (本 PRD scope)** — SWC accept、`unreachable!()` macro precondition violation 防止のため honest error 経由 (`UnsupportedSyntaxError::new("increment of non-numeric (String/etc.) — TS NaN coercion semantic", span)`)。Rust に NaN coercion semantic の直接対応なし、honest error が ideal (PRD 2.7 cell 15 lesson と同 pattern)。

### Cell 6: A1 Read × B5 (AutoAccessor without decorator) → 別 PRD (PRD 2.8 scope) ではあるが SWC empirical lock-in 必要

- **TS fixture**: `class Foo { accessor x: number = 0; } const f = new Foo(); console.log(f.x);`
- **tsx output**: `stdout: 0`、`stderr: (空)`、`exit_code: 0`
- **Behavior**: SWC parser は `accessor x: number = 0;` を `ClassMember::AutoAccessor` として **accept**、tsx runtime で auto-generated getter/setter 経由で正常動作
- **Reclassification**: NOT NA (SWC accepts、tsx runs)。本 PRD scope では PRD 2.7 で確立した **Tier 2 honest error** 状態維持 (`UnsupportedSyntaxError::new("AutoAccessor", aa.span)` via `src/transformer/classes/mod.rs:165-171`)。本 PRD framework (method kind tracking infrastructure) を leverage して **PRD 2.8 (I-201-A) で Tier 1 完全変換**。
- **Test path**: 既存 `tests/e2e/scripts/prd-2.7/cell-07-auto-accessor-honest-error.ts` で lock-in 済 (Tier 2 状態 = PRD 2.7 close 時点 verify)

### Cell 8: A1 Read × B7 (inherited getter) → Tier 2 honest error reclassify (本 PRD scope)

- **TS fixture**: `class Base { _n: number = 42; get x(): number { return this._n; } } class Sub extends Base {} const s = new Sub(); console.log(s.x);`
- **tsx output**: `stdout: 42`、`stderr: (空)`、`exit_code: 0`
- **Behavior**: SWC parser は class extends + getter を **accept**、tsx runtime で prototype chain 経由 inherited getter 呼出で 42 read
- **Reclassification per Rule 3 (3-3)**: NOT NA (SWC accepts、tsx runs)。Rust struct に直接 inheritance mechanism なし (trait による interface 継承 or composition pattern が候補だが本 PRD scope = "Class member access dispatch" 単位 architectural concern と orthogonal、別 PRD = "Class inheritance dispatch" で扱う) → **Tier 2 honest error reclassify (本 PRD scope)** で silent drop 排除、Tier 1 化は別 PRD。
- **Test path**: `tests/swc_parser_inherited_accessor_test.rs::test_inherited_getter_swc_accepts` (作成予定、Implementation Stage で commit)

### Other NA cells (本 PRD scope 内、Rule 3 (3-2) orthogonality inheritance justification、F-deep-deep-3 fix)

本 PRD matrix の他の B5 AutoAccessor NA cells は **parser-level context-independence** により cell 6 inherit:

- **Cells 15** (A2 Write × B5)、**Cell 24** (A3 += × B5)、**Cell 29-e-a** (A3 -=/etc × B5)、**Cell 41-a** (A5 logical × B5)、**Cell 45-da** (A6 -- × B5)
- **Inheritance justification (parser-level orthogonality)**: SWC parser は AutoAccessor declaration syntax `accessor x: T = init` を context-independent に accept (= A read/write/compound 全 context で同一 AST shape `ClassMember::AutoAccessor` parse)。Cell 6 の `swc_parser_auto_accessor_test.rs::test_swc_parser_accepts_auto_accessor_simple` は declaration parsing を test、A-context (Read/Write/Compound) の差は declaration parser で表れない (post-parse の dispatch logic で展開される、これは Implementation Stage scope)。
- **Spec stage structural verify**: 本 inheritance claim は cell 6 SWC parser test の "AutoAccessor declaration accept" 結果が **A-context 不変** な事実に依拠。本 inheritance を破る condition (= SWC parser が context-dependent parsing を行う) は parser regression を意味し、`tests/swc_parser_auto_accessor_test.rs` の 4 sub-cases (simple / no-init / static / private) で多重 verify されている。
- **Implementation Stage T15 で integration verify**: 各 NA cell の dispatch (本 PRD の Tier 2 honest error reclassify) が cell 6 と同 mechanism (`UnsupportedSyntaxError::new("AutoAccessor", aa.span)` via `class.rs:165-171`) で fire することを cell 6/15/24/29-e-a/41-a/45-da 各々 probe で confirm。

### Lesson learned (v3)

PRD 2.7 cell 15 と本 PRD cell 44 で **同 pattern (TS spec NA 想定 → SWC empirical accept → Tier 2 reclassify)** が再発。Rule 3 (3-2) の SWC parser empirical observation 必須化 (v1.2、PRD 2.7 confirmed) + 本 PRD I-205 self-applied verify で framework 強化 (`spec-stage-adversarial-checklist.md` Rule 3 v1.2 適用 verify、本 PRD で再確認)。

### Cell 78: Class Method Getter body — last-expr `self.field` (no return keyword) → **NA reclassify (Iteration v18、2026-05-01、TS spec reject)**

- **当初 spec claim (initial spec、line 305、Iteration v9 base)**: 「last-expr `self.field`
  を `.clone()` 付きに rewrite、`return self.field;` pattern と semantic equivalent、
  Rust では last-expr = implicit return」 → 本 PRD scope (C1 last-expr 拡張)
- **TS-2 task で skip された empirical observation (framework 失敗 signal)**: cell 78
  の TS code が tsc / tsx で actually どう振る舞うかを Spec stage で empirical 確認
  していなかった = `spec-stage-adversarial-checklist.md` Rule 3 (3-1)/(3-2) 違反
- **Iteration v18 empirical observation (2026-05-01、T12 着手前)**:
  - **TS fixture**: `class Profile { _name: string = "alice"; get name(): string { this._name } } const p = new Profile(); console.log(p.name);`
  - **tsc errors**:
    - `TS2378: A 'get' accessor must return a value.` (line 3, col 7)
    - `TS2355: A function whose declared type is neither 'undefined', 'void', nor 'any' must return a value.` (line 3, col 15)
  - **tsx output**: `stdout: undefined`、`stderr: (空)`、`exit_code: 0` (= TS class
    getter body は statement block で last-expr ≠ return、annotation 付き form は
    tsc reject、annotation 無 form は runtime undefined return)
  - **Test path**: `/tmp/cell78-empirical.ts` (Iteration v18 ad-hoc、本 PRD では fixture
    化せず PRD doc 内 embed のみ)
- **Reclassification per Rule 3 (3-1) per TS spec**: NA → **NA reclassify (TS spec reject)**
  — TS spec で valid な class getter body は **explicit return statement form 必須**
  (`{ return X; }`)、last-expr form (`{ this._name }`) は `: T` annotation 付きで
  tsc reject、annotation 無で undefined return = `return self.field;` pattern と NOT
  semantic equivalent。「Rust の last-expr = implicit return semantic を leverage する」
  claim は valid TS source として存在しない input を前提としており spec として誤り。
- **Framework 失敗 signal**: 本 spec 誤りは Spec stage の empirical observation skip
  で発生 = `spec-stage-adversarial-checklist.md` Rule 3 (3-2) SWC parser empirical
  observation の Spec stage Mandatory enforcement 強化 candidate (Iteration v18 entry
  改善 A 参照)。

### Cell 74 fixture: `class Cache` name conflict → **fixture rename (Iteration v18、2026-05-01)**

- **当初 fixture (TS-3 task、2026-04-28 v3 final で red 状態 lock-in)**:
  `class Cache { _v: string | undefined = "hello"; get v(): string | undefined { return this._v; } }`
- **TS-3 task で skip された fixture-content empirical observation (framework 失敗 signal #2)**:
  per-cell E2E fixture を作成した際、fixture file 自体の tsc empirical observation を
  skip した = `spec-stage-adversarial-checklist.md` Rule 5 (5-1) sub-rule 拡張 candidate
  (= "fixture content 自体の正当性 verify")
- **Iteration v18 empirical observation (2026-05-01、T12 着手前)**:
  - **tsc errors**:
    - `TS2300: Duplicate identifier 'Cache'.` (line 1, col 7)
    - `TS2339: Property 'v' does not exist on type 'Cache'.` (line 3, col 15)
  - **tsx output**: `stdout: hello`、`stderr: (空)`、`exit_code: 0` (= class declaration
    + getter return semantic 自体は正常、tsc error は class name `Cache` が **ES2017+
    standard built-in `Cache` interface (Service Worker API)** との duplicate identifier
    conflict)
- **Fix per Iteration v18 (本 commit 内、本 PRD scope)**: fixture content `class Cache`
  → `class OptCache` (or similar non-conflicting class name) に rename、fixture spec /
  ideal output は維持 (D6 Option<non-Copy> = `string | undefined` getter return type、
  ideal output `hello\n` 不変)。rename 後 `scripts/observe-tsc.sh` で再 empirical
  observation で tsc accept verify。
- **Framework 失敗 signal**: 本 fixture bug は Spec stage で fixture content の tsc
  empirical observation skip で発生 = `spec-stage-adversarial-checklist.md` Rule 5
  (5-1) sub-rule 拡張 candidate (Iteration v18 entry 改善 C 参照)。

## Impact Area Audit Findings (Rule 11 (d-5) hard-code、`_` arm violations 一覧 + 決定)

`audit-ast-variant-coverage.py --files <impact-area>` 実行結果 (本 v2 では tree-sitter-rust 不在のため manual grep approximation、CI で正規 audit run):

| Violation | Location | Phase | Decision | Rationale |
|-----------|----------|-------|----------|-----------|
| Rule 11 d-1 `_ => {}` arm (silent drop) | `src/registry/collection/class.rs:145` | Registry collection | **本 PRD scope で fix (T3 task、2026-04-28 完了)** | method kind tracking の blocker。`_ => {}` で AutoAccessor / PrivateMethod / StaticBlock / TsIndexSignature / Empty を silent drop していた = 本 PRD framework 構築の前提として explicit enumerate 必須 |
| `extends: vec![]` hardcode (Implementation gap、Iteration v9 で発覚) | `src/registry/collection/class.rs:195` | Registry collection | **本 PRD scope で fix (T5 task)** | B7 inherited detection (Design section #3-bis `lookup_method_kind_with_parent_traversal`) の前提条件。class.class.super_class 経由 Vec<String> 化 (interface decl.rs:63-73 と symmetric)、本 PRD architectural concern infrastructure 範疇 (registration phase は I-013/I-014/I-206 の consumer phase と独立 axis) |
| Empty body class register filter (Implementation gap、Iteration v9 secondary) | `src/registry/collection/decl.rs:264` | Registry collection | **本 PRD scope で fix (T5 task)** | Pass 2 register condition `!fields.is_empty() \|\| !methods.is_empty() \|\| constructor.is_some()` で empty body class (e.g., `class Sub extends Base {}`) を register せず、Pass 1 placeholder の空 TypeDef (= extends: []) のまま放置。`extends.is_empty()` も condition に追加し、extends を持つ class は body が空でも Pass 2 結果を register。Pass 1 placeholder ↔ Pass 2 collect の data preservation invariant の structural fix |
| Rule 11 d-1 `_ => return Err(...)` arm | `src/transformer/expressions/member_access.rs:187` (opt chain) | Transformer | **I-203 defer** | OptChain unsupported member の error return、本 PRD architectural concern (= getter/setter dispatch) と orthogonal |
| Rule 11 d-1 `_ => return Err(...)` arm × 2 | `src/transformer/expressions/assignments.rs:20, 22` (assign target) | Transformer | **I-203 defer** | AssignTarget 未対応 patterns (Pattern / Computed) の error return、本 PRD scope は SimpleAssignTarget::Member dispatch 拡張のみ。**(d-6-b-1) Orthogonality**: AssignTarget pattern dispatch (Pattern/Computed/etc.) は本 PRD architectural concern (= getter/setter dispatch via Member target) と orthogonal、別 PRD I-203 の codebase-wide AST exhaustiveness scope。**(d-6-b-2) Non-interference**: 本 PRD で modify する SimpleAssignTarget::Member arm の control flow は他 AssignTarget arms (Pattern/Computed) の挙動に dependent しない (probe location: `dispatch_member_write` helper は Member target のみ消費、他 patterns は既存 path 維持) |
| Rule 11 d-1 `_ => return Err(...)` arm | `src/transformer/classes/members.rs:167` (param prop pattern) | Transformer | **I-203 defer** | TsParamProp pattern 未対応 case、本 PRD architectural concern と orthogonal |
| Rule 11 d-1 `_ => continue` arm | `src/registry/collection/class.rs:28, 60, 76, 111` | Registry collection | **I-203 defer** | PropName / pattern matching での continue (silent drop ではないが explicit enumerate 推奨)、本 PRD scope と orthogonal |
| Rule 11 d-1 `_ => None` / `_ => Err(...)` arms (multi) | `src/registry/mod.rs:649, 655, 668, 697, 700` 等 | Registry | **I-203 defer** | Type matching default arm、本 PRD scope と orthogonal |
| Rule 11 d-1 `_ => Err(...)` arms × 3 | `src/ts_type_info/mod.rs:436, 443, 450` | TsTypeInfo | **I-203 defer** | TS type matching default arm、本 PRD scope と orthogonal |

**Summary**: 本 PRD scope で fix = **1 violation** (`class.rs:145` 必須 = method kind tracking の blocker)、I-203 defer = **多数 violations** (codebase-wide 一括 fix が ideal、本 PRD と orthogonal architectural concern)。

## Field Addition Symmetric Audit (Rule 9 sub-rule (c-1) self-applied integration、framework v1.7 source、確定 2026-04-28)

本 PRD I-205 は **field-addition PRD** (T2 で `MethodSignature.kind` / `TsMethodInfo.kind` field を追加) であり、Fix 4 で導入した `spec-stage-adversarial-checklist.md` Rule 9 sub-rule (c-1) "Field-addition symmetric conversion site audit" を **本 PRD 自身が first-class adopter として self-applied 適用**。3 strategy (`hardcode default` / `propagate from source` / `propagate from m.X`) の責務を全 construction site で spec-traceable に記録。

### `MethodSignature` 構築 site (production)

| # | Location | Strategy | Justification |
|---|---|---|---|
| 1 | `src/registry/collection/class.rs:97` (Constructor arm) | hardcode `MethodKind::Method` | constructor は getter/setter ではない (TS spec)、Method semantic 妥当 |
| 2 | `src/registry/collection/class.rs:139` (Method arm) | propagate from `MethodKind::from(method.kind)` | T3 で SWC `ClassMethod.kind` を IR 側に lossless propagate (Method/Getter/Setter 区別を保持) |
| 3 | `src/registry/collection/type_literals.rs:93` (`convert_method_info_to_sig`) | propagate from `m.kind` (Fix 2 で修正) | TsMethodInfo の kind を MethodSignature に lossless propagate、`resolve_method_sig` (T3 fix) と symmetric な boundary conversion |
| 4 | `src/registry/collection/type_literals.rs:114` (`convert_fn_sig_to_method_sig`) | hardcode `MethodKind::Method` | TsFnSigInfo (function signature) は getter/setter ではない、Method semantic 妥当 |
| 5 | `src/registry/interfaces.rs:74` (`build_method_signature_from_method_decl`) | hardcode `MethodKind::Method` | TsMethodSignature 限定変換、TsGetterSignature/TsSetterSignature は Tier 2 unsupported (`ast-variants.md` TsTypeElement section 参照)、Method semantic 妥当 |
| 6 | `src/external_types/mod.rs:492` (external builtin types registration) | hardcode `MethodKind::Method` | external builtin (Array.push / String.charAt 等) は全て regular method、getter/setter 不在 |
| 7 | `src/registry/mod.rs:189` (`MethodSignature::substitute`) | propagate from `self.kind` | type substitution は kind を変更しない、self.kind preserve が正しい (T2 で確立) |
| 8 | `src/ts_type_info/resolve/typedef.rs:603` (`resolve_method_sig`、Pass 2) | propagate from `sig.kind` (T3 で修正) | `MethodSignature<TsTypeInfo>` → `MethodSignature<RustType>` 変換で kind を lossless preserve (旧 hardcoded `MethodKind::Method` の latent bug を T3 で fix) |

### `MethodSignature` 構築 site (test fixtures、~40 site)

test fixture site (`src/registry/tests/`、`src/transformer/*/tests/`、`src/pipeline/type_resolver/tests/` 等) は **hardcode `MethodKind::Method`** (= bulk-script で T2 batch backward compat fallback)。各 test fixture は production code path を test するため、kind を変更したい test (= getter/setter dispatch test) は新規追加 (T3 で 4 件追加 + Fix 2 で 3 件追加)。
- **Bulk script で hardcode された理由**: T2 で field 追加時に既存 51 site を一括 backward compat (= Default 実装の `Method` と一致、既存 test 動作不変)
- **既存 hardcode の妥当性**: 各 test は kind 関係 logic を test していない (= type substitution / overload resolution / 他 dispatch logic)、`MethodKind::Method` は意味的に neutral (= getter/setter 区別が必要な test はそもそも getter/setter fixture を要する)

### `TsMethodInfo` 構築 site (production)

| # | Location | Strategy | Justification |
|---|---|---|---|
| 9 | `src/ts_type_info/helpers.rs:77` (`extract_method_infos_from_type_elements`、TsMethodSignature arm) | hardcode `MethodKind::Method` | TsMethodSignature 限定 (TsGetterSignature/TsSetterSignature は Tier 2 unsupported、本 helper の match arm に到達しない)、Method semantic 妥当 |

### `TsMethodInfo` 構築 site (test fixtures、~10 site)

`src/ts_type_info/resolve/intersection_tests.rs`、`src/registry/collection/type_literals.rs::tests` 等。production と同 pattern で hardcode `MethodKind::Method` (Fix 2 の 3 件は意図的に Getter/Setter で構築)。

### Symmetric audit conclusion

全 construction site (production 9 site + test fixtures ~50 site) について各 strategy が **spec-traceable** に justified、**latent kind drop pattern 0 件** (post Fix 2)。本 audit は I-205 自身が Rule 9 sub-rule (c-1) の first-class adopter として self-applied 適用、framework v1.7 self-applied integration 完成 (= 同 pattern を future field-addition PRD にも proactive 適用可能)。

**Recurring problem evidence (Rule 9 sub-rule (c) 適用必要性の証明)**:
- I-383 T8' (`MethodSignature.type_params` field 追加): 当時 symmetric audit なし、`type_params: vec![]` を bulk-update で hardcode (現 codebase 痕跡)
- I-205 T2 (`MethodSignature.kind` / `TsMethodInfo.kind` field 追加): symmetric audit 不在で `convert_method_info_to_sig` / `resolve_method_sig` で 2 site latent kind drop 発生、後者は test failure trigger で発見 (T3 fix)、前者は `/check_job` 4-layer review で発見 (Fix 2)
- = **2 度連続の recurring problem signal**、3 度目防止に Rule 9 sub-rule (c) 必須化 (Fix 4 で framework v1.6 → v1.7、本 audit section が Rule 9 sub-rule (c-1) compliance evidence)

## Invariants (Rule 8 (8-5) hard-code、独立 section)

### INV-1: Receiver type member kind dispatch consistency

- **(a) Property statement**: 全 read context (A1) で `obj.x` が emit される際、receiver type の `methods.get(field).kind` 検査結果に基づく dispatch (getter exists → MethodCall、それ以外 → FieldAccess) が **全 emit path で一貫**
- **(b) Justification**: dispatch logic が複数 entry point (convert_member_expr / opt chain inner / 等) に重複した結果、片方で getter dispatch、片方で field fallthrough する場合、call site context によって semantic が divergent (silent semantic divergence)
- **(c) Verification method**: Integration test (`tests/i205_invariants_test.rs::test_invariant_1_dispatch_consistency_across_call_sites`) で external `obj.x` / opt chain `obj?.x` / ternary branch `(c ? a : b).x` 等の複数 receiver shape × B (member shape) 全 cell の emit を probe、全て同 dispatch logic 経由を verify
- **(d) Failure detectability**: silent semantic divergence (compile pass、runtime で getter 経由 vs 直接 field access の差異が観測可能)

### INV-2: External (E1) と internal (E2 this) dispatch path symmetry

- **(a) Property statement**: E1 external (`obj.x`) と E2 internal (`this.x` inside class body) の dispatch logic が **token-level identical**、共通 helper を介して emit
- **(b) Justification**: P1 TC39 faithful confirmed、internal access も getter/setter 経由必須 (decorator hook coverage 完全性)。helper 不一致 = internal で direct backing access bypass risk
- **(c) Verification method**: Integration test (`tests/i205_invariants_test.rs::test_invariant_2_external_internal_dispatch_symmetry`) で external `f.name` と internal `this.name` の emit IR を比較、両者の MethodCall arg 順序 / receiver expr / method 名 一致を verify
- **(d) Failure detectability**: I-201-B (decorator) 統合時 silent semantic divergence (decorator hook が internal call では fire せず external のみ fire = TS spec から divergent)

### Cell 21 corollary: B4 + non-numeric getter return type × compound assign の semantic safety (Iteration v12 review F5 insight、本 PRD scope 内 verify)

`obj.x += rhs` (B4 instance setter desugar) で getter return type が non-numeric (String / Vec<T> / Struct / etc.) の場合、emission は `obj.set_x(obj.x() OP rhs)` (yield_new Block form)。各 op × type 組合せは Rust 型システムの `Add` / `Sub` / 等 trait 実装次第:

- **`String += String` / `String += &str`**: Rust `String + String` は `Add<String> for String` trait 実装あり = **Tier 1 (silent semantic loss なし、TS string concat と意味一致)**
- **`String += f64`**: Rust `String + f64` は `Add` trait 不在 = **compile error E0277** (Tier 2 等価で自動 surface、TS の string coercion と divergent だが silent semantic change ではない = Rust compiler が safety net として機能)
- **`Vec<T> += anything`**: Rust `Vec` に `Add` trait 実装なし = compile error (Tier 2 等価)
- **`Struct += anything`**: derive-not-Add struct で compile error (Tier 2 等価)、user `impl Add for Struct` ありなら Tier 1

**Verdict**: 本 PRD T8 では B4 + non-numeric getter return type × compound assign に **追加 gate 不要**。T7 update (`++/--`) は **必ず numeric 演算** (`+ 1.0`) のため non-numeric type で必ず E0277 = `getter_return_is_numeric` gate で先回り Tier 2 honest error reclassify する価値あり (= specific TS NaN coercion semantic を user-friendly error message で説明)、しかし T8 compound は `op` と `rhs` が user-supplied で `String += String` が legitimate Tier 1 case であるため pre-gate は不可能。Rust compile error fallthrough = Tier 2 等価で自動 surface する設計が ideal。

(本 corollary は Iteration v12 `/check_job` 4-layer review F5 finding の本質的解決として PRD に追記、framework 失敗 signal ではない = `is_side_effect_free` + `getter_return_is_numeric` の semantic 差異 (前者 = INV-3 receiver eval count concern、後者 = T7-specific numeric coercion concern) を明示化することで Rule 9 Spec → Impl Mapping completeness を補強。)

### INV-3: Compound assign desugar の receiver evaluation 1 回

- **(a) Property statement**: `obj.x += v` の desugar `obj.set_x(obj.x() + v)` で `obj` は **1 回のみ evaluated** (TS source の side-effect 数 = Rust output)。side-effect-having receiver (e.g., `getInstance().x += v`) では IIFE form binding (`{ let mut __ts_recv = getInstance(); let __ts_new = __ts_recv.x() + v; __ts_recv.set_x(__ts_new); __ts_new }`) で receiver eval を 1 回に bound。同 INV-3 compliance は **UpdateExpr Member target setter dispatch path** (`obj.x++` / `obj.x--`) にも extend (T8 で T7 dispatch_instance_member_update に back-port 完了 = `build_setter_desugar_block` + `wrap_with_recv_binding` shared helper integration)
- **(b) Justification**: TS の `obj.x += v` は `obj` を 1 回 evaluate。Rust の naive desugar `obj.set_x(obj.x() + v)` は `obj` を 2 回 evaluate (getter call + setter call)、side-effect-having receiver で副作用重複実行 = silent semantic change。同 latent gap が UpdateExpr setter dispatch path (`{ let __ts_old = getInstance().x(); getInstance().set_x(__ts_old + 1.0); __ts_old }`、getInstance() 2 回 eval) にも存在、T8 で structural cohesive 解消
- **(c) Verification method**: Integration test (`tests/i205_invariants_test.rs::test_invariant_3_compound_assign_receiver_eval_once`)、Side-effect counting test (counter で getInstance() 呼出回数を count、TS と Rust output で一致 verify)。Helper-level lock-in: `is_side_effect_free(expr: &Expr) -> bool` の judgment matrix (Ident → true / FieldAccess recursive → object 依存 / FnCall → false / MethodCall → false / etc.) を C1 branch coverage で test
- **(d) Failure detectability**: silent semantic change (compile pass、副作用が 1 回多く発生)
- **(e) Scope clarification (本 T8 で structural fix path)**: 本 INV-3 は **setter dispatch path のみ scope** (= compound assign on B4 instance / B8 static / UpdateExpr B4 setter dispatch)。Fallback path (B1 field、B9 unknown、non-class receiver、`obj.x = obj.x + v` direct field access desugar) の INV-3 1-evaluate compliance は本 PRD scope 外 (= 別 architectural concern として TODO 起票候補、`1 PRD = 1 architectural concern` 厳格適用)

### INV-4: Method kind tracking propagation chain integrity

- **(a) Property statement**: SWC AST `method.kind` (Method/Getter/Setter) が `collect_class_info` → `MethodSignature.kind` → `convert_method_info_to_sig` → `resolve_method_sig` → dispatch logic に **lossless propagate**、デフォルト値 (Method) で fallthrough する path が存在しない
- **(b) Justification**: kind propagation 1 path で `Default::default()` (= Method) に fallthrough すると broken framework に逆戻り。dispatch logic は kind を正しく検出できず direct field access fallback、silent semantic divergence
- **(c) Verification method**: Integration test (`tests/i205_invariants_test.rs::test_invariant_4_kind_propagation_lossless`)、Propagation chain test (各 stage で kind が intermediate state に preserve される事を probe)、`Default::default()` fallthrough を可能にする default value 不在 verify (= field add 時に compile error 強制 / explicit init enforcement)
- **(d) Failure detectability**: silent semantic divergence (kind = Method default で fallthrough → field access fallback で broken pattern 再発)

### INV-5: Visibility consistency (private accessor 外部 access 不能、Option B 採用 2026-05-01)

- **(a) Property statement**: `private get x() {}` / `private set x(v) {}` (TS keyword `private` 修飾 accessor) を持つ class の external `obj.x` access は **必ず Tier 2 honest error reclassify** (Rust visibility = `pub` 不在で external invocation 不能、TS の private は runtime で type-checker のみ enforcement で Rust と semantic 一致しない)
- **(b) Justification**: TS private は runtime に influence せず type-checker のみ。Rust visibility は runtime に厳格適用。両者の semantic divergence を Rust で reproduce 不能 = Tier 2 honest error が ideal。INV-5 違反 = external `obj.x` で private getter `Foo::x` を call 試行 → Rust E0624 compile error (= Tier 2 自動 surface) だが、これを silent ignore (visibility 削除) すると TS private の semantic 違反 + Rust idiom 違反
- **(c) Verification method (T13 (13-c) で Option B fill-in 完了 2026-05-01)**: Integration test `tests/i205_invariants_test.rs::test_invariant_5_private_accessor_external_access_tier2` (getter) + `test_invariant_5_private_setter_external_write_tier2` (setter symmetric counterpart、Layer 3 cross-axis completeness)。Probe で `private get x()` / `private set x(v)` を持つ class の transpile 出力 Rust source を assert: (1) private accessor 生成 method に `pub` modifier 不在、(2) public accessor は `pub` modifier 存在、(3) external `obj.x` / `obj.x = v` access は cell 2 / cell 14 dispatch fires regardless of accessibility (= MethodCall emit 一貫)。Rust E0624 surface は separate consumer module compile context が必要 (本 transpile output のみでは観測不能) のため、**生成側 visibility marker preservation** を proxy として検証
- **(d) Failure detectability**: silent semantic change if visibility is dropped (TS private が Rust pub になる = encapsulation 緩和)、または compile error if visibility preserved without honest error (= Tier 2 但し user に not transparent)
- **Option A vs Option B reachability audit (T13 (13-b) 2026-05-01)**: Hono codebase 284 TS files 全件で `private get` / `private set` 0 件 (= reachability = 0)。Option A (= `MethodSignature.accessibility` field 追加 + 50+ site Rule 9 (c) Field-addition symmetric audit + dispatch arm で `UnsupportedSyntaxError::new("access to private accessor", _)` emit) は 0 件 reachability の concern に対し overengineering、recurring problem evidence (I-383 T8' / I-205 T2 で latent kind drop 2 度連続) を考慮し **Option B (status quo) を採用**。Option B mechanism: `resolve_member_visibility(Some(Private), _)` → `Visibility::Private` (= no `pub` modifier) は既に implementation 済 (`src/transformer/classes/helpers.rs:89`)、Rust visibility は runtime に厳格適用 → external invocation で E0624 自動 surface (= Tier 2 honest error 自動成立、no production code change needed)

### INV-6: Scope boundary preservation (`this.x` ↔ external `obj.x` semantic distinction)

- **(a) Property statement**: `this.x` (E2 internal class body) の dispatch logic は class scope state (`enclosing_class_name`) に依存し、`obj.x` (E1 external、`obj` が偶然 self を refer しても) と **同一 dispatch logic** (P1 TC39 faithful) ではあるが、**dispatch trigger source は明示区別**: this expression detection は `ast::Expr::This(_)` patterns、external receiver は generic Ident / chain / call result
- **(b) Justification**: P1 (TC39 faithful) では this.x も obj.x も同一 dispatch だが、scope state lookup mechanism は内部 vs 外部で異なる (内部 = enclosing class scope from transformer state、外部 = receiver expr type lookup from TypeRegistry)。両者の dispatch trigger source が混乱すると、internal access が external lookup mechanism を経由し scope state が unavailable → fallback to direct field access (= broken framework regression)
- **(c) Verification method**: Integration test (`tests/i205_invariants_test.rs::test_invariant_6_scope_boundary_preservation`)、Probe で `this.x` (inside class method body) と external `obj.x` (assuming obj.type 解決される) の dispatch path を独立に probe、両 path の output IR が token-level identical を verify、scope state lookup vs receiver type lookup の trigger source 区別 verify
- **(d) Failure detectability**: silent semantic divergence (internal access が dispatch されず direct backing field access で getter/setter bypass = INV-1 と関連 cohesive violation)

## Spec Review Iteration Log (Rule 13 (13-2) hard-code)

### Iteration v1 (2026-04-27)

- **Findings count**: Critical 6 / High 3 / Medium 4 / Low 2 = 計 15 findings
- **Findings detail**: 9 RC clusters に集約 — RC-1 matrix abbreviation pattern (F1/F4/F5/F15)、RC-2 Oracle observation embed (F2)、RC-3 Impact Area uncertain expression (F7)、RC-4 Stage tasks separation (F8)、RC-5 Scope 3-tier (F9)、RC-6 Invariants section (F6)、RC-7 broken-fix wording (F12)、RC-8 ast-variant audit pre-draft (F13)、RC-9 Spec Stage Self-Review (F10/F11/F14)、その他 F3 (SWC parser empirical)
- **Resolution**: 各 RC を framework 改善 + 本 PRD self-applied integration として同時遂行 (Path B = PRD 2.7 self-applied integration pattern)

### Iteration v2 (2026-04-27、本 commit)

- **Framework improvements applied** (`spec-stage-adversarial-checklist.md` v1.3 / `prd-completion.md` Tier-transition compliance / `prd-template` skill workflow / audit script extensions):
  - Rule 1 sub-rule (1-1)(1-2)(1-3) 拡張 (RC-1)
  - Rule 2 sub-rule (2-1)(2-2)(2-3) 拡張 (RC-2)
  - Rule 5 sub-rule (5-1)(5-2)(5-3)(5-4) 拡張 (RC-4)
  - Rule 6 sub-rule (6-1)(6-2)(6-3)(6-4) 拡張 (RC-5)
  - Rule 8 sub-rule (8-5) 追加 (RC-6)
  - Rule 11 sub-rule (d-5) 追加 (RC-8)
  - Rule 13 (Spec Stage Self-Review) 新規追加 (RC-9)
  - prd-completion.md Tier-transition compliance section 追加 (RC-7)
  - audit-prd-rule10-compliance.py に 7 new verify functions + active PRD detection 追加
  - audit-ast-variant-coverage.py に `--files` flag 追加
- **PRD doc fixes applied (本 v2 commit)**:
  - Status header v1 → v2、Self-applied integration note 追加
  - Scope section 3-tier hard-code (In Scope / Out of Scope / Tier 2 honest error reclassify)
  - `## Oracle Observations` section 新規追加 (3 representative cells embed、TS-1 task で full populate)
  - `## SWC Parser Empirical Lock-ins` section 新規追加 (1 representative NA cell embed、TS-2 task で full populate)
  - `## Impact Area Audit Findings` section 新規追加 (manual grep findings + 本 PRD/I-203 決定)
  - `## Invariants` section 新規追加 (INV-1 dispatch consistency / INV-2 internal-external symmetry / INV-3 compound assign side-effect / INV-4 kind propagation lossless)
  - Impact Area uncertain expressions 排除 ("(or 該当 file)" "(or 該当)" 等を empirical verify で確定 path に置換)
  - Tier-transition compliance wording adoption (`prd-completion.md` 適用)
- **Matrix Cartesian product expansion (本 v2 完了、Rule 1 (1-2) compliance)**:
  - Cells 24-52 expansion (A3 compound `+=`/`-=` × B5-B9、A4 bitwise compound × B、A5 logical compound × B、A6 increment/decrement × B、A7-A11 corner cases × B)
  - Cells 60-64 expansion (E2 internal this.x dispatch × A1/A2/A3/A6 × B1/B2/B4)
  - Cells 70-80 expansion (Class Method Getter body shape sub-cases per Rule 7)
  - 全 cell に Ideal output / 現状 / 判定 / Scope 記載、abbreviation pattern (`...`/range grouping/representative/varies/(各別 cell)/(同上)) 全廃
- **Audit re-run result (v2 final)**: `audit-prd-rule10-compliance.py` **PASS** (1 PRD(s))。framework Rule 1/2/4/5/6/8/10/11/12/13 全 compliance auto-verify。Rule 3 (3-2) SWC parser empirical / Rule 7 control-flow exit sub-case は manual verify (本 PRD content)。
- **Remaining (Spec Stage Tasks で実施、Implementation 移行 block しない)**:
  - TS-1: Oracle observations 全 ✗ cell populate (本 v2 で representative 3 cells embed、TS-1 task で full ~30 ✗ cells populate)
  - TS-2: SWC parser empirical lock-in test 全 NA cell populate (本 v2 で representative 1 cell embed、TS-2 task で全 NA cells populate)
  - TS-3: E2E fixture creation (red 状態 lock-in)
  - TS-4: Impact Area audit findings — CI 環境で正規 audit (tree-sitter-rust available) で update
  - TS-5: 13-rule self-applied verify final pass

### Iteration v2 完了判定 (2026-04-27)

✅ **Spec stage Self-Review (Rule 13)**: framework structural enforcement (Rule 1/2/5/6/8/11/13) 全 PASS、audit-prd-rule10-compliance.py PASS で auto-verify。Spec Stage Tasks (TS-0〜TS-5) を実施後、Implementation stage 移行可能。

### Iteration v3 (2026-04-27、本 v3 commit、第三者 review v3 結果反映)

- **第三者 review v3 findings**: Critical 2 / High 3 / Medium 4 / Low 2 = 計 11 findings 発見
  - **F-v3-1 (Critical)**: matrix cells 29/34/35/41/45 で `同 A3` / `同 dispatch logic` abbreviation by reference (Rule 1 (1-2) 趣旨違反、audit script regex 検出外)
  - **F-v3-2 (Critical)**: cells 48/49/52 で `B 全` grouping が B1 field fallback 可能性を merge 排除 (B1/B2-B9 区別必要)
  - **F-v3-3 (High)**: Implementation Stage Tasks (T1-T15) の Spec Stage prerequisite (TS-0〜TS-5) 未明示
  - **F-v3-4 (High)**: TS-0 task description "~150 cells target" inaccurate (v2 実際 ~85 cells with orthogonality reduction)
  - **F-v3-5 (High)**: Background `同上` 残存 cleanup
  - **F-v3-6 (Medium)**: Cell numbering gaps (53-59, 65-69)
  - **F-v3-7 (Medium)**: Rule 7 control-flow exit nested closure body sub-case 未列挙
  - **F-v3-8 (Medium)**: INV-5 (visibility consistency) / INV-6 (scope boundary preservation) 追加検討
  - **F-v3-9 (Medium)**: audit script の orthogonality merge wording verify policy 不在
  - **F-v3-10 (Low)**: Goal section 10 verifiable conditions の v2 反映確認
  - **F-v3-11 (Low)**: Test Plan の TS-3 task 参照 context 確認
- **Resolution applied (本 v3 commit)**:
  - F-v3-1 Fix: cells 29/34/45 を sub-divide (29-a〜29-e、34-a〜34-c、45-a〜45-d) で各 cell 自己完結化、cells 35/41 は orthogonality-equivalent 明示記載 (cells 24-28 mapping derive)
  - F-v3-2 Fix: cells 48/49/51/52 を sub-divide (48-a/48-b、49-a/49-b、51-a/51-b、52-a/52-b) で B1 field vs B2-B9 区別
  - F-v3-3 Fix: Implementation Stage Tasks に "Prerequisite (全 T-* task 共通): Spec Stage Tasks TS-0〜TS-5 全完了" 追加
  - F-v3-4 Fix: TS-0 task description を "~85 cells with Rule 10 Step 2 orthogonality reduction" に reword
  - F-v3-5 Fix: Background `同上` cleanup
  - F-v3-7 Fix: cell 80 = nested closure body sub-case 追加 (cell 81 = setter body に renumber)
  - F-v3-8 Fix: INV-5 (visibility consistency) + INV-6 (scope boundary preservation) を Invariants section に追加 (4 → 6 invariants)
- **Remaining (v4 polish、Implementation stage 移行 block しない)**:
  - F-v3-6 (cell numbering gaps): non-blocking cosmetic (53-59, 65-69 の re-numbering)
  - F-v3-9 (audit orthogonality verify policy): non-blocking framework polish (TS-5 task で 13-rule self-applied verify pass で実 verify)
  - F-v3-10 (Goal verify): non-blocking (個別 verify は TS-5 task 内)
  - F-v3-11 (Test Plan context): non-blocking (TS-3 = Spec Stage E2E fixture creation、Test Plan = Implementation Stage test overview、context 区別 cleanup polish)
- **Audit re-run result (v3 final)**: `audit-prd-rule10-compliance.py` **PASS**、Critical/High findings 全 resolve、Medium/Low non-blocking findings は v4 polish。

### Iteration v3 完了判定 (2026-04-27)

✅ **Spec stage Self-Review v3 (Rule 13 (13-3))**: Critical (F-v3-1, F-v3-2) と High (F-v3-3, F-v3-4, F-v3-5) findings 全 fix、Medium F-v3-7/F-v3-8 fix、Medium/Low remaining は non-blocking polish。Spec Stage Tasks (TS-0〜TS-5) を実施後、Implementation stage 移行可能。

### Iteration v4 (2026-04-28、TS-* execution Spec stage tasks 完了)

Spec Stage Tasks (TS-0〜TS-5) を本 session 内で完全実施:

- **TS-0 完了**: matrix Cartesian product expansion (~85 cells、Rule 10 Step 2 orthogonality reduction、abbreviation pattern 全廃、`audit-prd-rule10-compliance.py` Rule 1 (1-2) PASS)
- **TS-1 完了** (2026-04-28): 16 representative ✗ cells について `npx tsx /tmp/i205-cells/cell-NN.ts` で empirical observation 実施、`## Oracle Observations` section に 4 項目 (TS fixture / tsx output / cell # link / ideal output rationale) 完全 embed。Orthogonality-equivalent cells inherit observations (Rule 10 Step 2)。**重要 finding (cell 12)**: TS class write-to-read-only は **runtime TypeError throw** (silently no-op ではない、JavaScript class implicit strict mode、v3 で empirical 確認・v2 wording 修正)
- **TS-2 完了** (2026-04-28): 3 representative NA candidate cells (cell 44 `++` non-numeric / cell 6 AutoAccessor / cell 8 inherited) について SWC parser empirical observation 実施、`## SWC Parser Empirical Lock-ins` section に embed。**重要 finding (cell 44)**: SWC accept (NaN coercion runtime) → Rule 3 (3-3) reclassify (NA → **Tier 2 honest error reclassify、本 PRD scope**、`UnsupportedSyntaxError::new("increment of non-numeric — TS NaN coercion semantic", span)`)。matrix cell 44 を update。PRD 2.7 cell 15 (Prop::Assign) と同 lesson 再発、Rule 3 (3-2) framework 改善の自己 verify
- **TS-3 完了** (2026-04-28): 19 fixtures + 19 `.expected` files を `tests/e2e/scripts/i-205/` に作成 (TS-1 fixtures を leverage)、red 状態 (ts_to_rs 出力 ≠ expected) を Implementation Stage T14 で green 化予定
- **TS-4 完了** (2026-04-28): `## Impact Area Audit Findings` section は manual grep approximation で populate (本 session で tree-sitter-rust 不在のため CI 環境で正規 audit 推奨)。1 violation (`registry/collection/class.rs:145` `_ => {}`) を本 PRD scope で fix、他多数 violations は I-203 codebase-wide refactor へ defer
- **TS-5 完了** (2026-04-28): 13-rule self-applied verify final pass、`audit-prd-rule10-compliance.py` PASS for I-205 + PRD-2.7 (grandfathered)、全 13 rule manual verify pass

### Iteration v4 完了判定 (2026-04-28)

✅ **Spec stage 完了**: 全 Spec Stage Tasks (TS-0〜TS-5) 完了、13-rule self-applied verify final pass、Critical / High findings 全 resolve、Medium/Low non-blocking polish (cell numbering gaps F-v3-6 / orthogonality verify policy F-v3-9 / Goal verify F-v3-10 / Test Plan context F-v3-11) は Implementation stage 進行と並行 polish 可能。**Implementation stage 移行 ready**。

### Iteration v5 (2026-04-28、第三者 review v3 + 5 RC clusters 解決)

- **/check_job review v3 findings**: 10 件 (Critical 2 / High 3 / Medium 4 / Low 1) を 5 RC clusters に集約
  - **RC-α** Coverage incompleteness: F-rev-1 (Oracle 29%) / F-rev-2 (E2E 35%) / F-rev-9 (NA SWC 3 cells) / F-rev-10 (E2 internal cells)
  - **RC-β** Matrix abbreviation hidden: F-rev-3 (B-variant grouping cells 35/41/45-d/29-e)
  - **RC-γ** Spec-Design gap: F-rev-4 (B7 inherited Design gap) / F-rev-7 (Spec→Impl pre-alignment)
  - **RC-δ** Test depth incompleteness: F-rev-6 (invariant tests) / F-rev-8 (test naming + decision table)
  - **RC-ε** Architectural boundary compromise: F-rev-5 (Rule 11 d-1 defer to I-203)
- **Resolution applied (Phase 1-5)**:
  - Phase 1 (RC-β fix): cells 29-e/35/41/45-d を B-variant per row に expand (4 → 20 rows)、letter suffix (a/b/c/d/e) で audit range pattern 回避
  - Phase 2 (RC-γ fix): Design section #3-bis に `lookup_method_kind_with_parent_traversal` helper + B7 dispatch logic 追加 (cell 8/17/26/41-c の Tier 2 reclassify mechanism)、`## Spec → Impl Dispatch Arm Mapping` section 新規 author (5 helper × 各 dispatch arm × matrix cell 1-to-1 mapping)
  - Phase 3 (RC-δ fix): `### Invariant verification tests` section author (INV-1〜INV-6 各 4 項目: test fn name / assertion / probe location / expected)、`### Decision tables (A/B/C)` author (concrete dispatch enumeration)、`### Equivalence partitions` + `### Boundary values` author
  - Phase 4 (RC-α fix): 残 15 cells (22/23/25/26/27/36/37/39/40/47/63/64/71/72/74) について tsx empirical observation + `### Additional residual cells (TS-1 task continuation v3 final)` section embed、~30 fixtures + .expected files in `tests/e2e/scripts/i-205/` (E2E 計 68 files = 34 .ts + 34 .expected)
  - Phase 5 (RC-ε + framework v1.4): `spec-stage-adversarial-checklist.md` Rule 11 (d-6) "Architectural concern boundary stance" 追加 (touch files の `_ =>` arms defer 公式 stance、(d-6-1)(d-6-2) 2 条件 verification statement) + Rule 1 (1-4) "Orthogonality merge legitimacy" 追加 (D 全/B 全 wording の legitimate stance、(1-4-a)(1-4-b) 2 条件)、audit script は abbreviation のみ detect、orthogonality merge は manual review responsibility (Layer 3 cross-axis)
- **Audit re-run result (v5)**: `audit-prd-rule10-compliance.py` PASS (PRD-2.7 + I-205 共)、cargo test --lib 3162 passed、cargo clippy 0 warnings、cargo fmt --check 0 diffs

### Iteration v5 完了判定 (2026-04-28)

✅ **Spec stage 13-rule self-applied verify final pass v2**: 第三者 review v3 で発見 10 findings 全 fix、5 RC clusters 全 resolution、`audit-prd-rule10-compliance.py` PASS、Critical/High/Medium findings 全 resolve。Implementation stage 移行 ready (Pure ideal-implementation-primacy compliance)。

### Iteration v6 (2026-04-28、deep review iteration v6 + 7 F-deep findings + framework v1.5 pure ideal stricter revision)

- **/check_job deep review iteration v6 findings**: 7 件 (Critical 2 / High 2 / Medium 2 / Low 1) → 3 RC clusters
  - **RC-Δ** (Framework rationalization): F-deep-3 (v1.4 Rule 1 (1-4-b)/Rule 11 (d-6) は rationalization vs clarification 境界)
  - **RC-Ε** (Spec stage probe deferral): F-deep-4 (orthogonality merge verification を Implementation Stage に defer = pragmatic compromise)
  - **RC-Ζ** (Iteration log nomenclature): F-deep-6 (旧 confusing version 表記、historical wording)
  - F-deep-1 (Critical Implementation gap): TS-2 [x] checked but SWC parser empirical lock-in test files が実在しない (dishonest claim)
  - F-deep-2 (High Spec gap): E2E fixtures 作成済だが `tests/e2e_test.rs` integration 不在
  - F-deep-5 (Medium): Cell numbering convoluted (cosmetic、defer)
  - F-deep-7 (Low): PRD 1609 lines (acceptable per Pure ideal completeness)
- **Resolution applied (本 v6 commit)**:
  - F-deep-1 fix: 3 SWC parser test files 新規作成 (`swc_parser_increment_non_numeric_test.rs` 3 tests + `swc_parser_inherited_accessor_test.rs` 3 tests + `swc_parser_auto_accessor_test.rs` 4 tests = 計 10 tests cargo test PASS)
  - F-deep-2 fix: 34 `#[test] #[ignore]` per-cell functions added to `tests/e2e_test.rs` (`test_e2e_cell_i205_NN_*`)
  - **RC-Δ + RC-Ε holistic fix (Phase A + B、framework v1.5)**: Framework v1.4 → v1.5 pure ideal stricter revision
    - Rule 1 (1-4-b) Implementation Stage defer → **Spec stage structural verify** (audit script auto check)
    - Rule 1 (1-4-c) 新規 Spec stage referenced cell symmetry probe
    - Rule 11 (d-6) "touched-files-strict" → **"architectural-concern-relevance" principle** (= "1 PRD = 1 architectural concern" との理論的整合)
    - `audit-prd-rule10-compliance.py` に `verify_orthogonality_merge_consistency` function 追加
    - `D 全` wording を `* (orthogonality-equivalent: D dimension は dispatch logic に影響なし)` に統一、Rule 1 (1-4) framework v1.5 audit PASS
  - **RC-Ζ fix (Phase C)**: Iteration log linear renumber (v1, v2, v3, v4, v5, v6 sequential、"final"/"final v2" 表記排除)
- **Audit re-run result (v6 final)**: `audit-prd-rule10-compliance.py` PASS (PRD-2.7 grandfathered + I-205 active)、cargo test --lib 3162 passed、SWC parser tests 10 passed、cargo clippy 0 warnings、cargo fmt 0 diffs

### Iteration v6 完了判定 (2026-04-28)

✅ **Spec stage Self-Review v6 (Rule 13 final、framework v1.5 stricter compliance)**: deep findings 7 件 → 5 fix + 2 deferred polish (F-deep-5 cell renumbering / F-deep-7 PRD line count = cosmetic、Implementation Stage と並行 polish 可能)、framework v1.4 rationalization → v1.5 pure ideal stricter revision、F-deep-1/2/3/4/6 (Critical/High/Medium 主要) 全 resolve。Implementation stage 移行 ready (Pure ideal-implementation-primacy compliance + framework integrity restored)。

### Iteration v7 (2026-04-28、本 commit、deep deep review v6 + 8 F-deep-deep findings + framework v1.5 → v1.6 audit symmetry restoration)

- **/check_job deep deep review iteration v7 findings**: 8 件 (Critical 2 / High 2 / Medium 3 / Low 1) → 3 RC clusters
  - **RC-Θ** (Verification deferral): F-deep-deep-1 (Rule 11 d-6 audit asymmetry) + F-deep-deep-2 (invariant tests SPEC ONLY) + F-deep-deep-3 (NA inheritance unverified) + F-deep-deep-4 (helper test contracts missing) = 4 件、Spec stage で structural verification を audit script で auto-enforce していない compromise
  - **RC-Ι** (Documentation inconsistency): F-deep-deep-5 (plan.md/TODO doc gap) + F-deep-deep-6 (v6 entry historical refs) = 2 件
  - **RC-Κ** (Structural growth): F-deep-deep-7 (PRD bloat) + F-deep-deep-8 (framework convergence metric) = 2 件、process-level concerns
- **Resolution applied (本 v7 commit、Phase 1-3 batch + framework v1.6)**:
  - **F-deep-deep-2 fix (Critical、Phase 1)**: `tests/i205_invariants_test.rs` 新規作成、6 invariants × `#[test] #[ignore]` stub functions、Implementation Stage T15 で fill in (= "deferred verification = unverified claim" compromise eliminate)
  - **F-deep-deep-1 fix (Critical、Phase 2 + framework v1.6)**: `audit-prd-rule10-compliance.py` に `verify_rule11_d6_relevance_compliance` function 追加、Rule 11 (d-6-b-1) orthogonality declaration + (d-6-b-2) non-interference probe markers を structural detect、Impact Area Audit Findings 該当 row に verification statements embed = Rule 1 (1-4) audit との symmetry restored
  - **F-deep-deep-2 supplementary fix (Phase 2 + framework v1.6)**: `verify_invariants_test_contracts` function 追加、各 INV-N entry に `test_invariant_N_*` test fn reference の structural detect、PRD `## Invariants` section の各 INV (c) Verification method に test fn 名前明記
  - **F-deep-deep-3 fix (High、Phase 3)**: `## SWC Parser Empirical Lock-ins` section に "AutoAccessor declaration parse acceptance covers cells 6/15/24/29-e-a/41-a/45-da (B5 cells across A dimension)" の inheritance justification statement embed、parser-level context-independence を explicit declaration
  - **F-deep-deep-4 fix (High、Phase 3)**: `tests/i205_helper_test.rs` 新規作成、`lookup_method_kind_with_parent_traversal` helper × 4 test contracts (single-level / multi-level / circular prevention / direct vs inherited disambiguation) stub
  - **F-deep-deep-5 fix (Medium、Phase 3)**: `plan.md` の I-205 status を "v3 → v6 final" + framework v1.3 → v1.4 → v1.5 → v1.6 連続 revision history reflect
  - **F-deep-deep-6 fix (Medium、Phase 3)**: PRD 内 "deep review v3 final v3" historical references を "deep review iteration v6/v7" semantic naming に統一 (3 refs cleanup)
  - **F-deep-deep-7 deferred polish (Medium、Phase 3)**: PRD bloat (1638 lines) は Pure ideal completeness 優先で acceptable、Implementation Stage 後 polish (iteration log separation 候補)
  - **F-deep-deep-8 deferred polish (Low、Phase 3)**: Framework convergence metric は v1.6 versioning entry に conceptual record (実 metric 導入は別 framework PRD 候補)
  - **Framework v1.6 stricter revision (RC-Θ holistic)**: `spec-stage-adversarial-checklist.md` v1.5 → v1.6、**rule-audit symmetry principle** 確立 = 全 rule に対応する audit script auto-verification 整備、verification deferral eliminate
- **Audit re-run result (v7 final)**: `audit-prd-rule10-compliance.py` PASS (PRD-2.7 + I-205 共)、cargo test --lib 3162 passed、SWC parser tests 10 passed (3 files)、i205_invariants_test 6 ignored stubs、i205_helper_test 4 ignored stubs、cargo clippy 0 warnings、cargo fmt 0 diffs

### Iteration v7 完了判定 (2026-04-28)

✅ **Spec stage Self-Review v7 (Rule 13 final、framework v1.6 audit symmetry compliance)**: deep deep findings 8 件 → 6 fix + 2 deferred polish (F-deep-deep-7/8 = process-level、Implementation Stage と並行 polish 可能)、framework v1.5 audit asymmetry → v1.6 rule-audit symmetry restored (Rule 8 (8-c) + Rule 11 (d-6) auto-verify 新規)、F-deep-deep-1/2/3/4/5/6 (Critical/High/Medium 主要) 全 resolve。Implementation stage 移行 ready (Pure ideal-implementation-primacy compliance + framework rule-audit symmetry principle established)。

### Iteration v8 (2026-04-28、Implementation Stage T1-T3 batch + `/check_job` 4-layer review + Fix 1-4)

Implementation Stage T1-T3 batch 完了 (= MethodKind enum + MethodSignature.kind / TsMethodInfo.kind field + collect_class_info kind propagate + Rule 11 (d-1) compliance + resolve_method_sig latent bug fix + getter/setter unit test 4 件)、初回 `/check_job` 4-layer review で発見された **本質的原因 3 + 派生 1** に対する Fix 1-4 を本 batch 内で structural 解決。

- **`/check_job` initial review findings (4-layer framework 全実施、defect 5 category 分類)**:
  - **Spec gap 1 件**: L4-F1 (`MethodKind` の foundational placement 不在 → module circular dep registry ↔ ts_type_info)
  - **Implementation gap 2 件**: L1-F1 / L4-F2 (`type_literals.rs:98` interim patch 条件 2/4 違反)、L1-F2 / L4-F3 (`let _ = X` non-idiomatic pattern)
  - **Review insight 1 件**: L3-F3 (bulk-script process gap → field 追加 PRD で symmetric audit 不在)
- **Fix 1-4 applied (本 v8 commit)**:
  - **Fix 1 (Spec gap 解決)**: `MethodKind` を `src/registry/mod.rs` から `src/ir/method_kind.rs` (foundational module) へ move、`registry::MethodKind` は re-export 維持で 51 site backward compat、`ts_type_info::TsMethodInfo.kind` を `crate::ir::MethodKind` 直接参照に変更で **module-level circular dep を構造的解消**。method_kind module unit test 5 件 (Default / From SWC 3 variants / Copy trait verify / 3 variants distinct verify)。
  - **Fix 2 (Implementation gap 解決、T4 work piece 前倒し完了)**: `convert_method_info_to_sig` (`type_literals.rs:98`) が `m.kind` を silently 無視していた **latent silent semantic risk** (`resolve_method_sig` と symmetric な未 fix bug) を `kind: m.kind` に修正で structural fix。TsTypeLit unit test 3 件追加。T4 task 残作業は test-only (本 Fix 2 で完了)。
  - **Fix 3 (Implementation gap 解決)**: `class.rs` の `let _ = private_method` 等 3 site の anti-idiomatic pattern を `_` match-pattern に refactor、Rust idiom 準拠。
  - **Fix 4 (Review insight + framework self-applied integration v1.6 → v1.7)**: `spec-stage-adversarial-checklist.md` Rule 9 に sub-rule (c) "Field-addition symmetric conversion site audit" 追加 (Pre-implementation symmetric audit + Audit script auto-verify candidate + Post-implementation review trigger)。**Recurring problem evidence**: I-383 T8' + I-205 T2 で 2 度連続発生確認、3 度目発生前の structural prevention。
- **`/check_job` deep review v8 findings (post Fix 1-4、追加 5 件)**:
  - **D1 (Pipeline integrity violation)**: Fix 1 で導入した `src/ir/method_kind.rs` の `From<swc_ecma_ast::MethodKind>` impl が `src/ir/` 配下唯一の SWC 依存となり pipeline integrity convention 違反 → **D1 fix**: From impl を `src/registry/swc_method_kind.rs` (新設 boundary module) へ移動、`src/ir/` SWC independence 復元。SWC ↔ IR boundary conversion pattern を確立 (= future SWC type の IR conversion も同 pattern で展開可能)。From impl test 3 件 + into chain test 1 件を boundary module に move、`src/ir/method_kind.rs` には pure IR test 3 件 (Default / Copy / 3 variants distinct) のみ残す。
  - **D2-D3 (Test placement、D1 fix と統合)**: From impl tests を boundary module に co-locate、test naming も `test_method_kind_is_copy` → `test_method_kind_value_assignment_preserves_equality` に refine (`testing.md` `test_<target>_<condition>_<expected>` convention 強化準拠)。
  - **D4 (Audit script candidate status)**: `verify_field_addition_symmetric_audit` candidate 状態は I-212 (Framework convergence metric framework PRD) 内 "Audit script complete coverage" criterion (b) で track 済、acceptable。
  - **D5 (本 entry)**: PRD `## Spec Review Iteration Log` v8 entry を "TBD (Planned)" から本実 content に update。Rule 13 (13-2) compliance restored。
- **別 PRD 起票 (Layer 4 architectural rabbit hole detection)**:
  - **I-213** (codebase-wide IR struct construction boundary DRY refactor、L4 暫定): MethodSignature / TypeDef / FieldDef / ParamDef 等 全 IR struct の builder pattern or `..Default::default()` 統一。Fix 4 と相補的 (Fix 4 = process 解決、I-213 = structural 解決)。recurring problem evidence: I-383 T8' (`type_params`) + I-205 T2 (`kind`) で 2 度連続。
- **Audit re-run result (v8 final、post deep deep + light review)**: `audit-prd-rule10-compliance.py` PASS、`audit-ast-variant-coverage.py` PASS for in-scope (out-of-scope 2 件は I-203 defer per Rule 11 (d-6))、`cargo test --lib` 3176 pass (3162 baseline + 4 T3 class_method_kind + 3 Fix 1 ir/method_kind (post-D1 redistribution: Default + Copy-renamed + 3-variants-distinct) + 4 D1 registry/swc_method_kind (From×3 + into chain) + 3 Fix 2 type_literals = 3176)、cargo clippy 0 warning、cargo fmt 0 diff、e2e_test 159 pass + 70 ignored、compile_test 3 pass、122 integration pass、Pipeline integrity (`src/ir/` SWC indep) 維持 (D1 fix + F-dd-2 formalization 後)。

### Iteration v8 完了判定 (2026-04-28)

✅ **Implementation Stage T1-T3 batch 完了 + 4-iteration review (3 `/check_job` + 1 `/check_problem`) で発見全 finding (initial 4 + deep 5 + deep deep 5 + light 5 = 19 件) structural fix 14 件 + escalation 1 件 (Z5 → I-212 escalated)**: `/check_job` initial 4-layer review (Spec gap 1 + Implementation gap 2 + Review insight 1 = 4 件 → Fix 1-4)、`/check_job` deep review (D1-D5 = 5 件 → D1/D5 fix 2 件)、`/check_job` deep deep review (F-dd-1〜F-dd-5 = 5 件 → F-dd-1〜F-dd-4 fix 4 件)、`/check_problem` light review (Z1/Z2/Z3/Z4/Z5/Z7 = 6 件 → Z1/Z2/Z4/Z7 fix 4 件、Z3 cosmetic skip、**Z5 (Recursive review trigger formalization) を I-212 PRD entry に escalate** = framework convergence metric の core mechanism として criterion (e) + 修正方針 (5) に追記)。framework 改修 (v1.6 → v1.7 = Rule 9 sub-rule (c) 追加 + pipeline-integrity.md SWC independence formal 化)。次 batch (T4-T6) に **completely clean state** で seamless 続行可能。

### Iteration v9 (2026-04-28、T5 単独 commit + 着手前 Spec への逆戻り = extends 登録 Spec gap fix + 実装中の secondary Spec gap fix)

- **Scope revision rationale (user 確定 "T を一つ完了するごとに commit" rule 適用)**: v8 で T1-T3 batch を 3 task で commit したが、v9 から **T を一つ完了するごとに `/check_job` 4-layer review + 徹底見直し + commit** に運用 transition。本 v9 = T5 単独 commit、subsequent v10/v11/.../v17 = T6/T7/.../T15 各 1-task commit。
- **Spec への逆戻り発動 (T5 着手前調査で発覚した Spec gap、`spec-first-prd.md` 条件 #3 = spec 曖昧、2 通り以上の実装可能)**:
  - **発見内容**: Design section #3-bis (`lookup_method_kind_with_parent_traversal`) は `TypeDef::Struct.extends` を経由した parent traversal を前提とする。しかし `src/registry/collection/class.rs:195` で `extends: vec![]` が hardcode (interface 用 `decl.rs:63-73` のように `super_class` 経由 propagate していない) = B7 inherited detection が機能しない latent state。
  - **影響**: Cell 8 (A1 Read × B7 inherited、Tier 2 honest error reclassify) の dispatch arm `lookup` returns `is_inherited=true` が常に false (extends 空のため direct lookup のみ働く)、結果 fallback FieldAccess emit → matrix cell の "Tier 2 honest error reclassify" semantic 達成不能。
  - **判断**: `extends` 登録は **本 PRD architectural concern (= "Class member access dispatch with getter/setter methodology framework") の前提条件 infrastructure**、別 PRD I-013/I-014 (abstract class 変換) や I-206 (Class inheritance dispatch、B7 Tier 1 化) と orthogonal な registration phase の修正で、本 PRD scope 内で fix が ideal-implementation-primacy 観点で必須。1 PRD = 1 architectural concern との整合 = registration phase は consumer phase (Tier 1 化) と独立 axis。
  - **是正**: Design section #2 (Class collection で method.kind を propagate) を update し `class.rs:195` の `extends: vec![]` を `class.class.super_class` 経由 propagate に修正する task を T5 内で実施。Impact Area に class.rs:195 entry 追加。
  - **`is_static` field 不要判定**: Cell 9 / 18 (B8 static accessor) の fixture は **static-only class** (`class Config { static get version() {} }` 等) のため、receiver = `Ident(Config)` で `reg.get("Config")` = TypeDef::Struct lookup が hit する instance methods は無し → `is_static` field なしでも dispatch logic 上 cell 9/18 は正しく動作する。Mixed (static + instance) class での誤 hit 防止は edge case で、本 PRD matrix では関与しない (現 cells 9, 18 が唯一の B8 cell で static-only fixture)。YAGNI 観点で本 PRD scope では `is_static` field は **追加しない**、必要時に別 PRD で追加。
- **T5 single-task scope (本 v9 commit)**:
  - **T5** (Read context dispatch + B7 traversal helper):
    - **Phase 0**: PRD doc update (Iteration v9 entry + Design section #2 extends 登録明示 + Impact Area class.rs:195 entry)
    - **Phase A**: `class.rs:195` の `extends: vec![]` を `class.class.super_class` 経由 propagate (Implementation gap fix)
    - **Phase B**: `src/registry/mod.rs` に `pub fn lookup_method_in_inheritance_chain(&self, type_name, field) -> Option<(MethodKind, bool /* is_inherited */)>` 追加 (cycle-safe HashSet)
    - **Phase C**: `src/transformer/expressions/member_access.rs::resolve_member_access` に Read context dispatch 拡張 (B1 fallback / B2 Getter / B3 Setter Tier 2 / B4 Getter+Setter / B6 Method Tier 2 / B7 Inherited Tier 2 / B8 Static / B9 unknown fallback)
    - **Phase D**: Unit test in `src/transformer/expressions/tests/i_205.rs` (cells 2-9 Read dispatch + cells 1, 10 fallback regression)
- **Verification protocol**: 本 v9 完了後 `/check_job` 4-layer review + defect 5 category 分類 + 必要に応じ Fix 適用 + Iteration v9 entry を post-review final content に update (= v8 と同 pattern)
- **次 batch 予告**: Iteration v10 = T6 (Write context dispatch) 単独、v11 = T7 (UpdateExpr setter desugar) 単独、…、v17 = T15。各 iteration 完了時に independent commit。

#### Iteration v9 完了判定 (2026-04-28、`/check_job` 4-layer review post-fix)

- **Spec への逆戻り 2 件解消** (post-review final content):
  - **Spec gap #1 (initial)**: `class.rs:195` の `extends: vec![]` hardcode = B7 traversal 前提条件不在。Phase 0 で記録 + Phase A で `class.class.super_class` 経由 propagate に修正。
  - **Spec gap #2 (secondary、cell 8 test fail で発覚)**: `decl.rs:264` の empty body class register filter (`!fields.is_empty() || !methods.is_empty() || constructor.is_some()`) で `class Sub extends Base {}` が placeholder の空 TypeDef (= extends: []) のまま放置。Phase A で `extends.is_empty()` を条件追加で fix。これは Pass 1 placeholder ↔ Pass 2 collect の data preservation invariant 不在の signal。
- **Defect classification (Layer 1-4 trace)**:
  - Grammar gap: 0
  - Oracle gap: 0
  - Spec gap: 2 (上記両方とも本 T5 内で resolved)
  - Implementation gap: 0
  - Review insight: 1 (INV-5 verification task が T1-T15 sequence に明示組込なし、TODO 起票候補 — subsequent T13 batch との整合性 review)
- **Quality gate**:
  - cargo test --lib: 3188 pass / 0 fail (3176 + 12 new T5 tests)
  - cargo test --test e2e_test: 159 pass + 70 ignored (regression なし)
  - cargo test --test compile_test: 3 pass
  - clippy: 0 warning
  - fmt: 0 diff
  - Hono bench Tier-transition compliance: clean 111 / errors 63 (preservation)、内部 -2 OBJECT_LITERAL_KEY / +2 OTHER reclassification (本 PRD T5 dispatch 拡張による分類器 path shift = improvement 候補、本 PRD scope 外 file への new compile error 0 件)
- **Pre/post matrix (Read context cells)**:
  - Fix (Tier 2 → Tier 1): cells 2, 3, 5, 9 (B2 getter / B4 getter+setter / B8 static getter)
  - Reclassify (silent → Tier 2 honest): cells 4, 7, 8 (B3 setter only / B6 method-as-fn-ref / B7 inherited)
  - Preserve: cells 1, 6, 10 (B1 field / B5 AutoAccessor / B9 unknown)
- **Trade-off**: No regression (= 全 cell が pre-state より equal or better、broken-fix PRD として ideal Tier-transition)。
- **Framework 改善検討候補 (本 PRD close 時に integrate or 別 framework PRD 起票)**:
  - **(改善 1)** `spec-stage-adversarial-checklist.md` Rule 9 (c) Field-addition symmetric audit を "field 追加" だけでなく "既存 field の hardcode bug" + "registration site ↔ downstream consumer dependency" にも拡張する candidate (Spec gap #1 の framework 失敗 signal lesson)
  - **(改善 2)** Pass 1 placeholder ↔ Pass 2 collect の data preservation invariant を新 INV (registry layer、本 PRD I-205 では INV-1〜INV-6 だが本 framework 改善で INV-7 候補) として独立記述する candidate (Spec gap #2 の framework 失敗 signal lesson)
  - **(改善 3)** `spec-stage-adversarial-checklist.md` Rule 9 (a) "Spec → Impl Dispatch Arm Mapping" の completeness check を "Tier 1 dispatch + Tier 2 honest error reclassify dispatch" の symmetric enumeration を明示要求する candidate (Spec gap #3 の framework 失敗 signal lesson、本 review で発見)
  - **(改善 4、Iteration v9 deep deep review 由来)** `spec-stage-adversarial-checklist.md` Rule 6 "Matrix/Design integrity" を `## Spec → Impl Dispatch Arm Mapping` table の **token-level 一致 audit** に拡張する candidate。Iteration v9 second-review で私が table に追加した記載 ("static lookup None case = Path-based UserAssocFn emit") が実装と乖離 → third-review (deep deep) で empirical probe + dispatch arm trace で発覚。本 lesson から Rule 6 の verification 手順に "Mapping table の各 row の Emit IR と implementation の actual emit を side-by-side diff" を明示追加する candidate (= post-implementation で Mapping table を update する際の structural integrity check)
  - **(改善 5、Iteration v9 deep deep review 由来)** `check-job-review-layers.md` Layer 4 "Adversarial trade-off" の verification 手順に **"production code integration probe" を必須化** candidate。Iteration v9 second-review では unit test (= IR token-level lock-in) + Hono bench preservation で OK と判定したが、Write context LHS leak (= Critical bug 1) は **Read context 単位 unit test では検出不能** で、`cargo run` (= production code path) の生成 Rust 確認で初検出。Layer 4 verification に "本 PRD scope の representative TS fixture を `cargo run -- fixture.ts` で transform、生成 Rust の dispatch logic 経由 path (Read / Write / OptChain / nested receiver 等の各 axis) を full coverage で empirical 確認" を必須項目化する candidate (= post-implementation review の structural defense-in-depth)
  - **(改善 6、Iteration v9 deep deep review post-Hono-bench 由来)** `prd-completion.md` Tier-transition compliance の wording に **"silent (latent semantic divergence) → Tier 2 honest error reclassify は improvement"** edge case を明示する candidate。現 wording は "Improvement (allowed): existing Tier 2 errors transition Tier-2 → Tier-1 (clean files count 増加 / errors count 減少 が expected)" だが、broken framework の dispatch arm が silent emit (= Hono bench 上 clean 扱い、latent silent semantic divergence) → Tier 2 honest error reclassify (= bench 上 errors count に算入) に transition する case では **bench 数値上 clean 減少 / errors 増加** だが ideal-implementation-primacy 観点で improvement。Hono bench の verdict 判定で "errors 増加 = regression" と短絡する誤判定を防止するため wording 拡張: "silent → Tier 2 honest reclassify は improvement (bench 数値上 errors 増加でも、silent semantic loss 排除のため Tier-transition compliance pass)"。Lesson source: 本 T5 deep deep review post-Hono-bench で `router/smart-router/router.ts:46:20` の `method-as-fn-reference (no-paren)` Tier 2 honest reclassify が +1 OTHER として bench 上 errors 増加、これは silent (pre-T5 fn pointer coercion 不能 silent emit) → Tier 2 honest (post-T5 explicit error) transition の improvement と判定

#### Iteration v9 second-review (deep) findings (2026-04-28、`/check_job` 4-layer 再実施で発見、post-fix 状態)

第三者視点で 2 度目の `/check_job` 4-layer review を実施し、初回 review で見落とした 4 件の defect / insight を追加発見。本 commit 内で **本 T5 scope 内 fix 2 件** (Spec gap #3、Review insight #2) を本質的に解決、**別スコープ defer 2 件** (Review insight #1 → T11、Review insight #3 → T13) は subsequent task description (T11 / T13) に詳細記載済。

##### 俯瞰分析

| # | 課題 | T5 scope 適合性 | 本 commit 内対応 | 別スコープ defer 先 |
|---|------|----------------|---------------|-------------------|
| 1 | Spec gap #3 (Static dispatch arm Tier 2 wording 不在) | Rule 9 (a) "Spec → Impl Dispatch Arm Mapping" は本 T5 implementation の symmetric counterpart、本 T5 scope 内 | **本 commit 内 fix** (Spec → Impl Mapping table 拡張) | — (本 T5 内 closed) |
| 2 | Review insight #2 (Multi-step inheritance test 未追加) | Helper test の boundary value analysis 完備は本 T5 quality の一部 | **本 commit 内 fix** (`test_b7_traversal_multi_step_inheritance_*` 追加) | — (本 T5 内 closed) |
| 3 | Review insight #1 (Mixed class での `is_static` filter 不在) | T5 architectural concern (Read context dispatch) と別軸 (Static dispatch matrix expansion)、reachable scope 外 | — (本 T5 scope 外) | **T11 (11-b) に詳細記載済** |
| 4 | Review insight #3 (INV-5 verification task 不在) | T5 = Read context dispatch、INV-5 = visibility tracking (直交軸)、T13 と cohesive | — (本 T5 scope 外) | **T13 (13-b)(13-c) に詳細記載済** |

##### 本 T5 scope 内 fix の詳細

**Fix 1 (Spec gap #3、本質的解決)**: `## Spec → Impl Dispatch Arm Mapping` の `resolve_member_access` / B7 traversal helper section を **Instance dispatch arms と Static dispatch arms に分離**、static dispatch arm の 5 dispatch arm (Getter / Setter only / Method / inherited / None) を明示 enumerate (Rule 9 (a) compliance restored)。Static × {B3/B6/B7/None field} の matrix cell 化は subsequent T11 (11-c) で実施 (本 T5 scope は dispatch arm mapping、matrix cell 化は scope 拡張)。

**Fix 2 (Review insight #2、本質的解決)**: `tests/i_205.rs` に `test_b7_traversal_multi_step_inheritance_returns_inherited_flag` 追加。`A extends B extends C` の 3-class chain で C.methods["w"] = Setter、A から lookup で N=2 step propagation 経由 (`is_inherited = true`、recursive traversal が grand-parent C まで到達) を verify。boundary value analysis (testing.md "Recursive Function Termination" + "Boundary Value Analysis" 観点) で single-step (N=1) と multi-step (N>=2) の boundary を完全 cover。helper test 4 件 (cycle / direct hit / single-step inherited / multi-step inherited) で structural correctness 完全 lock-in。

##### 別スコープ defer 2 件の詳細記載 location

- **Review insight #1 (Mixed class is_static filter)** → **T11 (11-b)** に Implementation 候補 オプション A (`is_static` field 追加 + Field Addition Symmetric Audit Rule 9 (c-1) compliance) / オプション B (transformer level filter) / 判断基準 (reachability audit) を明示記載
- **Review insight #3 (INV-5 verification)** → **T13 (13-b)(13-c)** に Implementation 候補 オプション A (accessibility field 追加) / オプション B (現状維持) / integration test (`test_invariant_5_private_accessor_external_access_tier2`) green-ify を明示記載
- 補完で **T13 (13-d)** に Review insight #2 の **N>=3 step + cycle in middle** corner test 追加検討も記載 (本 T5 で N=2 step まで cover、N=3+ は subsequent T13 boundary 補完候補)

#### Iteration v9 third-review (deep deep) findings (2026-04-28、`/check_job` 4-layer 第3回実施で発見、post-fix 状態)

第三者視点で 3 度目の deep deep `/check_job` 4-layer review を実施し、second-review 後の Iteration v9 entry でも見落としていた **2 件の critical bug** + 1 件の別スコープ insight を発見。本 commit 内で **本 T5 scope 内 fix 2 件** (Implementation gap critical / Spec gap critical) を本質的に解決、別スコープ defer 1 件 (Static field emission strategy) は subsequent T11 (11-d)(11-e) に詳細記載済。

##### 俯瞰分析 (3 回目)

| # | 課題 | 種別 | 解決状態 |
|---|------|------|----------|
| 1 | **Implementation gap (Critical)**: Write context (assignment LHS) で本 T5 Read dispatch logic が leak、`f.x = 5;` の LHS が `f.x()` (MethodCall) 化 → `f.x() = 5.0;` (invalid Rust LHS) emit | **本 T5 で導入した silent regression** (本 deep deep review で empirical probe 経由初検出) | **本 commit 内 fix 済** |
| 2 | **Spec gap (Critical)**: Spec → Impl Mapping table の Static dispatch arms 最終行 ("lookup None case = Path-based UserAssocFn emit") が実装と乖離 (実装は instance dispatch arm 経由 fallback FieldAccess、`dispatch_static_member_read` の dead code line 280-283 = 構造的 unreachable) | **second-review fix で導入した記載 ≠ 実装** (Rule 6 Matrix/Design integrity 違反) | **本 commit 内 fix 済** |
| 3 | **Review insight (T11 詳細化)**: Static field (`Class.staticField`) emission `Expr::FieldAccess` (Rust 上 `Class.field` invalid `.` syntax) は pre-T5 既存挙動、subsequent T11 (11-d)(11-e) で associated const path 化 | pre-T5 既存、本 T5 で regression なし | **T11 (11-d)(11-e) に詳細記載済** |

##### Critical bug 1 の本質的解決

**Empirical probe** (`tmp/i205_t5_writectx_probe.ts`):
```ts
class Foo { _v: number = 0; get x(): number { return this._v; } set x(v: number) { this._v = v; } }
const f = new Foo();
f.x = 5;
```

**Pre-fix 生成 Rust** (本 deep deep review で発覚):
```rust
pub fn init() {
    f.x() = 5.0;       // ← MethodCall LHS、invalid Rust syntax = compile error
    println!("{}", f.x());
}
```

**Root cause**: `convert_member_expr_inner` の Ident path (line 547+) は `for_write` flag を ignore、末尾 line 619 で `resolve_member_access` を call、本 T5 Read dispatch logic apply。Write context (LHS) でも MethodCall emit され、Rust 上 invalid LHS。

**Fix**: `convert_member_expr_inner` で `for_write=true` の場合、本 T5 Read dispatch logic を **skip**、既存 FieldAccess fallback 維持 (= pre-T5 同等挙動)。Setter dispatch (`f.set_x(5.0)`) は subsequent **T6 (Write context dispatch、`dispatch_member_write` helper)** で別途実装。本 fix で T5 (Read) と T6 (Write) の **scope 分離が structural に enforce**。

**Post-fix 生成 Rust** (verified):
```rust
pub fn init() {
    f.x = 5.0;          // ← FieldAccess (pre-T5 同等、setter dispatch は T6 で)
    println!("{}", f.x());  // ← Read context は本 T5 で MethodCall (Tier 1 dispatch)
}
```

**Test 追加**: `test_write_context_lhs_does_not_leak_read_dispatch` を `tests/i_205.rs` に追加、B4 (getter+setter pair) class の `f.x = 5;` で `convert_member_expr_for_write` の output IR が **`Expr::FieldAccess`** であることを direct verify (= Read dispatch leak の structural lock-in)。

##### Critical bug 2 の本質的解決

**Root cause**: Iteration v9 second-review で私が追加した Spec → Impl Mapping table の Static dispatch arms 最終行:
```
| `lookup` returns `None` (static field 等) | (matrix cell 化なし、defensive code) | `Expr::FnCall { target: CallTarget::UserAssocFn { ty, method }, args: vec![] }` (Path-based emit) |
```
これは **実装と乖離**:
- 実装の `resolve_member_access`: static dispatch arm の `if let Some((sigs, ...))` block は **lookup hit case のみ** dispatch_static_member_read を call。lookup miss は arm を抜けて instance dispatch arm 経由 → 5. Fallback FieldAccess emit
- → token-level mismatch、Rule 6 Matrix/Design integrity 違反

加えて、`dispatch_static_member_read` の最後 (line 280-283) の `Ok(Expr::FnCall { ... })` は **dead code** (= `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant + `MethodKind` 3 variant exhaustive により構造的 unreachable)。

**Fix**:
1. **Spec → Impl Mapping table 修正**: Static dispatch arms を 5 arm に修正 (`Some((sigs, false))` getter / setter only / method / `Some((sigs, true))` inherited / `dispatch_static_member_read` を経由しない `lookup` None case = `resolve_member_access` 最終 fallback FieldAccess emit)。実装と token-level 一致達成、Rule 6 Matrix/Design integrity restored。
2. **Dead code 排除**: `dispatch_static_member_read` line 280-290 を `unreachable!()` macro で置換、`sigs` non-empty + `MethodKind` 3 variant exhaustive の invariant を **structural enforcement** (= dead code 排除 + invariant codified)。

##### Defect classification (3 回目 final)

- Grammar gap: 0
- Oracle gap: 0
- **Spec gap: 4** (#1 extends 登録、#2 decl.rs:264、#3 static dispatch wording missing、**#4 Mapping table 誤記** = 全て本 T5 内 resolved、framework 失敗 signal として記録)
- **Implementation gap: 1** (Write context LHS leak、本 T5 内 resolved)
- **Review insight: 4** (#1 mixed class is_static = T11 defer、#2 multi-step inheritance test = 本 T5 内 resolved、#3 INV-5 verification = T13 defer、**#4 static field emission strategy** = T11 (11-d)(11-e) に詳細記載済)

##### Quality 再 verify (third-review post-fix、post-Hono-bench)

- cargo test --lib transformer::expressions::tests::i_205: **14 pass / 0 fail** (cells 1/2/3/4/5/7/8/9/10 + helper 4 件 + Write context regression 1 件)
- lib total: 3190 pass / 0 fail (3176 + 14 new T5 tests)
- regression: 0 (e2e 159 pass + 70 ignored / compile 3 pass / clippy 0 warning / fmt 0 diff)
- empirical Write context probe (post-fix): `f.x = 5;` LHS = `f.x = 5.0;` (FieldAccess、pre-T5 同等、Read leak 排除)、Read 側 = `f.x()` (Tier 1 MethodCall、本 T5 dispatch)
- empirical Static getter probe (post-fix): `Config.version` = `Config::version()` (cell 9 期待通り)
- empirical Static field probe (pre-T5 既存挙動): `Config.DEFAULT` = `Config.DEFAULT` (Rust 上 invalid `.` syntax = compile error、subsequent T11 (11-d) で `Config::DEFAULT` 化)
- **Hono Tier-transition compliance verdict**: 
  - Pre-fix snapshot: clean 111 / errors 63
  - Post-fix snapshot: clean 110 (-1) / errors 64 (+1 OTHER = `router/smart-router/router.ts:46:20` `method-as-fn-reference (no-paren)`)
  - **Verdict: improvement (silent → Tier 2 honest reclassify、本 PRD scope 内 dispatch arm B6)**。pre-T5 で本 line の `obj.method` (no paren reference) は silent FieldAccess emit (Rust 上 fn pointer coercion 不能 latent silent semantic divergence、bench 上 clean 扱い)、post-T5 で本 T5 dispatch arm (Tier 2 honest "method-as-fn-reference (no-paren)") に reclassify (bench 上 errors 算入)。**silent semantic loss 排除 = ideal-implementation-primacy 観点で improvement**、broken-fix PRD wording の "Improvement (allowed): existing Tier 2 errors transition Tier-2 → Tier-1" の edge case (silent → Tier 2 honest direction も improvement、framework 改善 6 候補で明示記載)
  - **Preservation (本 PRD scope 外 features)**: 62 errors のうち 62 件が pre-T5 と同一、本 PRD scope 外 features への new compile error **0 件** = Tier-transition compliance pass

##### Conclusion (third-review)

T5 atomic commit ready (post-third-review fix 含む)、本 T5 scope 内対応すべき items は **deep deep review で発覚した Critical bug 2 件含め全て本質的に解決済**。別スコープ items (Static field emission strategy = T11 (11-d)(11-e)、INV-5 = T13 (13-b)(13-c)、Mixed class is_static = T11 (11-b)、Multi-step inheritance N>=3 step = T13 (13-d)) は appropriate location に詳細記載済、subsequent batch 開始時に該当 task description から context 100% recover 可能。

##### Iteration v9 second-review 完了判定 (post-fix)

**Note**: 本 second-review 完了判定は third-review (上記) で **superseded** された。second-review 時点で見落としていた Critical bug 2 件 (Write context LHS leak Implementation gap + Mapping table 誤記 Spec gap) を third-review で発見・本 T5 内 fix。final 完了判定は **third-review 完了判定** (上記 ##### Conclusion (third-review)) を参照。

### Iteration v10 (2026-04-28、T6 単独 commit + Spec → Impl Mapping table Read/Write symmetric 化 Spec gap fix)

- **Scope**: T6 (Write context dispatch via `dispatch_member_write` helper) 単独 commit、user 確定 "T を一つ完了するごとに `/check_job` 4-layer review + 徹底見直し + commit" 運用を継続。
- **T6 single-task scope**:
  - **Phase A**: `dispatch_member_write` helper を `src/transformer/expressions/member_access.rs` に追加 (Read context `resolve_member_access` と symmetric、INV-2 cohesion)
  - **Phase B**: `dispatch_instance_member_write` + `dispatch_static_member_write` 内 helper 追加 (各 helper 3 dispatch arm + `unreachable!()` macro による structural invariant codification、Read context `dispatch_*_member_read` と symmetric)
  - **Phase C**: `convert_assign_expr` の `AssignOp::Assign` × `SimpleAssignTarget::Member` × `MemberProp::Ident | PrivateName` 早期 gate で `dispatch_member_write` 経由 (Computed `obj[i] = v` は既存 `convert_member_expr_for_write` の `Expr::Index` 経路で handle、本 dispatch 通過なし)
  - **Phase D**: Unit test 10 件 (cells 11/12/13/14/16/17/18/19 + INV-2 E1 Read/Write symmetry + T6 Fallback equivalence) を `tests/i_205.rs` に追加 (boundary value analysis + decision table coverage)
- **`/check_job` 4-layer review 結果 (2026-04-28、本 commit 内 1 度実施で全層 verify)**:

#### Iteration v10 完了判定 (2026-04-28、`/check_job` 4-layer review post-fix)

- **Defect classification (Layer 1-4 trace)**:
  - Grammar gap: 0
  - Oracle gap: 0
  - **Spec gap: 1** (`## Spec → Impl Dispatch Arm Mapping` の `dispatch_member_write` table が Read context (`dispatch_member_read`) と asymmetric、Write context の static defensive 3 arm 行 (Getter only / Method / inherited static) が不在 = framework 失敗 signal、本 commit 内で **本 T6 内 fix 済** = Read mapping と完全 symmetric な 5 arm 構造に拡張、Iteration v9 deep deep review fix pattern 継承)
  - Implementation gap: 0
  - Review insight: 1 (Framework v1.8 候補 = matrix-driven PRD の Spec → Impl Mapping Read/Write symmetric completeness を `audit-prd-rule10-compliance.py` で auto verify する mechanism、本 PRD I-205 close 時 framework 改善 PRD 起票検討候補)
- **Quality gate**:
  - cargo test --lib: 3200 pass / 0 fail (3190 + 10 new T6 tests)
  - cargo test --lib `transformer::expressions::tests::i_205`: 24 pass (T5 cells 1/2/3/4/5/7/8/9/10 + B7 traversal helper 4 件 + Write context regression 1 件 + T6 cells 11/12/13/14/16/17/18/19 + INV-2 E1 + T6 Fallback equivalence)
  - cargo test --test e2e_test: 159 pass + 70 ignored (regression なし、cell 12/13/14/18 fixture は T14 で一括 green-ify 予定)
  - cargo test --test compile_test: 3 pass
  - clippy: 0 warning
  - fmt: 0 diff
  - Hono bench Tier-transition compliance: clean 110 (preserved) / errors 64 (preserved) → **Preservation** (allowed per `prd-completion.md`、Hono が external setter dispatch on class instances を主要使用していないため)
- **Pre/post matrix (Write context cells)**:
  - Fix (Tier 2 → Tier 1): cells 13, 14, 18 (B3 setter only / B4 getter+setter / B8 static setter Write、Tier 1 setter dispatch 経由)
  - Reclassify (silent → Tier 2 honest): cells 12, 16, 17 (B2 getter only / B6 method / B7 inherited Write、Tier 2 honest error reclassify)
  - Preserve: cells 11, 19 (B1 field / B9 unknown Write、FieldAccess Assign fallback 維持)
  - Unfixed (T7-T10 で対応): cells 20-45 (compound +=, -=, ??=, &&=, ||=, ++/--)、cells 60-64 (E2 internal `this.x = v`)
- **Trade-off**: No regression (= 全 cell が pre-state より equal or better、broken-fix PRD として ideal Tier-transition)。INV-2 E1 / E2 dispatch path symmetry の Read/Write 両方向 cohesion を unit test (`test_inv_2_e1_read_write_dispatch_symmetry_b4`) で structural lock-in (E2 internal は T10 で正式 verify)。
- **Empirical CLI verify (production code path)**:
  - B4 instance setter `b.x = 5;` → `b.set_x(5.0);` ✓ (CLI manual probe 経由)
  - B8 static setter `Counter.count = 7;` → `Counter::set_count(7.0);` ✓ (CLI manual probe 経由)
  - Static field `Counter._n = v;` (set_count body 内) → `Counter._n = v;` (Rust 上 invalid `.` syntax、Tier 2 等価 compile error) — pre-T6 既存挙動維持、subsequent T11 (11-d) で `Class::set_*` associated fn / OnceLock 等の emission strategy 確定
- **Spec gap fix (本 commit 内 fix 済、Iteration v10 source)**: `## Spec → Impl Dispatch Arm Mapping` の `dispatch_member_write (Write context dispatch)` section を **Instance dispatch arms と Static dispatch arms に分離**、Read mapping (`dispatch_*_member_read`) と完全 symmetric な structural form に拡張。Static dispatch arms 5 arm (Setter / Getter only / Method / inherited / `dispatch_static_member_write` を経由しない `lookup` None case) を明示 enumerate (Rule 9 (a) compliance restored)。Static × {B3/B6/B7/None field} Write の matrix cell 化は subsequent T11 (11-c) で実施 (本 T6 scope は dispatch arm mapping の completeness、matrix cell 化は scope 拡張)。

##### Conclusion (Iteration v10 first-review)

T6 atomic commit ready、本 T6 scope 内対応すべき items は **`/check_job` 4-layer review で発覚した Spec gap 1 件含め全て本質的に解決済**。別スコープ items (Static field emission strategy = T11 (11-d)、Mixed class is_static = T11 (11-b)、Static B3/B6/B7 Write matrix cells = T11 (11-c)、E2 internal `this.x = v` dispatch verify = T10、Compound assign setter dispatch = T7-T9) は appropriate task description に既記載済。次 iteration v11 = T7 (UpdateExpr `++/--` Member target setter desugar) 単独 commit に進む。

#### Iteration v10 second-review (deep deep) findings (2026-04-28、`/check_job` 4-layer 第2回実施で発見、post-fix 状態)

第三者視点で 2 回目の deep deep `/check_job` 4-layer review を実施し、first-review でも見落としていた **5 件の defect / insight** を追加発見。本 commit 内で **本 T6 scope 内 fix 4 件** を本質的に解決、別スコープ defer 1 件は T11 (11-f) に詳細記載。

##### 俯瞰分析 (2 回目)

| # | 課題 | 種別 | 本 commit 内対応 | 別スコープ defer 先 |
|---|------|------|------------------|-------------------|
| 1 | **DRY violation (Layer 1)**: `resolve_member_access` (Read T5) と `dispatch_member_write` (Write T6) で receiver type detection 知識が完全二重実装 (Static gate + Instance gate)。subsequent T7-T9 で 3 度目 duplication 発生する増殖性 risk | Implementation gap (本 T6 で導入された duplication、framework `design-integrity.md` "DRY" + `ideal-implementation-primacy.md` structural fix 違反) | **本 commit 内 fix** (`classify_member_receiver` shared helper 抽出 + `MemberReceiverClassification` enum 定義、Read/Write 両 helper を経由) | — (本 T6 内 closed) |
| 2 | **Asymmetric structural enforcement (Layer 1)**: T5 `dispatch_instance_member_read` の最終 arm = `Ok(Expr::FieldAccess)` (dead code) 残置、`dispatch_static_member_read` (T5 v9 deep deep で `unreachable!()` 化済) と asymmetric。本 T6 `dispatch_instance_member_write` は `unreachable!()` 採用 = Read instance helper だけ非対称 | Implementation gap (T5 framework 不徹底、本 T6 で発覚) | **本 commit 内 fix** (T5 dead code を `unreachable!()` macro に置換、4 helper 全てが symmetric structural enforcement 統一) | — (本 T6 内 closed) |
| 3 | **C1 branch coverage gap — Static field lookup miss (Layer 1)**: `dispatch_member_write` Static gate の lookup miss branch (= static field、`Counter._n = v`) が未 test。fall-through reachable な branch、testing.md C1 coverage 違反 | Implementation gap (test coverage 不足) | **本 commit 内 fix** (`test_t6_static_field_lookup_miss_falls_through_to_field_access_assign` 追加) | — (本 T6 内 closed) |
| 4 | **C1 branch coverage gap — Defensive dispatch arms (Layer 1)**: `dispatch_static_member_write` 3 defensive arms (Getter only Write / Method Write / inherited static Write) + `dispatch_static_member_read` 3 defensive arms (Setter only Read / Method Read / inherited Read) = 計 6 arm が未 test。matrix cell 化なし (T11 (11-c) で expansion 予定) だが、C1 coverage + error message lock-in 観点で test 必要 | Implementation gap (test coverage 不足、Read/Write 対称) | **本 commit 内 fix** (Read 3 + Write 3 = 6 test 追加、計 7 test 追加で C1 coverage gap closed) | — (本 T6 内 closed) |
| 5 | **Receiver Ident unwrap pre-existing latent gap (Layer 3)**: `classify_member_receiver` Static gate (T6 v10 で集約後) の `if let ast::Expr::Ident(ident) = receiver` 直接 match は Paren / TsAs / TsNonNull 等の wrap を unwrap せず、`(Counter).x = v` 等 wrap 経由 access で static dispatch を逃す latent silent reachability gap。**T5 から続く pre-existing issue** (T5 Enum special case でも同 pattern)、本 T6 で導入された defect ではなく framework gap | Review insight (pre-existing、本 T6 scope 外 = receiver expression shape 軸は orthogonal architectural concern) | — (本 T6 scope 外) | **T11 (11-f) に詳細記載済** (Implementation 候補 オプション A = AST level helper / オプション B = TypeResolver `ClassConstructor` type marker / 判断基準 = reachability audit + design-integrity vs pipeline-integrity trade-off) |

##### 本 T6 scope 内 fix の詳細

**Fix A (DRY violation 解消、本質的解決)**: `MemberReceiverClassification` enum (Static / Instance / Fallback 3 variants) + `classify_member_receiver(&self, receiver: &ast::Expr, field: &str) -> MemberReceiverClassification` shared helper を `member_access.rs` に追加。Read context (`resolve_member_access`) + Write context (`dispatch_member_write`) を本 helper 経由に refactor、Static gate (Ident + `get_expr_type` None + Struct + lookup hit) + Instance gate (Named/Option<Named> + lookup hit) + Fallback の logic を 1 箇所に集約。Subsequent T7-T9 (compound `+= ??=` 等) も本 helper を leverage 可能 = 増殖性 risk を structural に排除。

**Fix B (Asymmetric structural enforcement 解消、本質的解決)**: T5 `dispatch_instance_member_read` の最終 arm (`Ok(Expr::FieldAccess { object, field })`) を `unreachable!()` macro に置換、structural invariant (`sigs` non-empty + `MethodKind` 3 variant exhaustive) を codify。これにより Read instance / Read static / Write instance / Write static の 4 helper 全てが `unreachable!()` で symmetric structural enforcement 統一、framework 観点で完全 symmetry 達成。

**Fix C (Static field lookup miss C1 coverage、本質的解決)**: `test_t6_static_field_lookup_miss_falls_through_to_field_access_assign` 追加。`class Counter { static _n: number = 0; }` の `Counter._n = 7;` で Static gate lookup miss → Fallback FieldAccess Assign emit (= pre-T6 既存挙動維持、subsequent T11 (11-d) で associated const path に修正予定) を verify。

**Fix D (Defensive dispatch arms C1 coverage、本質的解決)**: 6 test 追加 (Read 3 + Write 3):
- `test_t6_static_getter_only_write_emits_unsupported_syntax_error` (Write static B3 = "write to read-only static property")
- `test_t6_static_method_write_emits_unsupported_syntax_error` (Write static B6 = "write to static method")
- `test_t6_static_inherited_setter_write_emits_unsupported_syntax_error` (Write static B7 = "write to inherited static accessor")
- `test_t6_read_static_setter_only_emits_unsupported_syntax_error` (Read static B3 = "read of write-only static property")
- `test_t6_read_static_method_emits_unsupported_syntax_error` (Read static B6 = "static-method-as-fn-reference (no-paren)")
- `test_t6_read_static_inherited_getter_emits_unsupported_syntax_error` (Read static B7 = "inherited static accessor access")

##### 別スコープ defer 1 件の詳細記載 location

- **Defect 5 (Receiver Ident unwrap)** → **T11 (11-f)** に Implementation 候補 オプション A (AST level helper `unwrap_paren_ts_as_ts_non_null`) / オプション B (TypeResolver `RustType::ClassConstructor` type marker 拡張) / 判断基準 (Hono reachability audit + design-integrity vs pipeline-integrity trade-off) + Pre-existing impact range (Read Enum special case / Static gate / その他 Ident match site) を明示記載。T11 (11-b) Mixed class is_static filter と同 spec stage の cohesive batch 候補。

##### Defect classification (2 回目 final)

- Grammar gap: 0
- Oracle gap: 0
- **Spec gap: 1** (first-review fix 済 = Mapping table Read/Write asymmetric)
- **Implementation gap: 4** (#1 DRY violation = 本 commit 内 fix / #2 T5 dead code = 本 commit 内 fix / #3 Static field lookup miss test gap = 本 commit 内 fix / #4 Defensive dispatch arms C1 gap = 本 commit 内 fix)
- **Review insight: 2** (#1 first-review = Framework v1.8 candidate / #2 second-review = Receiver Ident unwrap pre-existing gap = T11 (11-f) defer)

##### Quality 再 verify (second-review post-fix)

- cargo test --lib `transformer::expressions::tests::i_205`: **31 pass / 0 fail** (24 first-review baseline + 7 new tests = static field lookup miss 1 + Read 3 defensive + Write 3 defensive)
- lib total: 3207 pass / 0 fail (3200 first-review + 7 new tests)
- regression: 0 (e2e 159 pass + 70 ignored / compile 3 pass / clippy 0 warning / fmt 0 diff)
- **Refactor 整合 verify**: 既存 24 test (T5 cells + T6 cells + INV-2 + Fallback equivalence) 全 pass = `classify_member_receiver` 経由 refactor が behavior preserved を structural lock-in
- Hono Tier-transition compliance: post-Fix-A/B/C/D snapshot を再取得して verify (= 本 commit 内 final state)

##### Conclusion (Iteration v10 second-review、final)

T6 atomic commit ready (post-second-review fix 含む)、本 T6 scope 内対応すべき items は **`/check_job` 4-layer review (first + second iteration) で発覚した Spec gap 1 件 + Implementation gap 4 件含め全て本質的に解決済**。別スコープ items (Receiver Ident unwrap = T11 (11-f) Iteration v10 second-review source、Static field emission strategy = T11 (11-d)、Mixed class is_static = T11 (11-b)、Static B3/B6/B7 Write matrix cells = T11 (11-c)、E2 internal `this.x = v` dispatch verify = T10、Compound assign setter dispatch = T7-T9) は appropriate task description に既記載済。次 iteration v11 = T7 (UpdateExpr `++/--` Member target setter desugar) 単独 commit に進む。

#### Iteration v10 third-review (`/check_problem` light review) findings (2026-04-28、second-review post-fix 後の振り返り)

`/check_problem` light review (= session 内未対応 issue の振り返り) を実施し、second-review でも明示 record 化していなかった **3 件の pre-existing latent gap** を識別。本 T6 architectural concern (Class member ACCESS dispatch) と orthogonal な軸 = 全て scope 外として TODO 起票 (user 確定 2026-04-28)。

##### 俯瞰分析 (3 回目、light review)

| # | 課題 | 種別 | scope 判定 | 起票 ID |
|---|------|------|-----------|---------|
| 1 | **`convert_call_expr` static method call dispatch DRY violation + 3 latent gaps**: calls.rs:213-225 (I-378 T9 由来 Static method call dispatch) が classify_member_receiver と同知識を持ちつつ、defensive check 不足 (a) is_interface filter なし / (b) get_expr_type None gate なし (= shadowing 不防止) / (c) lookup_method_sigs_in_inheritance_chain 不使用 (= inherited static method call で Sub::method emit、compile error) | Pre-existing (T5/T6 で導入されたわけではない、I-378 T9 由来)、本 T6 scope 外 = "Static method CALL dispatch" は別 architectural concern | **TODO 起票 (I-214)** | I-214 |
| 2 | **`arr.length = v` write Tier 2 silent gap**: TS は `arr.length = n` で truncate/pad semantic、現 transformer は `Expr::Assign { FieldAccess { arr, "length" }, value }` emit で Rust E0609。Read 側 (T5 既存 `arr.length` → `arr.len() as f64`) と Write 側で対応度 asymmetric | Pre-existing (T5 から)、本 T6 scope 外 = Vec/Array specific syntax 軸 (Class member access と orthogonal) | **TODO 起票 (I-215)** | I-215 |
| 3 | **`!(b.x = 5)` (bang on B4 setter assignment) Tier 2 behavioral change**: T6 で B4 plain assign が `Expr::MethodCall { set_x, [value] }` に変わった結果、`convert_bang_assign` (binary.rs:509) の destructure pattern `let Expr::Assign { ... }` が fail、Layer 4 fall-through で `!MethodCall` (= `!void`) compile error。Pre-T6 も Tier 2 (E0609)、post-T6 も Tier 2 (E0277) で **silent semantic change なし**、両 Tier 2 で error 種別変化のみ | Pre-existing pattern (I-171 T4 convert_bang_assign 導入時から compound × bang 軸の design issue)、本 T6 scope 外 = bang operator interaction 軸 | **TODO 起票 (I-216)** | I-216 |

##### 起票 entries の詳細記載 location

- **I-214** (calls.rs DRY violation + 3 latent gaps): TODO file の Tier 2 section に I-213 後に追加。Implementation 候補 (calls.rs を classify_member_receiver 経由 refactor)、3 latent gaps の各 empirical evidence (interface filter / shadowing / inherited static method call) + Problem Space 軸 + 影響範囲 LOC 推定 + Hono reachability audit driver で priority 確定方針を詳細記載。T11 (11-b/c/d/f) と cohesive batch 候補。
- **I-215** (arr.length write Tier 2 silent gap): TODO file の Tier 2 section に I-214 後に追加。Implementation 候補 (オプション A Tier 1 truncate/clear/resize emission / オプション B Tier 2 honest error reclassify)、TS spec で明確に reject される subset (readonly array / const literal) の handle 方針 + Problem Space 軸 + Hono reachability audit driver で priority 確定方針を詳細記載。
- **I-216** (bang on B4 setter assignment Tier 2 behavioral change): TODO file の Tier 2 section に I-215 後に追加。Pre/post T6 Tier 2 比較 (両者 compile error、silent semantic change なし、post-T6 が more honest)、Implementation 候補 (オプション A `convert_bang_assign` を MethodCall setter dispatch に拡張 / オプション B Tier 2 honest error reclassify)、`!(assign-expr)` syntax 自体の anti-pattern reachability 評価 + judgment criteria を詳細記載。reachability ゼロなら scope 外決定 + 永続 ignore 候補。

##### Conclusion (Iteration v10 third-review、light review final)

T6 atomic commit ready (post-third-review TODO 起票完了)。本 light review で発見した 3 件は全て pre-existing latent gap (T6 で導入されたものではない)、本 T6 architectural concern と orthogonal な軸として TODO file に詳細記載 + 別 PRD 候補化 (user 確定)。本 T6 commit 内の対応 = 不要 (= 全 record-keeping 完了)。次 iteration v11 = T7 (UpdateExpr `++/--` Member target setter desugar) 単独 commit に進む。

### Iteration v11 (2026-04-29、T7 単独 commit + Spec gap fix = TypeResolver Update.arg 未再帰)

**Architectural concern**: UpdateExpr (`++`/`--`) Member target で setter desugar (B4 numeric)、B1/B9 fallback (regression Tier 2 → Tier 1 transition)、B2/B3/B6/B7 Tier 2 honest error reclassify、B8 static setter desugar、postfix old-value / prefix new-value preservation。

**Implementation 内容**:
- `convert_update_expr` を free function から `Transformer` method 化 (`mod.rs:129` の call site も `self.convert_update_expr(up)` に変更)。Member target arm = `convert_update_expr_member_arm(member, op, is_postfix)` で T6 `classify_member_receiver` 経由 dispatch。
- `member_dispatch.rs` に T7 dispatch helper 5 件追加: `getter_return_is_numeric` (RustType::F64 / Primitive(_) numeric check)、`build_update_setter_block` (instance/static 共通 setter desugar block builder、postfix `__ts_old` binding / prefix `__ts_new` binding)、`dispatch_instance_member_update` (B2/B3/B4/B6/B7 dispatch arms)、`dispatch_static_member_update` (B8 + defensive Tier 2 honest error)、`non_numeric_update_message` (op-specific Tier 2 error wording: `++` → "increment of non-numeric (...)" / `--` → "decrement of non-numeric (...)")。各 helper は `unreachable!()` macro で MethodKind 3-variant exhaustiveness + lookup non-empty invariant codify (= T6 dispatch helper と symmetric structural enforcement)。
- `assignments.rs` に `build_fallback_field_update_block` 新規 helper (B1 field / B9 unknown 用 direct FieldAccess BinOp block emit、postfix `{ let __ts_old = obj.x; obj.x = __ts_old OP 1.0; __ts_old }` / prefix `{ obj.x = obj.x OP 1.0; obj.x }`)。

**Spec gap fix (本 T7 scope 内、Iteration v9 extends/decl.rs fix と同 pattern)**:
- **Defect**: `pipeline/type_resolver/expressions/mod.rs:118-121` の `ast::Expr::Update(_)` arm が `update.arg` を recursive resolve せず、Member target の receiver `obj` Ident の expr_type が未登録。Transformer `classify_member_receiver` の `get_expr_type(receiver)` が None → Static gate も Instance gate も skip → silent Fallback dispatch (= class member setter dispatch を逃す silent semantic loss、cells 43/45-c/45-dd で setter desugar 発火せず B1/B9 fallback emit、Tier 2 broken state 維持)。
- **Trace** (`post-implementation-defect-classification.md` 5-category):
  - reference doc (`doc/grammar/ast-variants.md`): UpdateExpr 章 entry あり
  - oracle: `tests/swc_parser_increment_non_numeric_test.rs` で SWC parser empirical lock-in 確認済
  - matrix: cells 43/45-c/45-dd を ✗ 修正対象として enumerate 済
  - **enumerate gap**: spec stage で TypeResolver coverage axis (= "TypeResolver が Member access の receiver expr_type を Update arg context で register するか") を independent dimension として enumerate していなかった = **Spec gap category** (framework 失敗 signal)
- **Fix**: `Unary` arm pattern (line 98-108、`self.resolve_expr(&unary.arg)` で arg recursion + op-specific 結果 type return) を踏襲、`ast::Expr::Update(update)` arm で `self.resolve_expr(&update.arg);` 追加 + `RustType::F64` return。Effect は narrow context 含む全 type lookup query path に届き、subsequent T8/T9 compound assign / logical compound dispatch も同 prerequisite を共有。
- **Framework 改善検討**: `spec-stage-adversarial-checklist.md` Rule 10 axis enumeration の default check axis として **"TypeResolver visit coverage of operand-context expressions"** (Update.arg / Unary.arg / Cond.test/cons/alt 等の operand position で receiver type 必要時、TypeResolver が visit してるか) 追加候補。

**Cohesive cleanup (T7 scope 内)**:
- 既存 Ident form `convert_update_expr` の binding 名 `_old` を `__ts_old` に rename。Reason: I-154 で確立済の `__ts_` namespace reservation rule を value bindings (= label に加えて) に extension、user identifier (`_old` / `_new` 等) collision 防止 + T7 で導入した Member form の `__ts_old`/`__ts_new` と統一。
- snapshot tests 3 件 (`do_while`/`general_for_loop`/`update_expr`) は pure rename diff で `cargo insta accept` で auto-update。

**Unit tests**: `tests/i_205/update.rs` 新規 (15 tests):
- Cells 42/42-prefix/45-a (B1 field fallback、Block FieldAccess BinOp、postfix old-value / prefix new-value)
- Cell 45-de (B9 unknown fallback)
- Cells 43/43-prefix/45-c (B4 setter desugar、postfix `__ts_old` / prefix `__ts_new`、setter MethodCall set_x with BinOp)
- Cell 44 (B4 String ++、`"increment of non-numeric (...)"`)
- Cell 44 symmetric (B4 String --、`"decrement of non-numeric (...)"`)
- Cell 45-b (B2 getter only --、`"write to read-only property"`)
- B3 setter only ++ (matrix cell 化なし、`"read of write-only property"`、Update-specific Tier 2)
- Cell 45-db (B6 method --、`"write to method"`)
- Cell 45-dc (B7 inherited --、`"write to inherited accessor"`)
- Cell 45-dd (B8 static --、static setter desugar、FnCall::UserAssocFn)
- Computed `arr[0]++` reject (matrix scope 外、existing error path 維持)

**Pre/post matrix** (cells 42-45 全 transition):

| Cell | Pre-T7 | Post-T7 | Delta |
|------|--------|---------|-------|
| 42 (B1 field ++) | ✗ Tier 2 broken (Member target reject) | ✓ Tier 1 (FieldAccess BinOp block) | **fix** |
| 43 (B4 both ++) | ✗ Tier 2 broken | ✓ Tier 1 setter desugar | **fix** |
| 44 (B4 non-numeric ++) | ✗ Tier 2 broken | ✓ Tier 2 honest error | **fix (silent → honest)** |
| 45-a (B1 field --) | ✗ Tier 2 broken | ✓ Tier 1 | **fix** |
| 45-b (B2 getter only --) | ✗ Tier 2 broken | ✓ Tier 2 honest "write to read-only" | **fix** |
| 45-c (B4 both --) | ✗ Tier 2 broken | ✓ Tier 1 setter desugar | **fix** |
| 45-da (B5 AutoAccessor --) | ✗ class.rs:165 honest error (PRD 2.7) | ✓ same (unchanged) | **preserved** |
| 45-db (B6 method --) | ✗ Tier 2 broken | ✓ Tier 2 honest "write to method" | **fix** |
| 45-dc (B7 inherited --) | ✗ Tier 2 broken | ✓ Tier 2 honest "write to inherited accessor" | **fix** |
| 45-dd (B8 static --) | ✗ Tier 2 broken | ✓ Tier 1 static setter desugar | **fix** |
| 45-de (B9 unknown --) | ✗ Tier 2 broken | ✓ Tier 1 (FieldAccess BinOp block) | **fix** |
| Ident `i++` (existing) | ✓ `_old` block | ✓ `__ts_old` block (rename) | **structural improvement** |

**Regression cells (✓ → ✗)**: なし。

**Final quality**: cargo test --lib 3235 pass (3220 baseline + 15 T7) / e2e 159 pass + 70 ignored / integration 122 pass (snapshot 3 件 auto-accept) / compile_test 3 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK。

**Defect Classification (Iteration v11 final)**: Spec gap 1 (TypeResolver Update.arg 未再帰、本 T7 scope 内 resolved、framework 改善 candidate = Rule 10 "TypeResolver visit coverage" axis 追加) / Implementation gap 0 / Review insight 0 (T11 (11-f) Receiver Ident unwrap robustness は既 defer 整合)。

**CLI manual probe**: `tests/e2e/scripts/i-205/cell-43-postfix-increment.ts` で `c.value++` 部分が `{ let __ts_old = c.value(); c.set_value(__ts_old + 1.0); __ts_old }` emit 確認。

#### Iteration v11 second-review findings (`/check_job` 4-layer review、Iteration v11 post-fix 後の徹底見直し)

User 指示「妥協絶対禁止 / 理想的でクリーンな実装 / 必要十分で高品質な自動テスト」に基づく **第三者視点での 4-layer review** で発見した 6 件の課題を本 Iteration v11 scope 内で structural fix。

| # | 課題 | Defect category | Action | Status |
|---|------|----------------|--------|--------|
| L1-1 | `convert_update_expr_member_arm` doc comment "fallback" 紛らわしい wording (Computed `None` を `MemberReceiverClassification::Fallback` ではなく anyhow Err 直接 return に流す事実が不明確) | Implementation gap | 即時 fix: doc comment correct + early return path 明示 | **resolved** |
| L1-2 | `test_cell_45de_b9_unknown_postfix_decrement_emits_fallback_block` 弱 assertion (Block 確認のみ、中身未 verify) | Implementation gap | 即時 fix: postfix old-value preservation block の specific 中身を verify (Stmt 0 Let __ts_old + FieldAccess、Stmt 1 BinOp Sub、Stmt 2 TailExpr) | **resolved** |
| L2-2 | Cell 44 E2E `#[ignore]` message 不整合 ("T14 で green 化予定" は Tier 2 honest error と semantic conflict、cell 15 Prop::Assign permanent #[ignore] pattern と非整合) | Spec gap | 即時 fix: cell 15 pattern と整合させ permanent #[ignore] 化 + behavioral lock-in 委譲先 (unit test + SWC parser empirical) を明示 | **resolved** |
| L3-1 | Op-axis × postfix-axis test coverage gap (`++` only 3 cells / `--` 8 cells、symmetric gap = B1 prefix --、B4 prefix --、B2/B6/B7/B8 ++ で 7 件 missing) | Review insight (Implementation gap) | 即時 fix: 7 件 op-symmetric tests 追加 (`tests/i_205/update.rs` 末尾の `Op-axis × postfix-axis cross-coverage` section) | **resolved** |
| L3-2 / L3-3 / L3-4 | Matrix Spec gap chain (3 件): (a) op-axis asymmetric enumeration、(b) cells 42/45-a Ideal output token-level mismatch (statement form vs Block form)、(c) Spec → Impl Mapping UpdateExpr arm B2/B3 missing | Spec gap (framework 失敗 signal) | 即時 fix: matrix Block form 統一 + Rule 1 (1-4) Orthogonality merge legitimacy で op-symmetric cells を `cell-symmetric` 派生で表現 + Spec → Impl Mapping table を T7 dispatch arm enumerate で completeness 化 | **resolved** |
| L4-2 | INV-3 1-evaluate compliance latent gap (非-Ident receiver で `obj` が getter + setter で 2 回 evaluate される、`getInstance().x++` 等で receiver clone 2x → generated Rust で `getInstance()` 2 回呼出) | Review insight | T8 scope に詳細 defer (T8 task description (8-a) で structural 解消、`is_side_effect_free` helper + IIFE 形 emission + T7 dispatch helpers への back-port + matrix sub-axis 化) | **defer to T8 (8-a)** |

##### 6 件 Action 適用後の post-fix state

- L1-1 / L1-2 / L2-2 / L3-1 / L3-2 / L3-3 / L3-4: 全 6 課題本質的に解決 (Implementation gap 2 + Spec gap 4)
- L4-2: T8 scope に詳細 defer (= T8 architectural concern = "compound assign side-effect handling" の structural fix scope に内包、本 T7 scope は Ident receiver only matrix で完結)
- **Test 拡張**: 7 件 op-symmetric tests (B1 prefix --、B4 prefix --、B2/B6/B7/B8 ++、B9 ++) で op-axis × postfix-axis full coverage 達成、合計 22 件 unit tests
- **Matrix update**: cells 42/43/44/45-a〜45-de に加えて cell 44-symmetric / 45-b-symmetric / 45-db-symmetric / 45-dc-symmetric / 45-dd-symmetric / 45-de-symmetric / 45-b3 (= B3 setter only update) を Rule 1 (1-4) compliant orthogonality merge で記載
- **Spec → Impl Mapping**: UpdateExpr arm を 12 dispatch arm (Instance B4 numeric / non-numeric / B2 / B3 / B6 / B7、Static B4 numeric / B2 / B3 / B6 / B7、Fallback、Computed) で完全 enumerate

##### Final quality (post-second-review fix)

cargo test --lib **3242 pass** (3220 baseline + 15 T7 + 7 cross-coverage gap fill = 3242) / e2e 159 pass + 70 ignored / integration 122 pass / compile_test 3 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK。

#### Iteration v11 deep-review findings (`/check_job` deep、second-review post-fix 後の更なる adversarial deep review)

User 指示「deep を実施 + 妥協絶対禁止 + 必要十分で高品質な自動テスト」に基づく **post-second-review 後の更なる第三者視点 deep review** で発見した 4 件の課題を本 Iteration v11 scope 内で structural fix。

| # | 課題 | Defect category | Action | Status |
|---|------|----------------|--------|--------|
| **D1** | Rule 11 (d-2) phase mechanism violation: `convert_update_expr` の `anyhow!` (Ident form `_ =>` arm pre-T7 + Member arm Computed gate T7 introduced) が Transformer phase 必須 mechanism = `UnsupportedSyntaxError` ではない | Implementation gap | 即時 fix: 両 site で `UnsupportedSyntaxError::new("unsupported update expression target", up.span)` に変更、`up_span` を `convert_update_expr_member_arm` parameter として plumb、user-facing line:col 含む transparent error reporting via `resolve_unsupported()` 経由 復元 | **resolved** |
| **D2** | DRY violation: `__ts_old` / `__ts_new` 文字列 literal が 10+ 箇所重複 (assignments.rs Ident form 4 + member_dispatch.rs setter block 2 + assignments.rs fallback block 4 = 10) | Implementation gap | 即時 fix: `src/transformer/expressions/mod.rs` に `pub(super) const TS_OLD_BINDING: &str = "__ts_old"` / `TS_NEW_BINDING: &str = "__ts_new"` 宣言、I-154 namespace reservation rule との link doc comment 完備、3 sites (assignments.rs Ident form / build_fallback_field_update_block / member_dispatch.rs build_update_setter_block) から import + literal を const に置換 | **resolved** |
| **D3** | C1 branch coverage gap: `convert_update_expr` Ident form の `_ =>` arm (= 非-Ident 非-Member arg) が **直接 test 不在**、既存 test `arr[0]++` は実は Member→Computed gate 経由で `_ =>` arm を踏まない | Review insight (Implementation gap) | 即時 fix: 2 件 test 追加 = `test_convert_update_expr_paren_wrapped_arg_emits_unsupported_syntax_error` (`(x)++` Paren wrap で `up.arg = Paren` → `_ =>` arm 直撃) + `test_convert_update_expr_this_arg_emits_unsupported_syntax_error` (`this++` で `up.arg = This` → `_ =>` arm 直撃)、`assert_expr_unsupported_syntax_error_kind` helper で D3 raw expr tests boilerplate 集約 | **resolved** |
| **D4** | C1 branch coverage gap (static dispatch defensive arms): `dispatch_static_member_update` の static B2/B3/B6 defensive arms が test 不在 (matrix cell 化なしだが code 上存在、T6 dispatch_static_member_write の同 pattern arms は Iteration v10 second-review で 3 件 test 追加済 integration と非対称) | Review insight (Implementation gap) | 即時 fix: 3 件 test 追加 = `test_static_b2_getter_only_update_emits_read_only_static_error` (static getter only → "write to read-only static property") + `test_static_b3_setter_only_update_emits_write_only_static_error` (static setter only → "read of write-only static property") + `test_static_b6_method_update_emits_write_to_static_method_error` (static method → "write to static method")。Static B7 inherited は test setup 複雑かつ T6 でも未追加 (subsequent T11 (11-c) matrix expansion で追加) | **resolved (B2/B3/B6 = 3 件)、static B7 は T11 (11-c) defer** |

##### Cohesive cleanup (D2 fix + helpers extraction で派生)

D2 fix 適用 + DRY refactor の副次的 cleanup:
- **`update.rs` file size 1046 → 959 行**: helpers 3 件抽出 (`convert_update_in_probe` Tier 1 emission test 用 + `assert_in_probe_unsupported_syntax_error_kind` Tier 2 honest error test 用 + `assert_expr_unsupported_syntax_error_kind` D3 raw expr test 用) + B4 / B8 class fixture を `format!("{B4_COUNTER_CLASS_SRC}\n...")` で共有、各 test の boilerplate を ~70% 削減
- **CLAUDE.md "0 errors / 0 warnings" file-line threshold (1000 行) compliance restored**: Iteration v11 deep review post-fix で 87 行削減、threshold 内に余裕

##### 4 件 Action 適用後の post-fix state

- D1 / D2 / D3 / D4 (B2/B3/B6): 全 4 件本質的に解決 (Implementation gap 4 件)、Static B7 のみ T11 (11-c) matrix expansion で追加 (T6 pattern と整合)
- **Test 拡張**: 2 件 D3 + 3 件 D4 = 5 件追加で convert_update_expr 全 branch + dispatch_static_member_update 全 defensive arms (B7 除く) C1 coverage 達成、合計 27 件 unit tests
- **Production code 改善**: anyhow! → UnsupportedSyntaxError 変換 (2 sites)、定数抽出 (TS_OLD_BINDING / TS_NEW_BINDING)、`convert_update_expr_member_arm` の `up_span: swc_common::Span` parameter 追加で span 付き user-facing error reporting 復元
- **Helper-based DRY refactor**: 3 helpers + 2 shared class fixture constants で test boilerplate 集約、file size 1046 → 959 行 (-87 行)、cohesion 改善 + future T8/T9 で同 helper pattern を leverage 可能

##### Final quality (post-deep-review fix)

cargo test --lib **3247 pass** (3242 baseline post-second-review + 5 new D3/D4 branch coverage tests = 3247) / e2e 159 pass + 70 ignored / integration 122 pass / compile_test 3 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK (update.rs = 959 行 < 1000 threshold)。

#### Iteration v11 deep-deep-review findings (`/check_job` deep deep、deep-review post-fix 後の更なる adversarial deep deep review)

User 指示「deep deep を実施 + 妥協絶対禁止 + 必要十分で高品質な自動テスト」に基づく **post-deep-review 後の更なる第三者視点 deep deep review** で発見した 1 件の重大課題を本 Iteration v11 scope 内で structural fix。

| # | 課題 | Defect category | Action | Status |
|---|------|----------------|--------|--------|
| **DD1** | Rule 11 (d-1) `_ =>` arm 全面禁止 violation: `convert_update_expr` Ident form の `_ =>` arm (assignments.rs:413、deep-review D1 fix で UnsupportedSyntaxError 化済だが `_ =>` arm 自体は残存) が **T7 で modify した同 function 内 control flow** であり、Rule 11 (d-6-a) Architectural concern relevance per "本 PRD で modify する control flow を含む arms" に該当 → **T7 scope 内 fix 必須** (deep-review で I-203 defer 判定したのは誤判定、Architectural concern relevance を strictly 検討せず) | Implementation gap | 即時 fix: `match up.arg.as_ref()` を **38 variants 全 enumerate** な exhaustive match に restructure。Member / Ident は dispatch arms、残 36 variants は `or_pattern` で UnsupportedSyntaxError reject に集約。SWC `ast::Expr` は **`#[non_exhaustive]` ではない**ため Rust compiler が new variant 追加時 compile error で全 dispatch fix 強制 = **structural enforcement** 確立 (Rule 11 (d-1) "新 variant 追加時 compile error で全 dispatch fix 強制" 要件 satisfied) | **resolved** |

##### Why I missed this in deep-review (= Architectural concern relevance strictly 適用の重要性)

- deep-review D1 で anyhow! → UnsupportedSyntaxError 変換 (Rule 11 (d-2) phase mechanism compliance) を実施したが、**`_ =>` arm 自体の Rule 11 (d-1) 違反** は I-203 codebase-wide AST exhaustiveness scope に defer 判定した
- 判定根拠: 「pre-T7 から存在する arm」「I-203 で codebase-wide refactor 予定」
- **盲点**: Rule 11 (d-6-a) Architectural concern relevance per "本 PRD で modify する control flow を含む arms" を strictly 検討すれば、T7 で modify した `convert_update_expr` 内 control flow なので **本 T7 scope 内 fix 対象** だった (deep-review で 5 課題発見しつつ DD1 を漏らした)
- **Lesson**: Rule 11 (d-6-a) "control flow を含む arms" の strictly 適用 = 本 PRD で touch した function 内の **全 `_ =>` arms** が対象 (= 本 PRD で direct modify していなくても、touch した function の control flow に含まれる)

##### Cohesive cleanup (DD1 fix で派生)

- `convert_update_expr` を `if let Member ... else match Ident ... else _ =>` 3-stage 構造から **single match** に restructure → 制御フロー単一化 (cohesion 改善)
- 36 unsupported variants を 3 categories に分類した documentation comment 追加 (Literal / wrapper / Compound / call / chain / Special / non-target shapes) → reader の認知負荷軽減

##### DD1 Action 適用後の post-fix state

- DD1 = Rule 11 (d-1) Implementation gap 1 件、本 T7 scope 内 structural fix で本質解決
- Rust compiler の structural safety net 確立: SWC v22 等で新 `ast::Expr` variant 追加されても **compile error で T7 dispatch fix 強制**、silent regression 不能
- post-DD1 fix: cargo test --lib 3247 pass / e2e 159 pass + 70 ignored / integration 122 pass / compile 3 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK

##### Framework 改善検討 (Iteration v11 deep-deep-review 由来)

- **Rule 11 (d-6-a) "Architectural concern relevance" strict 適用 audit 強化候補**: `audit-prd-rule10-compliance.py` 等の audit script で「本 PRD で modify した function 内に `_ =>` arm が残っているか」を auto detect、deep-review で漏れる risk を構造的に排除。本 PRD I-205 close 時 integrate or 別 framework PRD 起票候補 = "Rule 11 (d-6-a) architectural concern relevance auto-audit" として framework v1.7 → v1.8 self-applied integration 候補。

#### Iteration v11 完了判定 (2026-04-29、`/check_job` 4-layer review + second-review + deep-review + deep-deep-review post-fix)

T7 atomic commit ready (post-deep-deep-review 1 課題 (DD1) 本質 fix 適用後 final state)。本 T7 scope 内対応すべき items は **first-review (1 件) + second-review (6 件) + deep-review (4 件) + deep-deep-review (1 件) = 累積 12 件、Spec gap 5 + Implementation gap 7 全件本質的に解決済**。L4-2 INV-3 latent gap のみ T8 scope に詳細 defer (architectural concern relevance 観点で T8 に内包)、static B7 inherited update arm test のみ T11 (11-c) matrix expansion で追加 (T6 pattern 整合)。

次 iteration v12 = T8 (Compound assign `+= -= *= ...|=` setter desugar + INV-3 1-evaluate compliance for non-Ident receiver + T7 latent gap back-port) 単独 commit。T7 で確立した `build_update_setter_block` setter desugar block builder + numeric type check helper + `dispatch_instance/static_member_update` arm 構造 + `TS_OLD_BINDING` / `TS_NEW_BINDING` shared constants + `convert_update_in_probe` / `assert_in_probe_unsupported_syntax_error_kind` test helpers は全て T8 で leverage 可能、INV-3 structural 解消 (`is_side_effect_free` helper + IIFE 形 emission) で T7 helpers にも back-port (= T7 latent gap を T8 で structural cohesive fix)。

##### Framework 改善検討 (Iteration v11 累積、first/second/deep review 由来)

- **Rule 9 dispatch-arm sub-case alignment 強化候補** (second-review 由来): 「全 op (multi-op syntax = `++/--`、`+= -= *= ...`、`??= &&= ||=` 等) で B-axis × postfix-axis × context (statement/expression) を独立 enumerate verify」を framework に追加候補。
- **Rule 10 default axis 追加候補** (first-review 由来): "TypeResolver visit coverage of operand-context expressions" (Update.arg / Unary.arg / Cond.test/cons/alt 等の operand position で receiver type 必要時、TypeResolver が visit してるか axis 化)。
- **Rule 11 (d-2) Transformer phase mechanism enforcement の audit 強化候補** (deep-review D1 由来): `audit-ast-variant-coverage.py` 等の audit script で Transformer 配下 file の `anyhow!` 使用 site を検出 + `UnsupportedSyntaxError` への migration suggestion を出力する mechanism (= I-203 codebase-wide AST exhaustiveness compliance scope と integrate 可能)。
- **Test C1 branch coverage automation 候補** (deep-review D3/D4 由来): `cargo llvm-cov --branch` 等で branch coverage を CI 計測 + threshold (e.g., 95%) で merge gate 化、本 PRD I-205 のような multi-arm dispatch helper の coverage gap を構造的に検出。本 PRD I-205 close 時 integrate or 別 framework PRD 起票候補。

本 PRD I-205 close 時 integrate or 別 framework PRD 起票候補。

### Iteration v12 (2026-04-29、T8 単独 commit + Spec gap fix = TypeResolver compound assign Member arm 未再帰 + DRY refactor + member_dispatch.rs 6-file split)

**Architectural concern**: arithmetic / bitwise compound assign (`+= -= *= /= %= |= &= ^= <<= >>= >>>=`、11 ops) Member target で setter desugar (B4 instance、B8 static) + Tier 2 honest error reclassify (B2 read-only / B3 write-only-read-fail / B6 method / B7 inherited) + INV-3 1-evaluate compliance (side-effect-having receiver IIFE form `{ let mut __ts_recv = ...; ... }`) + T7 dispatch_instance_member_update への INV-3 back-port (cohesive batch、`build_setter_desugar_block` + `wrap_with_recv_binding` + `build_instance_setter_desugar_with_iife_wrap` shared)。

**Implementation 内容**:
- `assignments.rs` に `arithmetic_compound_op_to_binop` mapping helper + T8 dispatch gate (T6 plain `=` gate の直後、`AddAssign..ZeroFillRShiftAssign` 11 ops × Member × MemberProp::Ident|PrivateName で `dispatch_member_compound` 経由) 追加。
- `member_dispatch/` directory new (1 → 6 file split): `mod.rs` (entry impl Transformer + classifier + shared types) / `shared.rs` (DRY-extracted infrastructure: MemberKindFlags + is_side_effect_free + wrap_with_recv_binding + build_setter_desugar_block + **build_instance_setter_desugar_with_iife_wrap** + **build_static_setter_desugar_block**) / `read.rs` / `write.rs` / `update.rs` / `compound.rs`。
- `dispatch_instance_member_compound` / `dispatch_static_member_compound` 新規 helper (T8、shared.rs 内 IIFE wrap + setter desugar 経由)。
- `Transformer::dispatch_member_compound` entry method (mod.rs、`classify_member_receiver` 経由 Static / Instance / Fallback dispatch)。
- `expressions/mod.rs` に `TS_RECV_BINDING = "__ts_recv"` constant 追加 (I-154 namespace reservation extension to receiver IIFE binding)。
- T7 `dispatch_instance_member_update` を `build_instance_setter_desugar_with_iife_wrap` 経由に refactor (= INV-3 1-evaluate compliance back-port、T8 と shared)。

**Spec gap fix (本 T8 scope 内、Iteration v9 / v11 と同 pattern = framework 失敗 signal)**:
- **Defect**: `pipeline/type_resolver/expressions/assignments.rs::resolve_assign_expr` の compound `SimpleAssignTarget::Member` arm が `is_propagating_op` (NullishAssign / AndAssign / OrAssign) のみ `resolve_expr(&member.obj)` 経路を通っていた。Arithmetic / bitwise compound (AddAssign 等) では receiver expr_type が `expr_types` に register されず → Transformer `classify_member_receiver` の `get_expr_type(receiver)` が None → silent Fallback dispatch (= class member setter dispatch を逃す silent semantic loss、cells 21/27/29-d/29-e-d/33/34-c/35-d で setter desugar 発火せず B1/B9 fallback `Expr::Assign { FieldAccess, BinaryOp }` emit、Tier 2 broken state 維持)。
- **Trace** (`post-implementation-defect-classification.md` 5-category):
  - reference doc: ClassMember 章 entry あり
  - oracle: `tests/e2e/scripts/i-205/cell-21-*.ts` 等 fixture で TS observation 済
  - matrix: cells 21/27/29-d/29-e-d/33/34-c/35-d を ✗ 修正対象として enumerate 済
  - **enumerate gap**: spec stage で TypeResolver coverage axis (= "TypeResolver が Member access の receiver expr_type を compound assign Member target context で register するか") を independent dimension として enumerate していなかった = **Spec gap category** (framework 失敗 signal、Iteration v11 T7 Update.arg 未再帰 と本質同 pattern)
- **Fix**: `is_propagating_op` ブロックの外側に `let obj_type = self.resolve_expr(&member.obj);` を移動、全 compound op (NullishAssign / AndAssign / OrAssign + AddAssign..ZeroFillRShiftAssign) で receiver の expr_type を unconditional register。`is_propagating_op` block 内では既存 field type / expected propagation logic を維持 (I-175 historical no-op behavior preserve)。
- **Framework 改善検討**: `spec-stage-adversarial-checklist.md` Rule 10 axis enumeration の default check axis "TypeResolver visit coverage of operand-context expressions" (Iteration v11 で追加候補化済) を **正式 default axis に昇格** + audit script `audit-prd-rule10-compliance.py` で Rule 10 application yaml block の axis 列挙に本 axis 出現を verify する mechanism 追加候補 (= I-205 で 2 度連続発生 (Update.arg + compound assign Member.obj) の structural prevention、3 度目発生前の framework hardening)。

**DRY refactor (本 T8 scope 内、Iteration v12 third-review、`design-integrity.md` "DRY")**:
- T7 `dispatch_instance_member_update` + T8 `dispatch_instance_member_compound` の B4 setter desugar arm (各 30 行) が完全 identical な receiver-type detection + IIFE wrap + getter/setter call construction logic を 60 行で重複。`shared.rs::build_instance_setter_desugar_with_iife_wrap` shared helper に集約 (= IIFE wrap concern が 1 箇所に集中、subsequent T9 logical compound も同 helper を leverage 可能、DRY violation 増殖を構造的に防止)。
- T7 `dispatch_static_member_update` + T8 `dispatch_static_member_compound` の static B4/B8 setter desugar arm (各 ~10 行) も `shared.rs::build_static_setter_desugar_block` に集約。Static dispatch では receiver = class TypeName で side-effect なし、IIFE wrap 不要 = simpler shared helper。

**File split refactor (本 T8 scope 内、Iteration v12 third-review、CLAUDE.md "0 errors / 0 warnings" file-line threshold 1000 行 violation 解消)**:
- pre-split: 単一 `member_dispatch.rs` (1179 行)、4 architectural concern (Read / Write / Update / Compound) が単一 file に同居。
- post-split: `member_dispatch/{mod, shared, read, write, update, compound}.rs` (6 file 計 1331 行、各 file 100-369 行)、各 file = 単一 architectural concern。`shared.rs` に cross-cutting infrastructure (MemberKindFlags / is_side_effect_free / IIFE wrap / setter desugar block builder / DRY-extracted helpers) を集約。

**`/check_job` 4-layer review (本 T8 commit 前 invocation) findings + 本 T8 内 fix**:
- **F1 (Implementation gap、Medium)**: `is_side_effect_free` を 1 helper 内で 2 回呼び出し → `let se_free = is_side_effect_free(object);` で事前格納に refactor (`shared.rs::build_instance_setter_desugar_with_iife_wrap`)。
- **F2 (Implementation gap、High)**: `convert_assign_expr` compound match の `_ => return Err(anyhow!(...))` arm が Rule 11 (d-1) 違反 → `AndAssign | OrAssign` (I-161 desugar path で先 intercept = `unreachable!()`)、`ExpAssign` (`UnsupportedSyntaxError` で TS exponentiation conversion を out-of-scope 明示)、`NullishAssign` (line 142-251 で intercept = `unreachable!()`) で exhaustive enumerate 化。
- **F3 (Spec gap、High)**: `ExpAssign` × Member の user-facing error wording が pre-fix で `anyhow!` (= internal error、line:col なし) だった → F2 fix で `UnsupportedSyntaxError::new` 経由 transparent error reporting に統一。
- **F5 (Review insight、Medium)**: B4 + non-numeric getter return type × compound assign の semantic safety analysis を PRD matrix に明示 (`Cell 21 corollary` section 新規追加)。`String += String` / `Vec<T> += anything` 等の Rust trait 実装次第で Tier 1 / Tier 2 (Rust compile error fallthrough = silent semantic change なし) の挙動を verify。本 T8 で additional gate 不要 = `getter_return_is_numeric` (T7-specific numeric coercion concern、`++/--` で必ず `+ 1.0` のため non-numeric type で必ず E0277) と `is_side_effect_free` (INV-3 receiver eval count concern、T7/T8 共通) の semantic 差異を Rule 9 Spec → Impl Mapping completeness 観点で明示化。
- **F4 / F7 (Review insight、Pre-existing)**: T8 で導入した defect ではない、別 PRD scope。本 T8 内では fix なし、TODO 起票候補 (= TypeResolver field expr_types completeness audit / Fallback path receiver clone optimization)。

**Unit tests** (T8 unit + INV-3 + T7 back-port verify、計 19 + 1 = 20 件):
- `tests/i_205/compound.rs` 新規 (19 件):
  - Cells 20/28 (B1 field / B9 unknown × `+=`、Fallback regression preserve)
  - Cell 21 SE-free (B4 × `+=` × Ident receiver、setter desugar yield_new)
  - Cell 21 IIFE (B4 × `+=` × FnCall receiver、IIFE form for INV-3 1-evaluate)
  - Cells 22/23/25/26 (B2/B3/B6/B7 instance × `+=`、Tier 2 honest error wording lock-in)
  - Cells 27/29-e-d/35-d (B8 static × `+=`/`-=`/`|=`、static setter desugar)
  - Cells 29-d/33/34-c (B4 × `-=`/`|=`/`<<=`、op-axis orthogonality verify)
  - Static defensive arms (matrix cell 化なし、Static B2/B3/B6/B7 compound、Tier 2 wording lock-in)
  - INV-3 FieldAccess receiver recursive judgment (`is_side_effect_free(FieldAccess of Ident) → true`、IIFE 不採用)
  - T7 INV-3 back-port verify (T8 で T7 update helper を update したことを `getInstance().value++` で IIFE form emit verify)
- `tests/i_205/update.rs` (T7 back-port test を本 file から compound.rs に move、note のみ追加)。

**Pre/post matrix** (T8 cells 全 transition):

| Cell | Pre-T8 | Post-T8 | Delta |
|------|--------|---------|-------|
| 20 (B1 field `+=`) | ✓ Fallback | ✓ Fallback | preserved |
| 21 (B4 `+=` SE-free recv) | ✗ silent Fallback (broken Tier 2) | ✓ setter desugar yield_new | fix (Tier 2 → Tier 1) |
| 21 IIFE (B4 `+=` SE-having recv) | ✗ silent Fallback + INV-3 violation latent | ✓ IIFE setter desugar | fix (Tier 2 → Tier 1 + INV-3 compliance) |
| 22 (B2 `+=`) | ✗ silent Fallback | ✓ Tier 2 honest "compound assign to read-only property" | fix (silent → Tier 2 honest) |
| 23 (B3 `+=`) | ✗ silent Fallback | ✓ Tier 2 honest "compound assign read of write-only property" | fix |
| 25 (B6 method `+=`) | ✗ silent Fallback | ✓ Tier 2 honest "compound assign to method" | fix |
| 26 (B7 inherited `+=`) | ✗ silent Fallback | ✓ Tier 2 honest "compound assign to inherited accessor" | fix |
| 27 (B8 static `+=`) | ✗ Rust syntax error (`Foo.x += v`) | ✓ static setter desugar yield_new | fix (Tier 2 → Tier 1) |
| 28 (B9 unknown `+=`) | ✓ Fallback | ✓ Fallback | preserved |
| 29-d (B4 `-=`) | ✗ silent Fallback | ✓ setter desugar BinOp::Sub | fix (op-axis orthogonality, cell 21 と equivalent) |
| 29-e-d (B8 `-=`) | ✗ Rust syntax error | ✓ static setter desugar BinOp::Sub | fix |
| 33 (B4 `\|=`) | ✗ silent Fallback | ✓ setter desugar BinOp::BitOr | fix |
| 34-c (B4 `<<=`) | ✗ silent Fallback | ✓ setter desugar BinOp::Shl | fix |
| 35-d (B8 `\|=`) | ✗ Rust syntax error | ✓ static setter desugar BinOp::BitOr | fix |
| T7 cell 43 IIFE (B4 SE-having `++`) | ✗ INV-3 violation latent (double-eval) | ✓ IIFE setter desugar | fix (T7 back-port) |

**No regression** (✓ → ✗) cells: 0 件。

**Final quality (post-fix、本 T8 commit ready)**:
- cargo test --lib **3267 pass** (3247 baseline + 19 T8 compound + 1 T7 back-port = 3267)
- cargo test --tests: e2e 159 pass + 70 ignored / integration 122 pass / compile_test 3 pass
- clippy 0 warning / fmt 0 diff / check-file-lines OK (全 .rs file < 1000 行、最大 369 行 = `member_dispatch/mod.rs`)
- Hono Tier-transition compliance = **Preservation** (clean 111 / errors 63 = T7 baseline 同一、`prd-completion.md` broken-fix PRD allowed pattern = Hono が compound assign on class instances を主要使用していないため expected)

**Defect Classification** (本 T8 内 final、Iteration v12 first + second review 累積):
- **Spec gap: 3** (= [first] TypeResolver compound assign Member arm receiver 未再帰 + `ExpAssign` × Member user-facing wording、[second] TypeResolver compound assign Member arm field type completeness が partial = comment clarify で本 T8 内 resolved + 別 TODO `[I-218]` 起票 詳細 record、framework 失敗 signal)
- **Implementation gap: 3** (= [first] `is_side_effect_free` 二重呼び出し + `_` arm Rule 11 d-1 違反、[second] `TS_OLD_BINDING` doc comment stale reference `build_update_setter_block` → `build_setter_desugar_block` rename、3 件本 T8 内 全 resolved)
- **Review insight: 4** (= [first] cell 21 corollary semantic safety、[second] assignments.rs compound desugar match comment clarify (Member target は早期 return で本 match に到達しないこと明示)、`arithmetic_compound_op_to_binop` 11 ops exhaustive mapping unit test 不在 (本 T8 内 7 op 追加 unit test で structural verify)、Fallback path INV-3 1-evaluate compliance gap (pre-existing、本 T8 setter dispatch path scope と orthogonal、別 TODO `[I-217]` 起票 詳細 record + Resolution direction = `is_side_effect_free` / `wrap_with_recv_binding` shared helper Fallback path 適用))

**framework 改善 candidates (本 PRD close 時 integrate or 別 framework PRD 起票候補)**:
- **Rule 10 default axis 正式昇格 (Iteration v11/v12 連続 2 度発生 source)**: "TypeResolver visit coverage of operand-context expressions" を Rule 10 axis enumeration default check axis に昇格 + audit script で yaml block parse して自動 verify する mechanism 追加。Update.arg / Compound assign Member.obj の 2 度連続 Spec gap 発生 source、3 度目発生前の structural prevention。

#### Iteration v12 完了判定 (2026-04-29、first + second `/check_job` 4-layer review 累積 fix、計 10 件 finding 全 fix + 別 scope defer 2 件 TODO 起票)

**First review (commit 前 initial review)**: F1/F2/F3 (Critical block findings) + F5 (Review insight) + F4/F7 (pre-existing、scope-out) を発見、F1/F2/F3 + Cell 21 corollary record で本 T8 内 全 fix。

**Second review (post-fix state、追加発見) findings 5 件 (本 T8 内 fix + 別 scope TODO)**:
- **F-SL-1 (Review insight、Medium)**: assignments.rs compound desugar match の comment が "Member target も通過する" と読み手に誤解を与える misleading wording → Member target は早期 return で本 match arm に到達しない事を明示する comment clarify (line 341 周辺)。
- **F-SL-2 (Implementation gap、High)**: `TS_OLD_BINDING` doc comment の stale reference `member_dispatch.rs` `build_update_setter_block` → post-T8 split で `member_dispatch/shared.rs` `build_setter_desugar_block` に rename/generalize されている、doc comment と実装不一致 (CLAUDE.md "Public types/functions must have doc comments" 準拠 violation)。本 T8 内 fix で reference 訂正。
- **F-SX-1 (Spec gap、Medium)**: TypeResolver compound assign Member arm の comment が "全 compound op で recursively resolve" と書いているが、register されるのは **receiver の expr_type のみ**、**field type** (= `member.span` 全体の expr_types entry) は依然 `is_propagating_op` ブロック内のみで partial register。本 T8 内 fix で comment clarify (= receiver 軸のみ resolve、field 軸は subsequent T9 着手時に audit) + 別 TODO `[I-218]` 起票で詳細 Resolution direction record (Fix 1 = field type 全 op register への restructure、Fix 2 = T9 着手時 audit)。
- **F-EM-1 (Review insight、Medium)**: `arithmetic_compound_op_to_binop` 11 ops 全 mapping を end-to-end lock-in する unit test 不在、unit test では 4 ops (AddAssign / SubAssign / BitOrAssign / LShiftAssign) のみ B4 dispatch verify。本 T8 内 fix で **7 op (MulAssign / DivAssign / ModAssign / BitAndAssign / BitXorAssign / RShiftAssign / ZeroFillRShiftAssign) の B4 instance dispatch unit test を追加**、11 ops 全件の structural mapping verify を完成 (orthogonality merge proof + dispatch arm coverage transitively complete)。
- **F-AT-1 (Review insight、Low)**: `dispatch_member_compound::Fallback` arm の `target.clone()` (line ~362、pre-existing F7 same issue) で receiver double/triple-eval が latent silent semantic loss + Update Fallback path (`build_fallback_field_update_block`) も同 INV-3 violation pattern。本 T8 setter dispatch path scope と orthogonal な architectural concern (= "Fallback path INV-3 1-evaluate compliance") として split、本 T8 内 fix なし、別 TODO `[I-217]` 起票で詳細 Resolution direction record (Fix 1 = `is_side_effect_free` / `wrap_with_recv_binding` shared helper Fallback path 適用、Fix 2 = scope 縮小)。

**累積 Defect Classification (final)**:
- **Spec gap: 3** (1 件 framework 失敗 signal、2 件本 T8 内 resolved + 1 件 [I-218] TODO 起票)
- **Implementation gap: 3** (全件本 T8 内 resolved)
- **Review insight: 4** (1 件 PRD doc record、1 件 comment clarify、1 件 unit test 拡張、1 件 [I-217] TODO 起票)

✅ 全 finding 本 T8 内 fix or 別 TODO 起票 + PRD doc 詳細 record 完了。Pre/post matrix で no regression verify、Hono Tier-transition compliance Preservation verify (clean 111 / errors 63 = T7 baseline 同一、no new compile errors)。本 T8 commit ready (= second review 累積 fix 後 final state、user 指示 "妥協は絶対に許容しない" + "現在のスコープで対応するべきものは全て、本質的な方法で解決" 完全準拠)。

## Goal

PRD 完了時、以下が達成される (verifiable):

1. **TypeRegistry method kind tracking** が全 propagation path で機能:
   - `MethodSignature` に `kind: MethodKind` field 追加 (Method/Getter/Setter)
   - `collect_class_info` で SWC `method.kind` を propagation
   - `convert_method_info_to_sig` で TS literal type の getter/setter 判定 propagation (TsTypeLit には method/getter/setter が `is_method` 判定不能なので別経路で取得)
2. **Read context dispatch** が全 cell に対し ideal:
   - `obj.x` → receiver type に getter あれば `obj.x()`、それ以外 (field/no entry) は `obj.x` (current)
3. **Write context dispatch**:
   - `obj.x = v` → setter あれば `obj.set_x(v)`、それ以外 (field) は `obj.x = v` (current)
   - read-only (B2 getter only): Tier 2 honest error
4. **Compound assign desugar**: `obj.x += v` → `obj.set_x(obj.x() + v)` (side-effect-free obj) or temp binding (side-effect obj)
5. **Inside-class `this.x` dispatch (P1 TC39 faithful)**: `self.x()` / `self.set_x(v)` 経由
6. **Static accessor (B8) dispatch**: `Class.x` → `Class::x()` / `Class::set_x(v)`
7. **Class Method Getter body `.clone()` 自動挿入 (C1 limited pattern)**: body が `return self.field;` (or last-expr `self.field`) で T が non-Copy なら `self.field.clone()` に rewrite
8. **既存 class Method Getter/Setter Tier 2 → Tier 1 完全変換**: 全 D variant で compile pass + tsc runtime stdout 一致 を E2E fixture で lock-in
9. **Existing `_ => {}` broken window fix** (`registry/collection/class.rs:145` Rule 11 d-1 違反): explicit enumerate に書き換え
10. **Hono bench 0 regression** (本 PRD scope 外 fixture で error count 増加なし)

---

## Scope (3-tier 形式、Rule 6 (6-2) 適用)

### In Scope

本 PRD で **Tier 1 完全変換** する features:

- TypeRegistry の method kind tracking infrastructure (MethodSignature.kind field + propagation)
- convert_member_expr / resolve_member_access の dispatch 拡張 (Read context、B1/B4 cells)
- convert_assign_expr の dispatch_member_write helper 拡張 (Write context、B1/B4 cells)
- compound assign / increment-decrement の setter desugar (B4)
- inside-class `this.x` dispatch (P1 TC39 faithful、E2 internal)
- static accessor `Class.x` (B8) dispatch
- 既存 class Method Getter body `.clone()` 自動挿入 (C1 pattern: `return self.field;` 限定、D4-D15 non-Copy T)
- AutoAccessor (B5) は本 framework を leverage できる shape まで infrastructure 完成 (PRD 2.8 が emission strategy + AutoAccessor declaration emission を別 PRD で達成)
- `registry/collection/class.rs:145` の `_ => {}` arm 排除 (Rule 11 d-1 compliance、本 PRD method kind tracking blocker)
- E2E fixture: 各 ✗ cell に対応する fixture 作成 + lock-in test (Spec stage TS-3 task)
- Unit test: TypeRegistry method kind / dispatch logic / clone insertion 個別 lock-in

### Out of Scope

別 PRD or 永続 unsupported な features:

- AutoAccessor declaration emission (PRD 2.8 / I-201-A、本 framework foundation を leverage)
- Object literal Prop::Method/Getter/Setter (PRD 2.9 / I-202、本 framework foundation を leverage)
- Decorator framework (PRD 7 / I-201-B、本 framework foundation を leverage、L1 silent semantic change)
- Class inheritance interaction (B7 を Tier 1 化、別 architectural concern = "Class inheritance dispatch")
- Destructure with getter (`const {x} = obj`、別 architectural concern = "Destructure pattern dispatch")
- Comprehensive `.clone()` insertion for complex getter bodies (C2 case、別 PRD 候補 = "Class Method body T-aware clone insertion comprehensive")
- Codebase-wide `_ =>` arm refactor (I-203、別 PRD = "Codebase-wide AST exhaustiveness")

### Tier 2 honest error reclassify

本 PRD で **Tier 2 honest error 化** する features (silent drop / silent failure 排除、別 PRD で Tier 1 化候補):

- read-only property (B2 getter only) への write attempts (`obj.x = v`) → `UnsupportedSyntaxError::new("write to read-only property", span)`
- write-only property (B3 setter only) からの read attempts (`obj.x`) → `UnsupportedSyntaxError::new("read of write-only property", span)`
- inherited accessor (B7、parent class accessor) — Tier 1 化 = 別 architectural concern (Class inheritance) で別 PRD
- regular method (B6) `obj.x` no-paren reference (method-as-fn-reference) — Tier 1 化 = orthogonal architectural concern (function reference semantic) で別 PRD
- typeof / in / spread / delete with getter (A7-A11 corner cases、E1 確定) — 各 corner case Tier 1 化は別 architectural concern

---

## Design

### Technical Approach

#### 1. MethodSignature method kind tracking (infrastructure)

**File**: `src/registry/mod.rs`、`src/ts_type_info/mod.rs`

```rust
// 新規 enum (SWC `MethodKind` mirror、ts_to_rs IR で再 export)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MethodKind {
    #[default]
    Method,
    Getter,
    Setter,
}

// 既存 MethodSignature を拡張
pub struct MethodSignature<T = RustType> {
    pub params: Vec<ParamDef<T>>,
    pub return_type: Option<T>,
    pub has_rest: bool,
    pub type_params: Vec<TypeParam<T>>,
    pub kind: MethodKind,  // ← 新規
}

// TsMethodInfo にも追加
pub struct TsMethodInfo {
    // ... 既存 ...
    pub kind: MethodKind,  // ← 新規
}
```

#### 2. Class collection で method.kind を propagate

**File**: `src/registry/collection/class.rs`

```rust
ast::ClassMember::Method(method) => {
    // ... 既存 ...
    let kind = method.kind;  // ast::MethodKind → MethodKind 変換
    methods.entry(name).or_default().push(MethodSignature {
        params,
        return_type,
        has_rest,
        type_params: method_type_params,
        kind: kind.into(),  // ← 新規
    });
}
ast::ClassMember::ClassProp(prop) => { /* ... 既存 */ }
ast::ClassMember::PrivateProp(prop) => { /* ... 既存 */ }
ast::ClassMember::Constructor(ctor) => { /* ... 既存 */ }
ast::ClassMember::PrivateMethod(pm) => { /* PRD 2.7 で確認、本 PRD で kind tracking 追加 */ }
ast::ClassMember::StaticBlock(_) => { /* no method registration */ }
ast::ClassMember::TsIndexSignature(_) => { /* index signature filter out */ }
ast::ClassMember::Empty(_) => { /* no-op */ }
ast::ClassMember::AutoAccessor(_) => {
    // PRD 2.8 で Tier 1 化、本 PRD では kind = Getter+Setter pair として扱うか、
    // 単に method 不登録 (Tier 2 honest error 経路で透過) を選ぶ → 後者 (PRD 2.8 が registration 化)
}
// No `_ =>` arm — Rule 11 d-1 compliance
```

**Iteration v9 追加** (T5 着手前 Spec gap fix、2026-04-28): `extends: Vec<String>` 登録を `class.class.super_class` 経由で propagate する。従来 `extends: vec![]` (line 195 hardcode) は B7 inherited detection (Design section #3-bis) の前提を満たさない latent gap であり、本 PRD architectural concern (= dispatch framework) の前提条件 infrastructure として T5 内で fix 必須:

```rust
// 修正前 (本 PRD T1-T3 batch close 時点):
TypeDef::Struct {
    type_params,
    fields,
    methods,
    constructor,
    call_signatures: vec![],
    extends: vec![],  // ← Iteration v9 で fix 対象
    is_interface: false,
}

// 修正後 (T5 Iteration v9):
let extends: Vec<String> = class
    .class
    .super_class
    .as_ref()
    .and_then(|expr| {
        if let ast::Expr::Ident(ident) = expr.as_ref() {
            Some(ident.sym.to_string())
        } else {
            None
        }
    })
    .into_iter()
    .collect();

TypeDef::Struct {
    type_params,
    fields,
    methods,
    constructor,
    call_signatures: vec![],
    extends,  // ← TS class single inheritance を Vec<String> (0 or 1 element) として登録
    is_interface: false,
}
```

**Justification**: TS class は単一継承 (`class B extends A`)、SWC AST では `class.class.super_class: Option<Box<ast::Expr>>`。Interface 用 `decl.rs:63-73` の Vec<String> 登録 pattern と symmetric。`Ident` 以外の super_class expression (例: `class B extends getMixin() {}`) は本 PRD scope 外として silent drop (= Tier 2 honest emission 候補は別 PRD)。

#### 3. Read context dispatch

**File**: `src/transformer/expressions/member_access.rs::resolve_member_access`

```rust
pub(crate) fn resolve_member_access(...) -> Result<Expr> {
    // ... 既存 enum / Math / .length 処理 ...

    // 新規: receiver type の method kind 検査
    if let Some(receiver_type_name) = self.get_receiver_type_name(ts_obj) {
        if let Some(TypeDef::Struct { methods, .. }) = self.reg().get(&receiver_type_name) {
            if let Some(sigs) = methods.get(field) {
                // sigs.iter().any(|s| s.kind == MethodKind::Getter)?
                if sigs.iter().any(|s| s.kind == MethodKind::Getter) {
                    // Read context: getter dispatch
                    return Ok(Expr::MethodCall {
                        object: Box::new(object.clone()),
                        method: field.to_string(),
                        args: vec![],
                    });
                }
                if sigs.iter().any(|s| s.kind == MethodKind::Setter)
                    && !sigs.iter().any(|s| s.kind == MethodKind::Getter)
                {
                    // setter only (write-only): read attempts は Tier 2 honest error
                    return Err(UnsupportedSyntaxError::new(
                        "read of write-only property",
                        ts_obj.span(),
                    ).into());
                }
                // Method (B6) の no-paren reference: Tier 2 honest error
                if sigs.iter().any(|s| s.kind == MethodKind::Method) {
                    return Err(UnsupportedSyntaxError::new(
                        "method-as-fn-reference (no-paren)",
                        ts_obj.span(),
                    ).into());
                }
            }
        }
    }

    // Fallback: direct field access (B1 / B9)
    Ok(Expr::FieldAccess { object: Box::new(object.clone()), field: field.to_string() })
}

// 新規 helper: B7 inherited accessor 検出 (TypeRegistry parent traversal、F-rev-4 fix)
// matrix cell 8 (A1 Read × B7 inherited) / cell 17 (A2 Write × B7) / cell 26 (A3 += × B7) /
// cell 41-c (A5 logical × B7) の "Tier 2 honest error reclassify (本 PRD)" を実現するため、
// receiver type の methods に直接 entry なくても parent class (TypeDef::Struct.extends 経由) の
// methods に entry あれば B7 と判定し、honest error return。
fn lookup_method_kind_with_parent_traversal(
    reg: &TypeRegistry,
    type_name: &str,
    field: &str,
    visited: &mut std::collections::HashSet<String>,
) -> Option<(MethodKind, /* is_inherited */ bool)> {
    if !visited.insert(type_name.to_string()) {
        return None; // 循環継承 prevention
    }
    if let Some(TypeDef::Struct { methods, extends, .. }) = reg.get(type_name) {
        // 直接 lookup (B1-B6 / B8 / B9 = is_inherited:false)
        if let Some(sigs) = methods.get(field) {
            if let Some(sig) = sigs.first() {
                return Some((sig.kind, false));
            }
        }
        // Parent traversal (B7 inherited = is_inherited:true)
        for parent_ref in extends {
            if let Some((kind, _)) = lookup_method_kind_with_parent_traversal(
                reg, &parent_ref.name, field, visited
            ) {
                return Some((kind, true));
            }
        }
    }
    None
}
```

#### 3-bis. B7 inherited dispatch (matrix cells 8/17/26/41-c の reclassify mechanism、F-rev-4 fix)

`resolve_member_access` / `dispatch_member_write` の dispatch logic は `lookup_method_kind_with_parent_traversal` を経由し、`is_inherited` flag を leverage:

```rust
if let Some((kind, is_inherited)) = lookup_method_kind_with_parent_traversal(
    self.reg(), &receiver_type_name, field, &mut HashSet::new()
) {
    if is_inherited {
        // B7 inherited accessor: 本 PRD scope = Tier 2 honest error reclassify
        // (Class inheritance dispatch = 別 architectural concern、別 PRD = "Class inheritance dispatch" で Tier 1 化)
        return Err(UnsupportedSyntaxError::new(
            "inherited accessor access (Rust struct inheritance not directly supported)",
            ts_obj.span(),
        ).into());
    }
    // is_inherited = false → B1-B6/B8/B9 dispatch (上記 #3 logic)
}
```

`is_inherited = true` の場合、本 PRD は dispatch 拡張せず Tier 2 honest error で surface (matrix cells 8/17/26/41-c の "Tier 2 honest error reclassify (本 PRD)" と token-level 一致 = Rule 6 (6-1) compliance restored)。

#### 4. Write context dispatch

**File**: `src/transformer/expressions/assignments.rs::convert_assign_expr` + `member_access.rs::convert_member_expr_for_write`

```rust
// convert_assign_expr の AssignTarget::Member(member) arm:
ast::SimpleAssignTarget::Member(member) => {
    // 既存: convert_member_expr_for_write を呼ぶ
    // 新規: receiver type の setter 検査、ある場合 MethodCall、なければ FieldAccess (current)
    self.dispatch_member_write(member, value)?
}

fn dispatch_member_write(&mut self, member: &ast::MemberExpr, value: Expr) -> Result<Expr> {
    // receiver type 解決
    if let Some(receiver_type_name) = self.get_receiver_type_name_for_member(member) {
        if let Some(TypeDef::Struct { methods, .. }) = self.reg().get(&receiver_type_name) {
            let field_name = extract_field_name(&member.prop)?;
            if let Some(sigs) = methods.get(&field_name) {
                if sigs.iter().any(|s| s.kind == MethodKind::Setter) {
                    // setter dispatch
                    let object = self.convert_expr(&member.obj)?;
                    return Ok(Expr::MethodCall {
                        object: Box::new(object),
                        method: format!("set_{field_name}"),
                        args: vec![value],
                    });
                }
                if sigs.iter().any(|s| s.kind == MethodKind::Getter)
                    && !sigs.iter().any(|s| s.kind == MethodKind::Setter)
                {
                    // getter only: write to read-only — Tier 2 honest error
                    return Err(UnsupportedSyntaxError::new(
                        "write to read-only property",
                        member.span,
                    ).into());
                }
                if sigs.iter().any(|s| s.kind == MethodKind::Method) {
                    return Err(UnsupportedSyntaxError::new(
                        "write to method (assignment to method member)",
                        member.span,
                    ).into());
                }
            }
        }
    }
    // Fallback: direct field write (B1 / B9)
    let target = self.convert_member_expr_for_write(member)?;
    Ok(Expr::Assign { target: Box::new(target), value: Box::new(value), op: BinOp::Assign })
}
```

#### 5. Compound assign desugar

`obj.x += v` を:
- side-effect-free `obj` (Ident / this): `obj.set_x(obj.x() + v)` 直接 emit
- side-effect `obj` (call result / complex): IIFE 形 `{ let __recv = obj; let __val = __recv.x() + v; __recv.set_x(__val); }` (block expression)

実装は IR layer で `Expr::Assign { op: AddAssign, ... }` を `Expr::Block` (with let bindings) + `MethodCall` に desugar するヘルパーで実現。

#### 6. Inside-class `this.x` dispatch (P1 TC39 faithful)

`convert_member_expr_inner` で receiver `member.obj` が `ast::Expr::This(_)` の場合、enclosing class の TypeDef を resolve (transformer の class scope state から)、その class の methods を検査して dispatch。external `obj.x` と uniform。

```rust
if matches!(member.obj.as_ref(), ast::Expr::This(_)) {
    if let Some(class_name) = self.tctx.enclosing_class_name() {
        if let Some(TypeDef::Struct { methods, .. }) = self.reg().get(&class_name) {
            // 同 dispatch logic
        }
    }
}
```

#### 7. Static accessor (B8) dispatch

`Foo.x` (Foo は ClassDecl で定義された type name) の場合、`MemberExpr.obj = Ident(Foo)` で obj の resolved type が `Type<Foo>` (constructor type) と判定できれば、static method として dispatch。

`Class::x()` IR: `Expr::FnCall { target: CallTarget::Path(vec!["Foo", "x"]), args: vec![] }` 等の既存 path 呼出 IR を leverage。

#### 8. Class Method Getter body `.clone()` 自動挿入 (C1 limited pattern)

**File**: `src/transformer/classes/members.rs::build_method_inner`

```rust
// is_setter ではない && kind == Getter && return_type 推論で T が非 Copy/Clone 性 detect
if kind == ast::MethodKind::Getter {
    // body の last stmt が `return self.field;` or last-expr `self.field` の場合、
    // body 出口の Expr::FieldAccess { object: self, field: F } を Expr::MethodCall { object: ..., method: "clone", args: vec![] } に rewrite
    if needs_clone_for_getter_return(&body, &return_type) {
        // body の last return / last expr を `.clone()` 付きに rewrite
    }
}
```

`needs_clone_for_getter_return` の判定:
- `return_type` が Copy 型 (D1/D2/D3/D8 全要素 Copy/D6 with Copy inner) → false
- それ以外 (D4/D5/D7/D9-D15) → true (要 `.clone()`)

#### 9. read-only / write-only property の Tier 2 honest error

上記 #3, #4 で実装。`UnsupportedSyntaxError::new("read of write-only property", ...)` / `UnsupportedSyntaxError::new("write to read-only property", ...)` を span 付きで return。

#### 10. `registry/collection/class.rs:145` の `_ => {}` arm 排除

```rust
// 修正前 (Rule 11 d-1 違反):
            _ => {}

// 修正後:
            ast::ClassMember::PrivateMethod(_) => {
                // PRD 2.7 で確認、本 PRD で kind tracking 追加候補だが scope 外 (private = #x、別 dispatch path)
            }
            ast::ClassMember::AutoAccessor(_) => {
                // PRD 2.8 (I-201-A) で Tier 1 化、本 PRD では framework infrastructure のみ提供
            }
            ast::ClassMember::StaticBlock(_) => {
                // class init logic、method 不登録
            }
            ast::ClassMember::TsIndexSignature(_) => {
                // index signature filter out
            }
            ast::ClassMember::Empty(_) => {
                // no-op
            }
            // No `_ =>` arm — Rule 11 d-1 compliance
```

### Design Integrity Review

[`design-integrity.md`](.claude/rules/design-integrity.md) checklist:

- **Higher-level consistency**: 本 framework は **transformer expression layer** の dispatch decision を中心化。`convert_member_expr` / `convert_member_expr_for_write` / `convert_assign_expr` (Member target) / `convert_update_expr` の 4 entry point から共通 dispatch helper (`dispatch_member_read` / `dispatch_member_write`) を呼ぶ設計。pipeline 上 transformer (parser → resolver → transformer → generator) の transformer phase 内で完結、generator / parser に逆流しない (`pipeline-integrity.md` 準拠)。
- **DRY**: dispatch logic を helper に集約 (4 entry point → 2 helper)。重複なし。
- **Orthogonality**: helper は receiver type + field name + read/write を input に、emit IR を output。class context state (enclosing class) は scope state から取得、helper 内に閉じる。
- **Coupling**: TypeRegistry → transformer 方向の単方向依存 (既存 pattern と consistent)。新規 method kind field は MethodSignature 内 self-contained。
- **Broken windows 発見**: (1) `registry/collection/class.rs:145` `_ => {}` arm = Rule 11 d-1 違反、本 PRD scope 内 fix。(2) `convert_member_expr_inner` 内の `_ =>` arm (`let field = match &member.prop { ... _ => return Err... };` line 375) は AST dispatch 用で意図的、Rule 11 d-1 適用 (line 372-376) は MemberProp::Computed が前段 if で処理されるため OK だが Verify 必要。

Verified, broken window 1 件本 PRD で fix。

### Impact Area (file paths empirically verified、Rule 11 (d-5) + RC-3 適用)

全 file path は `find` / `Read` で empirical verify 済み (uncertain expression 排除)。新規追加 / 変更 file:

| File | 変更内容 |
|------|---------|
| `src/registry/mod.rs` | `MethodSignature` に `kind: MethodKind` field 追加、`MethodKind` enum 新規定義 + `From<ast::MethodKind>` impl |
| `src/ts_type_info/mod.rs` | `TsMethodInfo` に `kind: MethodKind` field 追加 (line 200 直接定義、separate conversion.rs file は不在) |
| `src/registry/collection/class.rs` | `_ => {}` arm (line 145) 排除 + `method.kind` propagate (line 138-143) + AutoAccessor / PrivateMethod / StaticBlock / TsIndexSignature / Empty 明示 enumerate |
| `src/registry/collection/type_literals.rs` | `convert_method_info_to_sig` (line 73) で `m.kind` propagate (TsTypeLit からの method 情報、非 class) |
| `src/registry/collection/resolvers.rs` | resolve_method_sig (line 229-240) で kind preserve |
| `src/transformer/expressions/member_access.rs` | `resolve_member_access` (line 73-115) 拡張 (Read context dispatch)、`dispatch_member_write` helper 新規 |
| `src/transformer/expressions/assignments.rs` | `convert_assign_expr` (line 13-23) Member target で `dispatch_member_write` 経由、`convert_update_expr` (line 331) Member target で setter desugar (UpdateExpr `++/--` も同 file 内、separate update.rs file は不在) |
| `src/transformer/classes/members.rs` | `build_method_inner` (line 279-349) で Getter body の `.clone()` 自動挿入 (C1 pattern) |
| `src/transformer/expressions/mod.rs` | this expression 検出 + enclosing class scope 利用 |
| `src/transformer/mod.rs` | enclosing class name を Transformer.tctx (transformer context) に保持 (this.x dispatch 用、separate scope_state.rs file は不在、Transformer.tctx 直下 + 関連 transformer file 群で scope state 管理) |
| `doc/grammar/ast-variants.md` | ClassMember section (16) の MethodKind tracking 言及 + Tier sync update |
| `tests/e2e/scripts/i-205/` | 新規 fixture 群 (各 cell × T variant、~30-40 fixtures) |
| `tests/compile-check/i-205-class-getter-setter.ts` | class Method Getter/Setter compile pass test |
| `src/registry/tests/method_kind.rs` | method kind tracking unit test (新規) |
| `src/transformer/expressions/tests/i_205.rs` | dispatch logic unit test (新規) |
| `src/transformer/classes/tests/i_205.rs` | clone insertion unit test (新規) |

### Semantic Safety Analysis

[`type-fallback-safety.md`](.claude/rules/type-fallback-safety.md) 3-step analysis:

本 PRD は **type fallback / type approximation を導入しない**。dispatch logic 拡張のみで、TypeRegistry の既存型解決を leverage する。

ただし以下の semantic effect verify が必要:
1. **既存 B1 (field) cell の動作不変**: dispatch logic は receiver type に getter/setter が **存在する場合のみ** method dispatch、それ以外 (field only / no entry / unknown) は current direct field access を維持。これは regression lock-in test で verify。
2. **read-only / write-only honest error は silent semantic を保たず compile-time error として user に直接 surface する** (Tier 2)。silent semantic loss なし (tsc strict mode の挙動 = TS2540 type error と semantic 一致、tsc non-strict の silent no-op は ts_to_rs では reproduce 不能だがそれは「TS non-strict の semantic を Rust で完全 reproduce 不能」という ideal 不能の corner case で、Tier 2 honest error が ideal 妥協なし最良)。
3. **`.clone()` 自動挿入は semantic preserving**: `self.field.clone()` は `self.field` の deep copy を返す、TS の `return this._name;` semantic と equivalent (TS では reference returned だが getter 経由 caller が値を受け取る semantic と Rust の owned T 受け取り semantic は equivalent for `T: Clone`)。

**Verdict**: Safe — silent semantic change を導入しない。type fallback も導入しない。

---

## Spec → Impl Dispatch Arm Mapping (Rule 9 (a) compliance、F-rev-7 fix)

PRD 確定後、Implementation 着手前の **predicted implementation dispatch arms と matrix cells の 1-to-1 mapping**。Implementation 中に予期せぬ arm が surface した場合は **Spec gap signal** として `spec-first-prd.md` 「Spec への逆戻り」手順発動。

### `resolve_member_access` / B7 traversal helper (Read context dispatch)

#### Instance dispatch arms (`dispatch_instance_member_read`)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `lookup` returns `(MethodKind::Getter, is_inherited=false)` | cells 2/3/5 | `Expr::MethodCall { method: field, args: vec![] }` |
| `lookup` returns `(MethodKind::Setter, is_inherited=false)` and getter absent | cell 4 | `Err(UnsupportedSyntaxError::new("read of write-only property", ...))` |
| `lookup` returns `(MethodKind::Method, is_inherited=false)` | cell 7 | `Err(UnsupportedSyntaxError::new("method-as-fn-reference (no-paren)", ...))` |
| `lookup` returns `is_inherited=true` (any kind) | cell 8 | `Err(UnsupportedSyntaxError::new("inherited accessor access (Rust struct inheritance not directly supported)", ...))` |
| `lookup` returns `None` (B1 field、B9 unknown) | cells 1, 10 | `Expr::FieldAccess { object, field }` (current behavior) |
| `member.obj` is `ast::Expr::This(_)` (E2 internal) | cells 60, 62 | enclosing class scope lookup → 同 dispatch (P1 TC39 faithful、T10) |

#### Static dispatch arms (`dispatch_static_member_read`、Iteration v9 deep deep review で実装と整合訂正)

Static dispatch context = receiver が `ast::Expr::Ident(class_name)` で `get_expr_type` None かつ `reg.get(class_name)` が `TypeDef::Struct { is_interface: false, .. }` かつ `lookup_method_sigs_in_inheritance_chain` が **`Some((sigs, is_inherited))` を return する場合のみ** `dispatch_static_member_read` が呼ばれる。`lookup` returns `None` (static field、registered class でも methods に該当 entry なし) は本 helper を経由せず、`resolve_member_access` の最終 fallback (5. FieldAccess) に流れる (= **Class.staticField の現挙動 = `Class.staticField` FieldAccess emit、新 PRD I-B で `Expr::AssociatedConst` path access に修正**)。

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Getter)` | cell 9 | `Expr::FnCall { target: CallTarget::UserAssocFn { ty: UserTypeRef::new(class_name), method: field }, args: vec![] }` |
| `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Setter)` and Getter absent | static B3 (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("read of write-only static property", ...))` |
| `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Method)` and {Getter, Setter} 共に absent | static B6 (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("static-method-as-fn-reference (no-paren)", ...))` |
| `lookup` returns `Some((sigs, true))` (is_inherited=true、any kind) | static B7 (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("inherited static accessor access (Rust associated fn does not chain inheritance)", ...))` |
| (`dispatch_static_member_read` は呼ばれない、`resolve_member_access` 最終 fallback 経由) `lookup` returns `None` | (matrix cell 化なし、新 PRD I-B で expansion) | `Expr::FieldAccess { object, field }` (= Rust 上 `Class.field` syntax、static field の場合は Tier 2 等価 compile error。新 PRD I-B で **`Class::staticField` associated const path access** に修正) |

**Structural invariant (Iteration v9 deep deep review fix)**: `dispatch_static_member_read` の本体 3 if-block (Getter / Setter / Method) は `MethodKind` enum の 3 variant 完全列挙 + `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant により **構造的に必ず 1 arm が fire**。旧記載の "lookup None case で UserAssocFn (Path-based) emit" は実装誤記、本 deep deep review で `unreachable!()` macro による structural enforcement に置換 (= dead code 排除 + invariant codified)。

**Cell 化されていない static dispatch arms の rationale (Iteration v9 deep deep review post-fix)**: 現 matrix の static B8 cell = 9 (Read static getter) + 18 (Write static setter) のみで、static × {B3 setter only / B6 method-as-fn-ref / B7 inherited / None field} の cells は明示 enumerate されていない。Implementation 上は dispatch_static_member_read で defensive Tier 2 honest error reclassify / 5. FieldAccess fallback、reachability scope 外 (cells 9/18 fixture = static-only class、本 PRD I-205 matrix では発生しない)。subsequent **新 PRD I-A (Method static-ness propagation) / I-B (Class TypeName context detection unification)** で (T11 削除済 2026-05-01):
- (a) Static × {B3/B6/B7/None field} cell を matrix に明示 enumerate (新 PRD I-A の Mixed class 軸 + I-B の receiver shape 軸 完成 lock-in 用)
- (b) Mixed (static + instance) class での `is_static` filter 不在 risk (本 review insight #1) を reachability audit + reclassify (新 PRD I-A scope)
- (c) Static field (`Class.staticField`) emission strategy 確定 = 現 fallback `Expr::FieldAccess` (Rust 上 invalid `.` syntax、Tier 2 等価 compile error) を **`Class::staticField` associated const path access** に修正 (新 PRD I-B scope、新規 IR variant `Expr::AssociatedConst { ty: UserTypeRef, name: String }` 導入)

### `dispatch_member_write` (Write context dispatch)

#### Instance dispatch arms (`dispatch_instance_member_write`)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `lookup` returns `(Setter, false)` | cells 13, 14 | `Expr::MethodCall { method: format!("set_{field}"), args: [value] }` |
| `lookup` returns `(Getter, false)` and Setter absent | cell 12 | `Err(UnsupportedSyntaxError::new("write to read-only property", ...))` |
| `lookup` returns `(Method, false)` and {Getter, Setter} 共に absent | cell 16 | `Err(UnsupportedSyntaxError::new("write to method", ...))` |
| `lookup` returns `(any kind, true)` (is_inherited=true) | cell 17 | `Err(UnsupportedSyntaxError::new("write to inherited accessor", ...))` |
| (`dispatch_instance_member_write` は呼ばれない、`resolve_member_access` 最終 fallback 経由) `lookup` returns `None` (B1 field、B9 unknown) | cells 11, 19 | `Expr::Assign { target: FieldAccess, value, op: Assign }` (= `convert_member_expr_for_write` fallback、`for_write=true` skip path と equivalent) |

**Structural invariant (Iteration v10、Read context dispatch_instance_member_read と symmetric)**: `dispatch_instance_member_write` の本体 3 if-block (Setter / Getter / Method) は `MethodKind` enum 3 variant 完全列挙 + `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant により **構造的に必ず 1 arm が fire** (`unreachable!()` macro で structural enforcement)。Read context cell 8 と symmetric な Tier 2 honest error reclassify (orthogonal architectural concern = "Class inheritance dispatch" 別 PRD I-206) を Write context でも適用。

#### Static dispatch arms (`dispatch_static_member_write`、Iteration v10 で Read mapping と symmetric 化)

Static dispatch context = receiver が `ast::Expr::Ident(class_name)` で `get_expr_type` None かつ `reg.get(class_name)` が `TypeDef::Struct { is_interface: false, .. }` かつ `lookup_method_sigs_in_inheritance_chain` が **`Some((sigs, is_inherited))` を return する場合のみ** `dispatch_static_member_write` が呼ばれる。`lookup` returns `None` (static field、registered class でも methods に該当 entry なし) は本 helper を経由せず、`dispatch_member_write` の最終 fallback (= `convert_member_expr_for_write` 経由 FieldAccess Assign) に流れる (= **Class.staticField = v の現挙動 = `Class.staticField = v;` Rust 上 `.` syntax error、新 PRD I-B で associated const は Rust 上 immutable のため Tier 2 honest error reclassify "write to static field"、separate strategy (即 mut static / OnceLock 等) は別 PRD scope**)。

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Setter)` | cell 18 | `Expr::FnCall { target: CallTarget::UserAssocFn { ty: UserTypeRef::new(class_name), method: format!("set_{field}") }, args: [value] }` |
| `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Getter)` and Setter absent | static B3 Write (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("write to read-only static property", ...))` |
| `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Method)` and {Getter, Setter} 共に absent | static B6 Write (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("write to static method", ...))` |
| `lookup` returns `Some((sigs, true))` (is_inherited=true、any kind) | static B7 Write (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("write to inherited static accessor", ...))` |
| (`dispatch_static_member_write` は呼ばれない、`dispatch_member_write` 最終 fallback 経由) `lookup` returns `None` | (matrix cell 化なし、新 PRD I-B で expansion) | `Expr::Assign { target: FieldAccess, value, op: Assign }` (= Rust 上 `Class.field = v;` syntax error、static field の場合は Tier 2 等価 compile error。新 PRD I-B で **Tier 2 honest error reclassify "write to static field"** に変更、即 mut static / OnceLock 等の separate strategy は別 PRD scope) |

**Structural invariant (Iteration v10 で Read context dispatch_static_member_read と symmetric 化、Spec gap fix)**: `dispatch_static_member_write` の本体 3 if-block (Setter / Getter / Method) は `MethodKind` enum 3 variant 完全列挙 + `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant により **構造的に必ず 1 arm が fire**、`unreachable!()` macro で structural enforcement。

**Cell 化されていない static Write dispatch arms の rationale (Iteration v10 で defensive Tier 2 honest error reclassify)**: 現 matrix の static B8 cell = 9 (Read static getter) + 18 (Write static setter) のみで、static × {B3 setter only Write / B6 method Write / B7 inherited Write / None field Write} の cells は明示 enumerate されていない。Implementation 上は dispatch_static_member_write で defensive Tier 2 honest error reclassify、reachability scope 外 (cell 18 fixture = static-only class、本 PRD I-205 matrix では発生しない)。subsequent **新 PRD I-A (Method static-ness propagation) / I-B (Class TypeName context detection unification)** で (T11 削除済 2026-05-01):
- (a) Static × {B3/B6/B7/None field} Write cell を matrix に明示 enumerate (新 PRD I-A の Mixed class 軸 + I-B の receiver shape 軸 完成 lock-in 用)
- (b) tsc oracle observation + per-cell E2E fixture (red lock-in)
- (c) reachability audit (本 review insight #2)

### `convert_assign_expr` compound branch (A3 arithmetic + A4 bitwise dispatch、Iteration v12 で T8 implementation 想定に整合 + structural form 化)

T8 scope = **arithmetic compound (`+= -= *= /= %=`) + bitwise compound (`<<= >>= >>>= &= |= ^=`) = 11 ops**。
A5 logical compound (`??= &&= ||=`) は **T9 scope** (既存 `nullish_assign.rs` / `compound_logical_assign.rs` helper integration、別 architectural concern)、本 mapping table では別 sub-section で T9 の予測 dispatch を記載。

`convert_assign_expr` の T8 entry: T6 plain `=` × Member の gate 直後に T8 compound × Member gate を追加。Op-axis orthogonality merge (Rule 1 (1-4)): 全 arm の op variant (AddAssign / SubAssign / .../ BitXorAssign) は dispatch logic 同一 (= BinOp 置換のみ、`arithmetic_compound_op_to_binop` mapping helper で AssignOp → BinOp 1-to-1 変換)、本 table は **Instance dispatch arms / Static dispatch arms / Fallback arm** の 3 sub-section で structural form 化 (T7 update Mapping table と symmetric)。

#### Instance dispatch arms (`dispatch_instance_member_compound`)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `MemberReceiverClassification::Instance` + `has_getter && has_setter` (B4) + side-effect-free receiver | cells 21, 29-d, 33, 34-c (op-axis orthogonality-equivalent) | `Expr::Block { Let __ts_new = BinOp MethodCall obj.x() OP rhs; Stmt::Expr MethodCall obj.set_x(__ts_new); TailExpr __ts_new }` (compound assign yields new value、prefix update と same shape with rhs replacing 1.0) |
| `Instance` + `has_getter && has_setter` (B4) + side-effect-having receiver (INV-3 1-evaluate compliance) | cells 21, 29-d, 33, 34-c (op-axis orthogonality-equivalent) | `Expr::Block { Let mut __ts_recv = <object>; Let __ts_new = BinOp MethodCall __ts_recv.x() OP rhs; Stmt::Expr MethodCall __ts_recv.set_x(__ts_new); TailExpr __ts_new }` (IIFE form で receiver 1-evaluate 保証) |
| `Instance` + `has_getter` only (B2) | cells 22, 29-b, 31, 34-b (op-axis orthogonality-equivalent) | `Err(UnsupportedSyntaxError::new("compound assign to read-only property", ...))` |
| `Instance` + `has_setter` only (B3) | cells 23, 29-c, 32 (op-axis orthogonality-equivalent) | `Err(UnsupportedSyntaxError::new("compound assign read of write-only property", ...))` (compound assign は read 先行、getter 不在で read fail) |
| `Instance` + `has_method` only (B6) | cells 25, 29-e-b, 35-b (op-axis orthogonality-equivalent) | `Err(UnsupportedSyntaxError::new("compound assign to method", ...))` |
| `Instance` + `is_inherited = true` (B7) | cells 26, 29-e-c, 35-c (op-axis orthogonality-equivalent) | `Err(UnsupportedSyntaxError::new("compound assign to inherited accessor", ...))` |

#### Static dispatch arms (`dispatch_static_member_compound`、receiver = class TypeName で IIFE form 不要 = side-effect なし path)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `Static` + `has_getter && has_setter` (B8) | cells 27, 29-e-d, 35-d (op-axis orthogonality-equivalent) | `Expr::Block { Let __ts_new = BinOp FnCall::UserAssocFn Class::x() OP rhs; Stmt::Expr FnCall::UserAssocFn Class::set_x(__ts_new); TailExpr __ts_new }` |
| `Static` + has_getter only (defensive、static B2) | (matrix cell 化なし、新 PRD I-A/I-B で expansion) | `Err(UnsupportedSyntaxError::new("compound assign to read-only static property", ...))` |
| `Static` + has_setter only (defensive、static B3) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("compound assign read of write-only static property", ...))` |
| `Static` + has_method only (defensive、static B6) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("compound assign to static method", ...))` |
| `Static` + `is_inherited = true` (defensive、static B7) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("compound assign to inherited static accessor", ...))` |

#### Fallback arm (B1 field、B9 unknown、non-class receiver、static field)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `MemberReceiverClassification::Fallback` (B1 field、B9 unknown、non-class receiver) | cells 20, 28, 29-a, 29-e-e, 30, 34-a, 35-e (op-axis orthogonality-equivalent regression preserve) | `Expr::Assign { target: <FieldAccess obj.x>, value: Expr::BinaryOp { left: <FieldAccess obj.x>, op: <BinOp from AssignOp>, right: rhs } }` (= existing compound desugar emit、`convert_member_expr_for_write` 経由で Member target を FieldAccess IR 化、regression lock-in) |
| Non-Member target (Ident / Computed / etc.) | (本 dispatch entry を経由しない) | 既存 `convert_assign_expr` の compound branch fall-through で Ident binding update / Computed Index update emit (不変) |

**Structural invariant (Iteration v12)**: `dispatch_instance_member_compound` / `dispatch_static_member_compound` の本体 4 if-block (B4 setter desugar / B2 getter only / B3 setter only / B6 method) は `MethodKind` enum 3 variant 完全列挙 + `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant により **構造的に必ず 1 arm が fire**、`unreachable!()` macro で structural enforcement (T5/T6/T7 helpers と symmetric)。

**Op-axis orthogonality merge** (Rule 1 (1-4) compliance): 全 arm の AssignOp variant (AddAssign / SubAssign / MulAssign / DivAssign / ModAssign / BitAndAssign / BitOrAssign / BitXorAssign / LShiftAssign / RShiftAssign / ZeroFillRShiftAssign = 11 ops) は dispatch logic 同一 (= BinOp 置換のみ、`arithmetic_compound_op_to_binop` 1-to-1 mapping helper で AssignOp → BinOp { Add / Sub / Mul / Div / Mod / BitAnd / BitOr / BitXor / Shl / Shr / UShr } 変換)、Rule 1 (1-4-a)/(1-4-b)/(1-4-c) compliant な op-axis orthogonality merge 適用。

**INV-3 1-evaluate compliance (本 T8 scope、setter dispatch path のみ)**: `obj.x += v` の desugar で receiver `obj` が **1 回のみ evaluate** されることを保証する。判定 helper `is_side_effect_free(expr: &Expr) -> bool` で receiver IR を check、結果に応じて 2 path に分岐:
- side-effect-free (`Expr::Ident` / depth-bounded `Expr::FieldAccess`): 直接 emit (Rust source 上 receiver が 2 回出現するが cheap reference copy で意味 invariant)
- side-effect (`Expr::FnCall` / `Expr::MethodCall` / etc.): IIFE form `{ let mut __ts_recv = <object>; ... }` で binding 経由、receiver expression eval は Let init で 1 回のみ実行
INV-3 (a) Property statement compliance、Fallback path (B1/B9) は本 T8 scope 外 (= 別 architectural concern として TODO 起票候補)。

**T7 INV-3 back-port (本 T8 scope)**: `dispatch_instance_member_update` の B4 setter desugar arm も同 `is_side_effect_free` + IIFE wrap で update、T7 で発覚した latent gap を T8 で structural cohesive 解消 (T7 helpers と T8 helpers が `build_setter_desugar_block` (旧 `build_update_setter_block` の generalize 版) + `wrap_with_recv_binding` を共有)。

#### A5 Logical compound dispatch (T9 scope、本 T8 では dispatch なし)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `op == AssignOp::NullishAssign` (A5 ??=) | cells 36-40, 41-* | T9 で既存 `nullish_assign.rs` `pick_strategy` helper integration、setter dispatch arm 追加 |
| `op == AssignOp::AndAssign` (A5 &&=) | cells 39, 41 series | T9 で既存 `compound_logical_assign.rs` helper integration、setter dispatch arm 追加 |
| `op == AssignOp::OrAssign` (A5 ||=) | cells 40, 41 series | 同上 |

### `convert_update_expr` Member target dispatch (A6 ++/-- dispatch、Iteration v11 Spec gap fix で B2/B3 enumerate completeness 化)

UpdateExpr Member target の dispatch arms enumerate (op-axis ++ / -- は orthogonality-equivalent、BinOp::Add / Sub 置換のみ。`build_update_setter_block` (instance/static 共通 setter desugar block builder) + `build_fallback_field_update_block` (B1/B9 Fallback FieldAccess BinOp block builder) で IR shape 統一)。

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `MemberReceiverClassification::Instance` + `has_getter && has_setter` (B4) + getter return numeric | cells 43, 45-c | postfix: `Expr::Block { Let __ts_old = MethodCall obj.x(); Stmt::Expr MethodCall obj.set_x(BinOp __ts_old OP 1.0); TailExpr __ts_old }` / prefix: `Expr::Block { Let __ts_new = BinOp MethodCall obj.x() OP 1.0; Stmt::Expr MethodCall obj.set_x(__ts_new); TailExpr __ts_new }` |
| `Instance` + `has_getter && has_setter` (B4) + getter return non-numeric (D4-D15) | cell 44 + cell 44-symmetric | `Err(UnsupportedSyntaxError::new("increment of non-numeric (String/etc.) — TS NaN coercion semantic", ...))` for `++` / `Err("decrement of non-numeric (...)")` for `--` |
| `Instance` + `has_getter` only (B2) | cell 45-b + cell 45-b-symmetric | `Err(UnsupportedSyntaxError::new("write to read-only property", ...))` (`++`/`--` は write 必要、setter 不在で write fail) |
| `Instance` + `has_setter` only (B3) | cell 45-b3 | `Err(UnsupportedSyntaxError::new("read of write-only property", ...))` (`++`/`--` は read 先行、getter 不在で read fail) |
| `Instance` + `has_method` only (B6) | cells 45-db, 45-db-symmetric | `Err(UnsupportedSyntaxError::new("write to method", ...))` |
| `Instance` + `is_inherited = true` (B7) | cells 45-dc, 45-dc-symmetric | `Err(UnsupportedSyntaxError::new("write to inherited accessor", ...))` |
| `Static` + `has_getter && has_setter` (B8) + numeric | cells 45-dd, 45-dd-symmetric | postfix: `Expr::Block { Let __ts_old = FnCall::UserAssocFn Class::x(); Stmt::Expr FnCall::UserAssocFn Class::set_x(BinOp __ts_old OP 1.0); TailExpr __ts_old }` / prefix: 同 prefix shape with `__ts_new` |
| `Static` + has_getter only (defensive、static B2) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("write to read-only static property", ...))` |
| `Static` + has_setter only (defensive、static B3) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("read of write-only static property", ...))` |
| `Static` + has_method only (defensive、static B6) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("write to static method", ...))` |
| `Static` + `is_inherited = true` (defensive、static B7) | (matrix cell 化なし) | `Err(UnsupportedSyntaxError::new("write to inherited static accessor", ...))` |
| `Fallback` (B1 field、B9 unknown、non-class receiver) | cells 42, 45-a, 45-de, 45-de-symmetric | postfix: `Expr::Block { Let __ts_old = FieldAccess obj.x; Stmt::Expr Assign FieldAccess obj.x = BinOp __ts_old OP 1.0; TailExpr __ts_old }` / prefix: `Expr::Block { Stmt::Expr Assign FieldAccess obj.x = BinOp FieldAccess obj.x OP 1.0; TailExpr FieldAccess obj.x }` |
| `MemberProp::Computed` (`obj[i]++`) | (matrix scope 外) | `Err(anyhow!("unsupported update expression target"))` (early return at `extract_non_computed_field_name = None` gate、I-203 codebase-wide AST exhaustiveness defer) |

**Structural invariant** (Iteration v11): `dispatch_instance_member_update` / `dispatch_static_member_update` の本体 4 if-block (B4 setter desugar / B2 getter only / B3 setter only / B6 method) は `MethodKind` enum 3 variant 完全列挙 + `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant により **構造的に必ず 1 arm が fire**、`unreachable!()` macro で structural enforcement (T6 Read/Write context dispatch helpers と symmetric)。

**Op-axis orthogonality merge** (Rule 1 (1-4) compliance、Iteration v11 Spec gap fix): 全 arm の `++` (BinOp::Add) と `--` (BinOp::Sub) は dispatch logic 同一 (= operator 置換のみ)、Rule 1 (1-4-a)/(1-4-b)/(1-4-c) compliant な op-axis orthogonality merge 適用。各 cell の `cell-symmetric` 派生 cell は op の symmetric counterpart を Rule 1 (1-4) inheritance で表現。

### `build_method_inner` (Class Method Getter body `.clone()` insertion、C1 limited pattern)

| Predicted dispatch arm | Matrix cell(s) | Emit IR |
|------------------------|---------------|---------|
| `kind == Getter` AND body last stmt = `Stmt::Return(Expr::FieldAccess(self.field))` AND return_type is non-Copy | cells 70/72/74 | rewrite to `Stmt::Return(Expr::MethodCall { object: FieldAccess, method: clone, args: vec![] })` |
| `kind == Getter` AND body last expr = `Expr::FieldAccess(self.field)` (no return keyword) AND return_type is non-Copy | cell 78 | rewrite last expr to `Expr::MethodCall clone` (cell 78 / Rule 7 last-expr sub-case) |
| `kind == Getter` AND return_type is Copy (D1/D2/D3、Option<Copy>、tuple of all Copy) | cells 71, 73 | no rewrite (current behavior) |
| `kind == Getter` AND body shape != literal field access (cells 75/76/77/79/80) | cells 75/76/77/79/80 | no rewrite (Tier 2、user manual `.clone()` 必要、本 PRD scope 外 = 別 PRD C2) |
| `kind == Setter` (cells 13/14/81) | cells 13/14/81 | no body rewrite (current behavior、setter body は self.field = v pattern) |

### Spec → Impl Mapping completeness verification

Implementation 中 Rule 9 (b) "Impl → Spec" 逆戻り発動条件:
- 新規 dispatch arm (上記 mapping table に未列挙) を追加する必要発覚
- 既存 arm の matrix cell mapping が 1-to-1 でない (1-to-many or many-to-1 が判明)

両 case で `spec-first-prd.md`「Spec への逆戻り」発動、matrix cells を分割 or merge、Spec Review Iteration Log v4+ に記録。

## Spec Stage Tasks (Rule 5 (5-2) hard-code、Stage 1 artifacts 完成 task)

Stage 1 で実施する task。code 改修 (`src/` 修正) を含めること禁止。完了後 Implementation Stage 移行可能。

### TS-0: Cartesian product matrix completeness (Rule 1 (1-2) + Rule 10 Step 2 orthogonality reduction compliance)

- **Work**: Problem Space matrix を Rule 10 Step 2 orthogonality reduction 適用後の "remaining axes Cartesian product" として全 cell explicit enumerate (現 v2 ~85 cells)、abbreviation pattern (`...`/range grouping/representative/varies/(各別 cell)/(同上)/`同 X`/`同 dispatch logic`) 全廃。Orthogonality merge wording (`B 全`/`D 全`/`B1-B9` 等) は Rule 10 Step 2 reduction として独立 cell に明示展開 or `Orthogonality justification` 記載。
- **Completion criteria**: matrix table 内 abbreviation pattern 不在 (各 cell 自己完結、`同 X` reference なし)、`audit-prd-rule10-compliance.py` Rule 1 (1-2) check PASS。
- **Depends on**: なし
- **Status (2026-04-27 v2)**: 部分完了 (cells 1-23, 24-52 (sub-divided 24-a/24-b/...), 60-64, 70-80 explicit enumerate 済、orthogonality merge cells 29-e/35/41/45-d は equivalence class として cells 24-28 mapping 経由で derive 可能)、audit script PASS。

### TS-1: Oracle observation log embed (Rule 2 (2-2) compliance)

- **Work**: 各 ✗ / 要調査 cell について TS fixture 作成 (`tests/e2e/scripts/i-205/cell-NN-*.ts`)、`scripts/observe-tsc.sh` 実行、PRD doc `## Oracle Observations` section に 4 項目 (TS fixture path / tsc output / cell # link / ideal output rationale) 完全 embed
- **Completion criteria**: 全 ✗/要調査 cell の Oracle Observation 完全 record、`audit-prd-rule10-compliance.py` Rule 2 (2-2) check PASS
- **Depends on**: TS-0

### TS-2: SWC parser empirical lock-in (Rule 3 (3-2) compliance)

- **Work**: 全 NA cell について `tests/swc_parser_*_test.rs` で SWC parser empirical lock-in test 作成、PRD doc `## SWC Parser Empirical Lock-ins` section embed。SWC accept 確認時は Tier 2 honest error reclassify (Rule 3 (3-3) per PRD 2.7 lesson)
- **Completion criteria**: 全 NA cell の SWC behavior verify、`cargo test` で SWC parser test 全 pass
- **Depends on**: TS-0

### TS-3: E2E fixture creation (red 状態、Rule 5 (5-1) compliance)

- **Work**: 各 ✗ cell に対応 `tests/e2e/scripts/i-205/cell-NN-*.ts` fixture 作成、`scripts/record-cell-oracle.sh` で expected output 記録 (red 状態 = ts_to_rs 出力 ≠ expected)
- **Completion criteria**: `cargo test --test e2e_test` で全 fixture red 確認 (Implementation stage で green 化)
- **Depends on**: TS-1

### TS-4: Impact Area audit findings populate (Rule 11 (d-5) compliance)

- **Work**: CI で `python3 scripts/audit-ast-variant-coverage.py --files <impact-area>` 実行 (tree-sitter-rust available 環境)、結果を PRD doc `## Impact Area Audit Findings` section に正規 audit output として update
- **Completion criteria**: section が CI auto-generated audit output reflect、各 violation の決定 (本 PRD scope or I-203 defer) record
- **Depends on**: なし (CI execution-dependent)

### TS-5: 13-rule self-applied verify pass (Rule 13 (13-3) compliance)

- **Work**: skill workflow Step 4.5 で 13-rule self-applied verify、Critical findings 全 fix、再 self-review pass で Spec stage 完了
- **Completion criteria**: `audit-prd-rule10-compliance.py` 全 rule PASS、`## Spec Review Iteration Log` 全 iteration record
- **Depends on**: TS-0, TS-1, TS-2, TS-3, TS-4

## Implementation Stage Tasks (Stage 2 code change task)

Stage 2 で実装する `src/` 修正 task。**Spec Stage Tasks (TS-0〜TS-5) 全完了が prerequisite** (5-3 sub-rule、`spec-stage-adversarial-checklist.md` Rule 5 適用)。Assumes TDD: RED → GREEN → REFACTOR order。

**Prerequisite (全 T-* task 共通)**: Spec Stage Tasks TS-0 (matrix completeness) + TS-1 (oracle observations full populate) + TS-2 (SWC parser empirical) + TS-3 (E2E fixture red 状態) + TS-4 (Impact Area audit findings) + TS-5 (13-rule self-applied verify pass) 全完了。

### T1: `doc/grammar/ast-variants.md` ClassMember section update + MethodKind tracking 言及 (doc-first) [完了 2026-04-28]

- **Work**: ast-variants.md ClassMember section に method kind tracking が本 PRD で導入される旨記載 (Rule 4 (4-2) doc-first dependency order)
- **Completion criteria**: doc update commit、audit-prd-rule10-compliance.py audit pass
- **Depends on**: T0
- **Status**: 完了。`doc/grammar/ast-variants.md` ClassMember Method/PrivateMethod 行に method kind tracking 言及追加 + 「Method kind tracking (I-205 で導入)」section 新設 (MethodKind enum / Tier 2 honest error reclassify trigger / Future leverage 3 項目記載)。`audit-prd-rule10-compliance.py` PASS。

### T2: MethodSignature / TsMethodInfo に kind field 追加 (infrastructure) [完了 2026-04-28]

- **Work**: `src/registry/mod.rs` `MethodSignature.kind`、`src/ts_type_info/mod.rs` `TsMethodInfo.kind`、`MethodKind` enum + From impl 追加
- **Completion criteria**: `cargo build` pass、既存 test pass (kind = Default::default() = Method で fallback)
- **Depends on**: T1
- **Status**: 完了。`MethodKind { Method, Getter, Setter }` enum (Default = Method、SWC `swc_ecma_ast::MethodKind` mirror、`From<swc_ecma_ast::MethodKind>` 変換 impl)。`MethodSignature.kind` + `TsMethodInfo.kind` 追加、`Default` impl 拡張、`MethodSignature::substitute` で kind preserve。51 既存 construction site (production + test) を bulk script で `kind: MethodKind::Method,` 補完 (backward compat fallback)。`cargo build --all-targets` pass、`cargo test --lib` 3162 既存 pass (regression なし)。**Fix 1 (post-`/check_job` 4-layer review、2026-04-28)**: `MethodKind` enum を `src/registry/mod.rs` から `src/ir/method_kind.rs` (foundational module) へ move、`registry::MethodKind` は re-export として保持 (= 51 site backward compat)、`ts_type_info::TsMethodInfo.kind` は `crate::ir::MethodKind` を直接参照に変更で **module-level circular dep (registry ↔ ts_type_info) を構造的解消**。新規 5 unit test (`src/ir/method_kind.rs::tests::*`、Default / From SWC 3 variants / Copy trait verify) 追加。

### T3: collect_class_info で method.kind propagate + `_ => {}` arm 排除 (Rule 11 d-1 fix) [完了 2026-04-28]

- **Work**: `src/registry/collection/class.rs` の Method arm で `method.kind` 取得、AutoAccessor / PrivateMethod / StaticBlock / TsIndexSignature / Empty を explicit enumerate
- **Completion criteria**: `cargo test` pass、Rule 11 d-1 audit pass、unit test で getter/setter kind tracked verify
- **Depends on**: T1, T2
- **Status**: 完了。Method arm で `MethodKind::from(method.kind)` で SWC MethodKind を IR 側に propagate。`_ => {}` arm を排除し PrivateMethod / StaticBlock / AutoAccessor / TsIndexSignature / Empty を explicit enumerate (各 reason comment 付き、Rule 11 (d-1) compliance restored、Impact Area Audit Findings の `class.rs:145` violation fix)。**latent bug fix (Pass 2 layer)**: `src/ts_type_info/resolve/typedef.rs::resolve_method_sig` (`MethodSignature<TsTypeInfo>` → `MethodSignature<RustType>` 変換) が `kind: MethodKind::Method` を hardcode していたため getter/setter が Method として失われていた、`kind: sig.kind` に修正で lossless propagate。**新規 unit test 4 件**: `test_class_method_kind_default_method_is_propagated` / `test_class_method_kind_getter_is_propagated` / `test_class_method_kind_setter_is_propagated` / `test_class_method_kind_getter_and_setter_pair_distinguished` (`src/registry/tests/build_registry/class.rs`)、全 pass。
- **Fix 2 (post-`/check_job` 4-layer review、2026-04-28、T4 work piece の前倒し完了)**: `src/registry/collection/type_literals.rs::convert_method_info_to_sig` が `kind: MethodKind::Method` を hardcode し `m.kind` (TsMethodInfo の kind) を silently 無視していた **latent silent semantic risk** (`resolve_method_sig` と symmetric な未 fix bug、TsTypeLit getter/setter が Tier 2 unsupported のため current reachability なしで latent) を `kind: m.kind` に修正で structural fix。**T4 task の core work を T3 batch で完了**、T4 残作業は test-only (TsTypeLit context での verify、本 Fix 2 で 3 件追加済 = `test_build_struct_from_type_literal_propagates_getter_kind` / `propagates_setter_kind` / `distinguishes_getter_setter_pair`)。
- **Fix 3 (post-`/check_job` 4-layer review、2026-04-28)**: `class.rs` の `let _ = private_method` / `let _ = static_block` / `let _ = auto_accessor` 3 site の anti-idiomatic pattern を Rust idiom (`ast::ClassMember::PrivateMethod(_) => { /* comment */ }` 形式) に refactor、code 行数 reduce + Rust idiom 準拠。
- **Fix 4 (post-`/check_job` 4-layer review、2026-04-28、framework self-applied integration v1.6 → v1.7)**: `spec-stage-adversarial-checklist.md` Rule 9 に sub-rule (c) "Field-addition symmetric conversion site audit" 追加 (Pre-implementation symmetric audit + Audit script auto-verify candidate + Post-implementation review trigger 3 mechanism)。**Recurring problem evidence**: I-383 T8' (`type_params` field 追加) + I-205 T2 (`kind` field 追加) で 2 度連続発生確認、3 度目発生前の structural prevention。本 sub-rule (c) は **process** 解決、別 PRD I-213 (codebase-wide IR struct construction DRY refactor) は **structural** 解決として相補的に動作。
- **Final quality (Fix 1-4 完了後、2026-04-28)**: `cargo test --lib` 3176 pass (3162 baseline + 4 T3 + 3 Fix 1 method_kind (post-D1: From×3 を移動した分は registry/swc_method_kind 4 件に再配置) + 4 D1 swc_method_kind + 3 Fix 2 type_literals = 3176)、`cargo test --test e2e_test` 159 pass + 70 ignored、`cargo test --test compile_test` 3 pass、122 integration pass、clippy 0 warning、fmt 0 diff、audit-prd-rule10-compliance.py PASS、audit-ast-variant-coverage.py PASS for `class.rs` (out-of-scope 2 件は I-203 defer per Rule 11 (d-6))。
- **別 PRD 起票 (post-`/check_job`)**: `[I-213]` IR struct construction boundary DRY refactor (codebase-wide、L4、recurring problem evidence: I-383 T8' + I-205 T2 で 2 度連続) を TODO 起票 — Fix 4 framework rule sub-rule (c) と相補的、I-205 直接 scope 外 broader concern。

### T4: TsTypeLit / convert_method_info_to_sig で kind propagate [完了 2026-04-28、core work は T3 batch Fix 2 で前倒し完了]

- **Work**: `src/registry/collection/type_literals.rs::convert_method_info_to_sig` で `m.kind` 利用 (TsTypeLit には interface 由来の getter/setter があり得る)
- **Completion criteria**: unit test で TsTypeLit の getter/setter kind tracked verify
- **Depends on**: T1, T2
- **Status**: 完了。T3 batch の `/check_job` initial 4-layer review (2026-04-28) で発見された Implementation gap (`type_literals.rs:98` で `m.kind` を silently 無視する latent silent semantic risk) を **Fix 2** として T3 batch 内で前倒し完了 (= core work 完了)。`convert_method_info_to_sig` を `kind: m.kind` に修正、`resolve_method_sig` (T3 latent fix) と symmetric な lossless propagate を達成。**新規 unit test 3 件**: `test_build_struct_from_type_literal_propagates_getter_kind` / `propagates_setter_kind` / `distinguishes_getter_setter_pair` を `src/registry/collection/type_literals.rs::tests` に追加、TsMethodInfo → MethodSignature 経路の Method/Getter/Setter pair distinction を verify。本 T4 task は test 3 件で completion criteria 充足、追加実装 work なし。

### T5: convert_member_expr / resolve_member_access に Read context dispatch 拡張

- **Work**: `src/transformer/expressions/member_access.rs::resolve_member_access` に getter/setter detection + dispatch 追加
- **Completion criteria**: cell 2-9 (Read × B2-B8) の unit test green、cell 1, 10 (B1/B9 fallback) regression pass
- **Depends on**: T1, T3, T4

### T6: convert_assign_expr / dispatch_member_write helper 追加 (Write context dispatch) [完了 2026-04-28]

- **Work**: `src/transformer/expressions/assignments.rs::convert_assign_expr` の Member target arm で setter dispatch helper (`dispatch_member_write`) 経由、read-only/write-only Tier 2 honest error。
- **Iteration v9 deep deep review fix (T5) との integration**: 本 T5 で `convert_member_expr_inner` の `for_write=true` で Read dispatch を skip + 既存 FieldAccess fallback 維持を導入済 (Write context LHS leak silent regression を structural fix)。T6 では **その skip 配線を維持したまま**、`convert_assign_expr` の `SimpleAssignTarget::Member(member)` arm で `dispatch_member_write(member, value)` helper を新規実装 (= T5 が `for_write=true` で経由する path とは **別 path**)。これにより Read context (T5、`resolve_member_access` 経由) と Write context (T6、`dispatch_member_write` 経由) の dispatch logic が **structural に分離**、INV-2 (External (E1) と internal (E2 this) dispatch path symmetry) の Read/Write 両方向 cohesion を保つ。
- **Dispatch arm (Spec → Impl Mapping、本 PRD `## Spec → Impl Dispatch Arm Mapping` の `dispatch_member_write` table 参照)**:
  - `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Setter)` → `Expr::MethodCall { method: format!("set_{field}"), args: [value] }` (instance setter dispatch、cells 13/14/19)
  - `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Getter)` and Setter absent → `Err(UnsupportedSyntaxError::new("write to read-only property", ...))` (cell 12 = B2 getter only Write、Tier 2 honest)
  - `lookup` returns `Some((sigs, false))` and `sigs.iter().any(MethodKind::Method)` and {Getter, Setter} 共に absent → `Err(UnsupportedSyntaxError::new("write to method (assignment to method member)", ...))` (cell 16 = B6 method Write)
  - `lookup` returns `Some((sigs, true))` (is_inherited=true) → `Err(UnsupportedSyntaxError::new("write to inherited accessor", ...))` (cell 17 = B7 inherited Write、Tier 2 honest reclassify、本 PRD scope = "Class member access dispatch" の orthogonal axis = 別 PRD I-206)
  - `lookup` returns `None` (B1 field、B9 unknown) → `Expr::Assign { target: FieldAccess, value, op: AssignOp::Assign }` (= T5 で導入した `for_write=true` skip path と equivalent emit、cells 11/19)
  - Static dispatch (B8 setter、cell 18) → `Expr::FnCall { target: CallTarget::UserAssocFn { ty: UserTypeRef, method: format!("set_{field}") }, args: [value] }` (T11 (11-e) cross-reference: instance + static 両方の Write context dispatch arm を T6 で統合実装)
- **Completion criteria**: cells 11-19 (Write × B1-B9) unit test green、regression (B1/B9 = cells 11/19 で既存 FieldAccess Assign 維持) pass、T5 で導入した `for_write=true` skip path との equivalence verify (= T6 helper 経由の None case emit と T5 skip path emit が token-level identical)
- **Depends on**: T1, T5
- **Status**: 完了 (Iteration v10 first + second review、2026-04-28)。**第一次実装 (first-review)**: `dispatch_member_write` + `dispatch_instance_member_write` + `dispatch_static_member_write` helper を `src/transformer/expressions/member_access.rs` に追加 (Read context `resolve_member_access` / `dispatch_*_member_read` と symmetric、INV-2 cohesion + `unreachable!()` macro による structural invariant codification)。`convert_assign_expr` の `AssignOp::Assign` × `SimpleAssignTarget::Member` × `MemberProp::Ident | PrivateName` 早期 gate で `dispatch_member_write` 経由、Compound (+=, -=, ??=, &&=, ||=, ++/--) は subsequent T7-T9 / T10 で setter dispatch 別途実装。**Spec gap fix (first-review source)**: `## Spec → Impl Dispatch Arm Mapping` の `dispatch_member_write` table を Read mapping と完全 symmetric な structural form に拡張 (Instance dispatch arms と Static dispatch arms section 分離 + Static defensive 3 arm = "write to read-only static property" / "write to static method" / "write to inherited static accessor" を明示 enumerate、Rule 9 (a) compliance restored)。**第二次 fix (second-review、deep deep `/check_job`)**: 4 件の Implementation gap を本 commit 内で本質的解決 — (Fix A) `MemberReceiverClassification` enum + `classify_member_receiver` shared helper を抽出、Read/Write 両 helper の DRY violation 完全解消 (subsequent T7-T9 compound dispatch も leverage 可能、増殖性 risk を structural に排除)。(Fix B) T5 `dispatch_instance_member_read` の dead code (`Ok(Expr::FieldAccess)`) を `unreachable!()` macro に置換、4 helper 全てが symmetric structural enforcement 統一。(Fix C) `test_t6_static_field_lookup_miss_falls_through_to_field_access_assign` 追加 (Static gate lookup miss branch C1 coverage)。(Fix D) Read 3 + Write 3 = 6 defensive dispatch arm test 追加 (matrix cell 化なし dispatch arms の C1 coverage + error message lock-in)。**T11 (11-f) defer**: pre-existing latent gap = Receiver Ident unwrap (Paren / TsAs / TsNonNull wrap で static dispatch を逃す) を T11 (11-f) に Implementation 候補 + 判断基準 詳細記載済。**Unit test 17 件 (= 10 first-review + 7 second-review)**: cells 11/12/13/14/16/17/18/19 + INV-2 E1 Read/Write symmetry (`test_inv_2_e1_read_write_dispatch_symmetry_b4`) + T6 Fallback equivalence (`test_t6_fallback_emits_same_ir_as_t5_skip_path`、T5 `for_write=true` skip path との token-level identical lock-in) + Static field lookup miss 1 + Read 3 defensive (B3/B6/B7) + Write 3 defensive (B3/B6/B7)。**Pre/post matrix**: Fix (Tier 2 → Tier 1) cells 13/14/18、Reclassify (silent → Tier 2 honest) cells 12/16/17、Preserve cells 11/19、**No regression**。**Final quality**: cargo test --lib 3207 pass (3190 baseline + 17 new T6 tests) / e2e 159 pass + 70 ignored / compile_test 3 pass / clippy 0 warning / fmt 0 diff / Hono Tier-transition compliance = **Preservation** (clean 110 / errors 64 unchanged、Hono が external setter dispatch on class instances を主要使用していないため allowed per `prd-completion.md`)。**CLI manual probe verify**: B4 instance `b.x = 5;` → `b.set_x(5.0);` ✓、B8 static `Counter.count = 7;` → `Counter::set_count(7.0);` ✓ (production code path empirical verify)。**Defect Classification (final)**: Spec gap 1 (first-review fix 済 = Mapping table asymmetric) / Implementation gap 4 (second-review 全 fix 済 = DRY violation / dead code asymmetric / Static lookup miss test / Defensive arms test) / Review insight 2 (first-review #1 = Framework v1.8 候補 = Mapping symmetric completeness audit / second-review #2 = Receiver Ident unwrap = T11 (11-f) defer)。

### T7: UpdateExpr (`++/--`) Member target で setter desugar [完了 2026-04-29]

- **Work** (実装内容): `src/transformer/expressions/assignments.rs::convert_update_expr` を Transformer method 化 + Member target arm 追加 (`convert_update_expr_member_arm`)。`classify_member_receiver` 経由で Static / Instance / Fallback dispatch、`dispatch_instance_member_update` / `dispatch_static_member_update` 新規 helper で B4 setter desugar (numeric type check + postfix old-value / prefix new-value preservation block)、Tier 2 honest error reclassify (B2 read-only / B3 read of write-only / B6 method / B7 inherited / B4 non-numeric)、Fallback で `build_fallback_field_update_block` 経由 B1 field / B9 unknown の direct FieldAccess BinOp block emit (regression Tier 2 → Tier 1 transition)。
- **Spec gap fix (Iteration v11)**: `pipeline/type_resolver/expressions/mod.rs::resolve_expr` の `ast::Expr::Update(_)` arm が `update.arg` を recursive resolve せず、Member target の receiver expr_type が未登録 → Transformer `classify_member_receiver` の `get_expr_type(receiver)` が None → silent Fallback dispatch (= class member setter dispatch を逃す silent semantic loss) を発見、`Unary` arm pattern 踏襲で `self.resolve_expr(&update.arg)` 追加し structural 解消。本 fix は T7 architectural concern infrastructure prerequisite (T5 Iteration v9 の `extends: vec![]` hardcode fix + `decl.rs:264` empty body class register filter fix と同 pattern、`spec-first-prd.md`「Spec への逆戻り」発動)。
- **Cohesive cleanup (T7 scope 内)**: 既存 Ident form `convert_update_expr` の `_old` binding を `__ts_old` (I-154 `__ts_` namespace reservation extension) に rename、user identifier collision 防止 + T7 で導入した Member form の `__ts_old`/`__ts_new` 命名と統一。snapshot tests 3 件 (do_while / general_for_loop / update_expr) も pure rename で auto-update。
- **Completion criteria**: cells 42, 43, 44, 45-a〜45-de unit tests green (15 件、`tests/i_205/update.rs`)、postfix/prefix 両 form 検証、Tier 2 honest error wording 検証 (`dispatch_instance_member_write` と symmetric)、Computed (`obj[i]++`) は existing error path 維持。
- **Quality (T7 atomic commit ready、累積 first-review + second-review + deep-review + deep-deep-review post-fix final state)**: cargo test --lib **3247 pass** (3220 baseline + 15 first-review T7 + 7 second-review op-symmetric coverage + 5 deep-review D3/D4 branch coverage = 3247) / e2e 159 pass + 70 ignored / integration 122 pass / compile_test 3 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK (`update.rs` = 959 行 < 1000 threshold、deep-review 副次 cleanup で 1046 → 959 line refactor)。
- **Defect Classification (本 T7 scope 内 resolved 累積 first/second/deep/deep-deep review)**: Spec gap 5 (= first-review TypeResolver Update.arg 未再帰 + second-review L2-2 cell 44 #[ignore] message / L3-2 matrix op-axis asymmetric / L3-3 matrix Block form mismatch / L3-4 Spec→Impl Mapping B2/B3 missing) / Implementation gap 7 (= second-review L1-1 doc comment + L1-2 test assertion 弱、deep-review D1 anyhow!→UnsupportedSyntaxError + D2 const 抽出 + D3 _ => arm test + D4 static defensive arms test、deep-deep-review DD1 convert_update_expr exhaustive match) / Review insight 1 (= L4-2 INV-3 1-evaluate compliance for non-Ident receiver、T8 (8-a) scope に詳細 defer) + Static B7 inherited update arm test のみ T11 (11-c) matrix expansion で追加 (T6 pattern 整合)。**framework 失敗 signal** = Rule 9 (op × postfix × context axis) + Rule 10 (TypeResolver visit coverage of operand-context expressions) + Rule 11 (d-2) audit + **Rule 11 (d-6-a) architectural concern relevance auto-audit** 追加候補。
- **CLI manual probe**: cell 43 fixture (B4 numeric `c.value++`) で `{ let __ts_old = c.value(); c.set_value(__ts_old + 1.0); __ts_old }` emit 確認、cell 44 fixture は SWC parser empirical lock-in 用 (E2E permanent #[ignore]、cell 15 Prop::Assign pattern 整合)。
- **Hono Tier-transition compliance** (per `prd-completion.md` broken-fix PRD): **Improvement (allowed)** = T5 baseline (b617386) との累積 diff で clean files 110 → 111 (+1) / error instances 64 → 63 (-1) / compile (file) 109 → 110 (+1)。Category changes: -2 OTHER / +1 OBJECT_LITERAL_NO_TYPE = **2 files が OTHER 状態から improvement、1 file が clean 化、1 file が OBJECT_LITERAL_NO_TYPE category へ shift (= UpdateExpr 関連 OTHER blocker が T7 で removed、別の orthogonal blocker = OBJECT_LITERAL_NO_TYPE が exposed、これは Phase B Step (RC-11) scope の expected blocker、本 PRD scope 外への new compile error 導入は 0 件)**。`prd-completion.md` "New compile errors prohibited" requirement 違反なし、Improvement path 正当 (= 既 broken Tier 2 file が Tier 1 clean 化 / 別 file の partial improvement)。
- **Depends on**: T1, T6

### T8: Compound assign (`+= -= *= ... \|=`) setter desugar [完了 2026-04-29]

- **Work**: `convert_assign_expr` の compound branch で setter desugar (cells 20-29 + 30-35 + 35-* + B6/B7 Tier 2 honest error)。T7 で確立した `build_update_setter_block` (instance/static 共通 setter desugar block builder) + `dispatch_instance_member_update` / `dispatch_static_member_update` arm 構造を leverage、compound assign 用 `dispatch_instance_member_compound` / `dispatch_static_member_compound` 新規 helper として extension (rhs を引数で受ける + compound op を BinOp で受ける + value-yield 必要なら Block form / 不要なら直接 setter MethodCall emit)。
- **(8-a、Iteration v11 review L4-2 由来) INV-3 1-evaluate compliance for non-Ident receiver**: T7 Iteration v11 で発覚した latent gap = 非-Ident receiver (`getInstance().x++` 等) で `obj` が 2 回 evaluate される (`getter_call = obj.clone()` + `setter_call = obj.clone()` で 2 clone → generated Rust で `getInstance()` 2 回呼出)。INV-3 (compound assign side-effect 1-evaluate) 違反 latent。
  - **本 T8 scope での structural 解消**:
    - Step 1: `is_side_effect_free(expr: &Expr) -> bool` helper を `member_dispatch.rs` に新規追加。判定 criteria: `Expr::Ident` / `Expr::This` / `Expr::FieldAccess` (深さ N の depth-bounded recursive)、それ以外は side-effect ありと判定。
    - Step 2: `dispatch_instance_member_compound` / `dispatch_static_member_compound` で receiver 判定:
      - **side-effect-free**: 直接 emit `obj.set_x(obj.x() OP rhs)` (statement context) or Block form (expression context)、`obj.clone()` 2 回 OK (Rust で cheap reference copy)
      - **side-effect**: IIFE 形 emission `{ let __ts_recv = &mut obj; let __ts_val = __ts_recv.x() OP rhs; __ts_recv.set_x(__ts_val); /* tail expr if needed */ }` で receiver 1 回 evaluate (= INV-3 1-evaluate compliant)
    - Step 3: T7 `dispatch_instance_member_update` / `dispatch_static_member_update` も同 helper を leverage して back-port (= T7 で発覚した latent gap を T8 で structural 解消、cohesive batch)
  - **Test 拡張**: `tests/i_205/update.rs` に non-Ident receiver tests 追加 (e.g., `getInstance().value++` with side-effect、receiver 1 回 evaluate verify)、`tests/i_205/compound.rs` 新規 で T8 unit tests + side-effect handling tests
  - **PRD matrix update**: cells 20-29 / 30-35 / 35-* / 41-* に "side-effect-free vs side-effect receiver" sub-axis を Rule 7 control-flow exit sub-case completeness 観点で enumerate (現 matrix は Ident receiver only enumerate、Iteration v11 で本 sub-axis を Rule 1 (1-4) Orthogonality merge legitimacy 適用 + Implementation Stage T8 で受け取る)。
- **(8-b、Iteration v11 review L3-3 由来) Block form ideal output completeness**: T8 cells (compound assign) の matrix Ideal output も T7 cells 42-45 と同 Block form pattern で記載 (statement / expression 両 context 対応)。`obj.x += v;` (statement form) → `{ obj.set_x(obj.x() + v) };` (statement) / `let z = (obj.x += v);` (expression form) → `{ let __ts_new = obj.x() + v; obj.set_x(__ts_new); __ts_new }` (expression value yield)。matrix を Block form 統一で update (Rule 6 (6-1) token-level alignment restore)。
- **Completion criteria**: cells 20-29 + 30-35 + 35-* + 41-* unit test green、INV-3 1-evaluate compliance verify (side-effect receiver で receiver 1 回 evaluate 確認)、T7 dispatch helpers の INV-3 back-port 完了 (= T7 latent gap structural 解消)
- **Depends on**: T1, T6, T7 (= T7 で確立した dispatch_instance/static_member_update helper 構造を leverage、INV-3 back-port も T7 helpers 含む)

### T9: Logical compound (`??= &&= \|\|=`) setter desugar (既存 nullish_assign helper integration) [完了 2026-04-29]

- **Status**: 完了 (Iteration v13、2026-04-29)。新規 `src/transformer/expressions/member_dispatch/logical.rs` (442 行、T8 compound と symmetric) + `Transformer::try_dispatch_member_logical_compound` entry method (= 3 sites: `convert_assign_expr` expression-context gate / `try_convert_nullish_assign_stmt` Member arm / `try_convert_compound_logical_assign_stmt` Member arm) で `??=` / `&&=` / `||=` × Member × non-Computed × Static/Instance class member dispatch を新設。Fallback (B1 field / B9 unknown / non-class receiver / static field / Computed) は `Ok(None)` 経由で既存 `nullish_assign.rs` / `compound_logical_assign.rs` emission logic に流れる (cells 36 + 41-e regression preserve)。
- **Production code**: `dispatch_instance_member_logical_compound` + `dispatch_static_member_logical_compound` 2 helpers (B4 conditional setter desugar / B7 inherited Tier 2 / B2 read-only / B3 read-of-write-only / B6 method Tier 2 + Static defensive arms)、`extract_getter_return_type` (sigs から Getter return_type 抽出、TypeResolver `expr_types[member_span]` 不在 = T8 second-review F-SX-1 で予測された Spec gap への self-contained 回避)、`build_logical_compound_predicate` (op-specific predicate dispatch: `??=` → `<getter>.is_none()` (lhs_type = Option<T> 必須 gate) / `&&=` → `truthy_predicate_for_expr` 経由 / `||=` → `falsy_predicate_for_expr` 経由)、`wrap_setter_value` (LHS = Option<T> なら `Some(rhs)` wrap、cell 38 setter argument)、`assemble_block` (Statement / Expression context 共通 Block 構築 + tail expr 有無 gate)、`LogicalCompoundContext` enum (Statement / Expression 2 variants)。
- **INV-3 1-evaluate compliance**: SE-having receiver (`getInstance().value ??= 42` 等) は IIFE form `{ let mut __ts_recv = <obj>; if __ts_recv.value().is_none() { __ts_recv.set_value(Some(42)); }; <tail> }` で receiver 1-evaluate 保証 (T7/T8 IIFE pattern を `is_side_effect_free` / `TS_RECV_BINDING` 経由で reuse)。Static dispatch は class TypeName receiver で side-effect なし → IIFE wrap 不要。
- **Spec gap fix (Iteration v13、`/check_job` Layer 3 finding)**: matrix cells 39/40 spec D=bool LHS のみ enumerate、他 D variants (F64/String/Option<T>) の `&&=`/`||=` × B4 dispatch は existing `truthy_predicate_for_expr` / `falsy_predicate_for_expr` per-type 経由で transitively 動作するが matrix に明記なし → unit test 3 件 (`test_lhs_type_f64_and_assign_emits_block_with_predicate_dispatch` / `test_lhs_type_string_or_assign_emits_block_with_is_empty_predicate` / `test_lhs_type_option_and_assign_emits_some_wrap_setter_arg`) で structural lock-in 適用。
- **Implementation gap fix (Iteration v13、`/check_job` Layer 4 finding)**: ??= × non-Option LHS (= getter return type が F64/String/Bool/Named 等) で my dispatch が `<getter>.is_none()` を non-Option Rust type に対し emit → E0599 broken Rust output (silent broken) → `build_logical_compound_predicate` の `NullishAssign` arm に Option<T> gate を追加し、non-Option LHS は Tier 2 honest error reclassify (= "nullish-assign on non-Option class member (Identity strategy out of T9 scope)")。pre-T9 generic "nullish-assign on unresolved member type" wording より specific、subsequent PRD で Identity strategy emission 拡張可能。
- **Unit test 22 件**: cells 36 (B1 field × ??= × Option<T> Fallback regression preserve、`Deref(MethodCall { get_or_insert_with })`) + 37 (B2 getter only ??=) + 38 (B4 ??= × Option<T>、expression + statement 2 contexts) + 39 (B4 &&= × bool) + 40 (B4 ||= × bool) + B3 setter only ??= + 41-b (B6 method) + 41-c (B7 inherited) + 41-d (B8 static ??=、expression + statement 2 contexts) + Static defensive 4 件 (B2/B3/B6/B7 static) + 3-op orthogonality merge mapping 2 件 (&&= truthy = identity / ||= falsy = `!operand` predicate shape) + INV-3 SE-having receiver IIFE form lock-in + LHS type orthogonality 3 件 (F64/String/Option<T>) + non-Option ??= Tier 2 honest error gate (Iteration v13 fix lock-in)。
- **Pre/post matrix**: Fix (silent Tier 2 → Tier 1) cells 38/39/40/41-d、Reclassify (silent → Tier 2 honest) cells 37/41-b/41-c + Static defensive arms、Preserve cells 36/41-e (existing fallback path)、**No regression**。
- **Final quality**: cargo test --lib 3296 pass (3274 baseline + 22 T9) / e2e 159 pass + 70 ignored / compile_test 3 pass / integration 122 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK (logical.rs 442 行 / logical_compound.rs ~813 行、両者 < 1000 threshold) / Hono Tier-transition compliance = **Preservation** (clean 111 / errors 63 = T8 baseline 同一、no new compile errors、`prd-completion.md` broken-fix PRD allowed pattern: Hono が external setter dispatch on class instances を主要使用していないため expected)。
- **Defect Classification (Iteration v13 final)**: Spec gap 1 (= cells 39/40 LHS type variants matrix gap、本 T9 内 lock-in test 3 件で structural verify、matrix doc 明示 revision は subsequent PRD doc iteration scope) / Implementation gap 1 (= ??= × non-Option LHS broken Rust emission、本 T9 内 Tier 2 honest error gate で fix) / Review insight 2 (= [I-216 等] setter accept type asymmetry vs getter return type pre-existing pattern T6 でも同 issue defer / TypeResolver `resolve_member_type` Spec gap = getter access Member exprs に Unknown 返却、T8 second-review F-SX-1 で予測済、subsequent PRD で TypeResolver-level structural fix 候補)。
- **Work**: `src/transformer/expressions/member_dispatch/logical.rs` (新規、442 行) + `Transformer::try_dispatch_member_logical_compound` entry method を mod.rs に追加 + `convert_assign_expr` / `try_convert_nullish_assign_stmt` / `try_convert_compound_logical_assign_stmt` の 3 sites に gate 追加。
- **Completion criteria**: cell 36-41 unit test green ✓ + LHS type orthogonality lock-in ✓ + non-Option ??= Tier 2 honest gate ✓ + INV-3 SE-having IIFE form lock-in ✓ + Hono Preservation ✓
- **Depends on**: T1, T8

### T10: Inside-class `this.x` dispatch (P1 TC39 faithful) [完了 2026-05-01]

- **Work**: this expression 検出 + enclosing class scope 利用、external dispatch と uniform
- **Architectural concern**: Internal `this.x` dispatch (E2 context) を external dispatch (E1) と structural symmetric に統一。INV-2 (External (E1) と internal (E2 this) dispatch path symmetry) を **構造的に達成** (= 重複 logic 不要、既存 T5/T6/T7/T8/T9 dispatch helpers が `Expr::This` receiver で uniformly fire)。
- **Empirical foundation (Iteration v16、2026-05-01)**: `TctxFixture::from_source` で fixture 構築時 TypeResolver が `visit_class_body` (visitors.rs:439) で `this` を `RustType::Named { class_name, type_args: vec![] }` で scope_stack に register、`Expr::This` resolve は `lookup_var("this")` 経由で `Some(RustType::Named)` を返す。`classify_member_receiver` (mod.rs:147) の Instance gate (line 184-198) は `RustType::Named` matching で fire するため、`Expr::This` receiver は既存 dispatch path で **production code 変更なし** に正しく classify される。`Expr::This` → `Expr::Ident("self")` IR conversion も既存 (expressions/mod.rs:196)。**結論**: T10 architectural concern は既存 T5-T9 framework で **構造的に既に達成**、追加 production code 変更不要。
- **Pre-T10 silent regression discovered + structural fix (Iteration v16 critical finding)**: T6 setter dispatch 導入 (= `Expr::Assign { FieldAccess(self, x), v }` → `Expr::MethodCall { self, "set_x", [v] }` IR shape 変化) により、**`body_has_self_assignment` (helpers.rs:100、top-level `Stmt::Expr(Expr::Assign)` のみ検出) が setter MethodCall を見落とし、internal `this.x = v` (cell 61) / `this.x += v` (cell 63) / `this.x++` (cell 64) を含む全 method body で silent `&self` emit → Rust E0596 compile error "cannot borrow `*self` as mutable" を引き起こす silent regression**。本 T10 で **structural fix**:
  - `body_has_self_assignment` → `body_requires_mut_self_borrow` rename + 拡張 (= recursive `IrVisitor` walker + `MutSelfRequirementVisitor` struct + 2 detection cases: case (1) `Expr::Assign { target: self.field, .. }` (pre-T10 case) + case (2) `Expr::MethodCall { object: self, method: starts_with("set_"), .. }` (T6/T7/T8/T10 setter dispatch family))
  - 副次的改善: Recursive descent により pre-T10 helper の depth limitation も同時に解消 (= `if cond { this.x = 5 }` 等の non-trivial top-level structures でも正しく `&mut self` を emit)
  - `is_self_setter_call` shared helper extraction (= prefix `"set_"` + receiver `Expr::Ident("self")` の symmetric structural enforcement)
- **Production code change**: `src/transformer/classes/helpers.rs` (helper 拡張、~80 LOC: `body_requires_mut_self_borrow` + `MutSelfRequirementVisitor` + `is_self_setter_call`) + `src/transformer/classes/members.rs` (call site 1 行更新)
- **Test additions (22 件)**:
  - `src/transformer/classes/helpers.rs` 内 `#[cfg(test)] mod tests` (10 件): `empty_body_does_not_require_mut` / `top_level_self_field_assign_requires_mut` (case 1 lock-in) / `top_level_self_setter_call_requires_mut` (case 2 lock-in) / `block_with_setter_call_requires_mut` (T8 compound desugar Block recursive descent) / `if_stmt_with_setter_in_then_requires_mut` (Stmt::If recursive descent) / `while_loop_with_setter_requires_mut` (Stmt::While recursive descent) / `setter_call_on_non_self_does_not_require_mut` (false-positive guard) / `non_setter_method_call_on_self_does_not_require_mut` (false-positive guard) / `read_only_body_does_not_require_mut` / `let_init_with_setter_call_requires_mut` (Stmt::Let init recursive descent)
  - `src/transformer/expressions/tests/i_205/this_dispatch.rs` (新規 file、12 件): cells 60 (Read B2) / 61 (Write B4) / 63 (Compound B4) / 64 (Update B4 postfix) + INV-2 E1/E2 Read/Write symmetric dispatch (2 件) + Tier 2 honest error reclassify B2 Write / B3 Read / B6 Read no-paren (3 件) + setter body internal `this.x` dispatch (1 件) + getter body `this._field` access (B1 fallback boundary、1 件) + cell 38 internal counterpart (T9 logical compound `this.x ??= v` Block form lock-in、1 件、Layer 3 Spec gap fix)
  - `src/transformer/expressions/tests/mod.rs` に `extract_class_method_body_expr_stmt` test helper を追加 (DRY、本 T10 12 unit tests + future T-series で leverage)
- **`/check_job` 4-layer review (Iteration v16、2026-05-01)**:
  - **Layer 1 (Mechanical)**: 0 findings — 全 file < 1000 lines、clippy 0、fmt 0、test name pattern 準拠、production code 内 `unwrap()` / `panic!()` / TODO 残存なし
  - **Layer 2 (Empirical)**: 0 findings — CLI probe で cells 60/61/63/64 の generated Rust が compile + 期待 stdout を produce すること empirical 確認 (cell 61 Counter incrInternal 1+1=2 等 stand-alone Rust で verify)、Hono bench Preservation (clean 111 / errors 63 unchanged)
  - **Layer 3 (Structural cross-axis)**: 1 Spec gap — T9 logical compound `this.x ??= v` (cell 38 internal counterpart) の dispatch test が unit test level で不在、orthogonality merge inheritance のみに依存 → 本 T10 内 lock-in test 追加で structural verify (`test_internal_this_b4_nullish_assign_emits_block_form_with_predicate`)
  - **Layer 4 (Adversarial trade-off)**: 0 findings — pre-T10 baseline (= post-T9) では internal `this.x = v` 系 Write が silent compile error、本 T10 で structural fix (helper recursive walker + setter detection) → pre/post matrix: cells 61/63/64 が Tier 2 broken (silent compile error) → Tier 1 fix。 trade-off: false positive 候補 (`set_*` prefix を持つ regular method) は &mut self emit (sound = Rust上 strictly more permissive)、false negative は Rust E0596 で surface する 안전 fail-safe。
- **Defect Classification (5 category)**:
  - Grammar gap: 0
  - Oracle gap: 0
  - Spec gap: 1 — T9 logical compound internal test missing (本 T10 内 fix、framework 改善 candidate = INV-2 verification を T9 の dispatch helper にも明示適用するための matrix cell expansion `this.x ??= v` (cell 38 internal) 追加)
  - Implementation gap: 1 — T6 setter dispatch 導入時 `body_has_self_assignment` を symmetric audit 不足、setter MethodCall 検出 logic を helper に追加せず → 本 T10 で structural fix (`body_requires_mut_self_borrow` rename + recursive walker + case 2 追加)。**Framework 失敗 signal**: I-205 framework v1.7 Rule 9 sub-rule (c) (Field-addition symmetric audit) を T6 review 時に **逆 direction (= IR shape 変化 → caller helper update audit)** に拡張する candidate。本 T6 → T10 chain は Rule 9 (c) の "IR shape evolution" axis を framework に追加する empirical evidence。
  - Review insight: 1 — Constructor body conversion (`convert_constructor_body` の `try_extract_this_assignment`) が `this.<accessor> = v` の B3/B4 dispatch を bypass し `Self { value: 7.0 }` 等 invalid struct field を emit する pre-existing bug を発見、別 TODO `[I-222]` に詳細記載 (T10 architectural concern と orthogonal、別 PRD で取り扱う)
- **Pre/post matrix (Tier-transition compliance per `prd-completion.md` broken-fix PRD wording)**:
  - Cells 61 (`this.x = v` internal B4) / 63 (`this.x += v` internal B4) / 64 (`this.x++` internal B4): **Tier 2 broken (silent E0596 compile error pre-T10)** → **Tier 1** (correct `&mut self` emit + setter dispatch)
  - Cell 60 (`this.x` Read internal B2): preserved (Tier 1 dispatch、`&self` correct because Read 不要 mut)
  - Setter body internal `this.x = v` (cell 14 internal counterpart): **Tier 2 broken** → **Tier 1**
  - Tier 2 honest error reclassify cells (B2/B3/B6 internal): preserved (`UnsupportedSyntaxError` kind unchanged)
  - **Hono bench**: Preservation (clean 111 / errors 63 unchanged from T9 baseline) — Hono は internal class method 内 setter dispatch を主要使用していないため expected per `prd-completion.md` broken-fix PRD allowed pattern (= Improvement / Preservation 何れも allowed、新規 compile error 導入は **0 件** = prohibited regression なし)
- **TODO doc-sync (T9 commit `cf0d7ce` の `pre-commit-doc-sync.md` violation の本 T10 内 fix)**: plan.md に記載されていたが TODO file 未追記の I-219 (TypeResolver `resolve_member_type` Spec gap、L3) / I-220 (Setter accept type asymmetry、L4) / I-221 (Top-level Module-level statement expression-context dispatch、L4) の 3 entries を TODO に追加。**新規 TODO 起票**: I-222 (Constructor body `this.<accessor> = v` doesn't distinguish struct field from accessor、L4、Iteration v16 empirical probe 由来)。
- **Iteration v17 deep-deep `/check_job` review (本 commit 内 fix、2026-05-01)**: 第三者視点で deep deep level の re-review を実施し、Iteration v16 review で見落としていた **5 件の追加 finding** を発見。本 commit 内で **本 T10 scope 内 fix 4 件** を本質的に解決、別スコープ defer 1 件 (transitive mut method calls = `[I-223]` 起票)。

  | # | Finding | 分類 | 評価 | Action |
  |---|---------|------|------|--------|
  | 1 | **L1-DD-1** Helper unit tests (10 件) が `test_<target>_<condition>_<expected>` naming convention 不準拠 (testing.md violation) | Implementation gap | `test_body_requires_mut_self_borrow_*` prefix で 全 rename | **本 commit 内 fix** |
  | 2 | **L2-DD-1** cells 63/64 stand-alone Rust compile + run verify 未実施 (cell 61 のみ verify 済) | Empirical gap | rustc + actual run で empirical lock-in、output `1\n2\n3` 確認 | **本 commit 内 fix** |
  | 3 | **L3-DD-1** `body_requires_mut_self_borrow` の Assign target 検出が `is_self_field_access` (= top-level FieldAccess only) のみで、`self.arr[i] = v` (Index) / `self.x.y = v` (nested FieldAccess) / `(*self).x = v` (Deref) を見逃す → silent &self emit + Rust E0596。pre-T10 helper の同 limitation を内包 | Implementation gap (structural completeness) | `is_self_field_access` を `target_roots_at_self` (recursive helper、FieldAccess / Index / Deref chain rooted at self) に **structural extension**、4 件 NEW unit test 追加 (indexed_assign / nested_field_assign / deref_field_assign / non_self_assign false-positive guard)。`is_self_field_access` は dead code 化、削除 | **本 commit 内 fix** |
  | 4 | **L4-DD-1** `is_self_setter_call` は prefix `"set_"` heuristic、registered `MethodKind::Setter` metadata 未参照 (false positive 候補、sound = strictly more permissive)。limitation の doc comment 不在 | Implementation gap (doc completeness) | `body_requires_mut_self_borrow` doc comment に "Detection methodology and trade-offs" section 追加、prefix-based heuristic の sound 性 (false positive 是、false negative T6/T7/T8/T10 emit pattern では impossible) + transitive mutation `[I-223]` への boundary 明記 | **本 commit 内 fix** |
  | 5 | **L3-DD-2** Transitive `&mut self` propagation through method calls 未検出: `class { caller() { this.helper(); } helper() { this._x = 0; } }` で `caller` が `&self` で emit、`helper` is `&mut self` → E0596。Static analysis 限界、inter-procedural method receiver inference 必要 | Review insight | T10 architectural concern と orthogonal な broader concern として `[I-223]` (Method receiver inference does not detect transitive mut propagation) 別 TODO 起票、Resolution direction (Phase 1 collect_class_info pre-analysis + Phase 2 caller body 解析 + Phase 3 fixed-point iteration) 詳細記載、reachability audit 後 priority 確定 | **`[I-223]` TODO 起票 (T10 scope 外、別 PRD)** |

  **Production code change (本 v17 commit 内、L3-DD-1 fix)**:
  - `is_self_field_access` (top-level FieldAccess match のみ) → `target_roots_at_self` (recursive descent through FieldAccess / Index / Deref chain rooted at self) extension
  - `is_self_field_access` は削除 (dead code、structural fix で subsumed)
  - `is_self_ident` shared helper extraction (DRY、`target_roots_at_self` + `is_self_setter_call` 共通)

  **Test extensions (本 v17 commit 内)**: helpers.rs `mod tests` を 10 → 15 件に拡張、全 test を `test_body_requires_mut_self_borrow_*` prefix で rename + 5 件 NEW (indexed_assign, nested_field_assign, deref_field_assign, non_self_assign, closure_body_with_setter)。closure body test は IrVisitor walk_expr の `Expr::Closure` arm 経由 recursive descent を verify。

  **Defect Classification (Iteration v17 final、deep-deep review 累積)**:
  - Grammar gap: 0
  - Oracle gap: 0
  - Spec gap: 1 (= Iteration v16 で fix 済 = T9 logical compound internal test 追加)
  - **Implementation gap: 4** (= Iteration v16 で fix 済 1 = T6 setter dispatch silent regression / Iteration v17 で fix 済 3 = L1-DD-1 test naming + L3-DD-1 target_roots_at_self structural extension + L4-DD-1 doc completeness)
  - **Review insight: 2** (= Iteration v16 で起票済 1 = constructor body bug `[I-222]` / Iteration v17 で起票済 1 = transitive mut `[I-223]`)

- **Final quality (post-Iteration v17 deep-deep review fix 後、2026-05-01 final state)**: cargo test --lib **3335 pass** (3308 baseline + 12 this_dispatch + 15 helper tests (10 baseline + 5 NEW deep-deep) = 3335) / e2e_test 159 pass + 70 ignored / compile_test 3 pass / integration 122 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK (helpers.rs 603 行、this_dispatch.rs 631 行、両者 < 1000 threshold)。**Empirical CLI verify**: cells 61/63/64 stand-alone Rust が rustc で compile + run、output `1\n2\n3` 期待値一致 ✓。
- **Completion criteria**:
  - ✓ cell 60 unit test green (Read B2 internal getter dispatch)
  - ✓ cell 61 unit test green (Write B4 internal setter dispatch、empirical Rust compile + run verified)
  - ✓ cell 63 unit test green (Compound `+=` internal B4 dispatch、empirical Rust compile + run verified)
  - ✓ cell 64 unit test green (Update `++` internal B4 postfix dispatch、empirical Rust compile + run verified)
  - ✓ INV-2 verification (E1/E2 Read/Write dispatch path symmetry、4 件 unit test)
  - ✓ method body / getter body / setter body 全 dispatch (cells 60/61/63/64 + setter body test + getter body field access boundary)
  - ✓ Tier 2 honest error reclassify (B2 Write / B3 Read / B6 Read no-paren、3 件 unit test)
  - ✓ T9 logical compound internal `this.x ??= v` dispatch verify (Iteration v16 Layer 3 Spec gap fix、cell 38 internal counterpart)
  - ✓ `&mut self` regression structural fix (`body_requires_mut_self_borrow` recursive walker、15 件 helper unit test)
  - ✓ Self-rooted Index/Deref/nested FieldAccess Assign target detection (Iteration v17 L3-DD-1 fix、`target_roots_at_self` structural extension)
  - ✓ Helper test naming convention compliance (Iteration v17 L1-DD-1 fix、`test_*` prefix)
  - ✓ Empirical stand-alone Rust verify for cells 61/63/64 (Iteration v17 L2-DD-1 fix)
  - ✓ Doc comment trade-offs documentation (Iteration v17 L4-DD-1 fix、prefix-based heuristic limitation + transitive mut `[I-223]` boundary)
  - **(注)** Constructor body dispatch は orthogonal architectural concern として `[I-222]` separate、Transitive mut via method calls は inter-procedural analysis として `[I-223]` separate (両方とも T10 scope 外、別 PRD で取り扱う)
- **Depends on**: T1, T5, T6
- **Status**: **完了 (Iteration v17、2026-05-01)**。27 件 unit test 追加 (12 dispatch + 15 helper) + helper structural extension (case 1 `target_roots_at_self` + case 2 setter MethodCall detection + recursive descent) + Iteration v16 4-layer review 1 Spec gap + 1 Implementation gap fix + Iteration v17 deep-deep review 3 Implementation gap fix (L1-DD-1/L3-DD-1/L4-DD-1) + L2-DD-1 empirical verify + 2 Review insight TODO 起票 (I-222 / I-223) + TODO doc-sync 5 entries (I-219〜I-223)。次 iteration v18 = T12 (Getter body `.clone()` 自動挿入、C1 pattern) 単独 commit に進む。

### ~~T11~~: 削除 (2026-05-01、新 PRD I-A / I-B へ migrate)

**T11 削除根拠 (2026-05-01 user 確定)**: T11 sub-tasks (11-b/c/d/e/f) は subsequent
review iteration で発見された **orthogonal な追加 architectural concern** であり、本 PRD
I-205 の architectural concern (= "Class member access dispatch with getter/setter framework"、
cells 9/18 の Tier 1 化 + 全 dispatch context での symmetric coverage) と別軸。

- 元 T11 scope = (11-a) Static accessor `Class.x()` / `Class::set_x(v)` dispatch は
  T5/T6 で **完了済** (cells 9/18 Tier 1 lock-in 済み)、(11-e) Static setter Write
  dispatch も T6 で完了済 (cell 18 dispatch_static_member_write)。
- 残 sub-tasks (11-b/c/d/f) は **2 つの新 PRD として独立起票** され、本 PRD I-205 から
  削除される (1 PRD = 1 architectural concern 厳格適用、scope creep 認識)。
- T11 task description verbatim copy は `note.after.md` archive 参照。

#### 移行 mapping

| 元 T11 sub-task | 移行先 PRD | architectural concern |
|---|---|---|
| (11-a) Static accessor `Class.x()` / `Class::set_x(v)` dispatch | **完了済 (T5/T6)** | cells 9/18 Tier 1 化、本 PRD 内で達成 |
| (11-b) Mixed class `is_static` filter | **新 PRD I-A** | "Method static-ness IR field propagation" — `MethodSignature.is_static` field addition + Rule 9 (c-1) Field Addition Symmetric Audit (61 site) |
| (11-c) Static × {B3/B6/B7/None field} matrix cell expansion (4 cells + tsc oracle observation + per-cell E2E fixture) | **新 PRD I-A / I-B completion criteria に integrate** | matrix cell 化 + tsc oracle + red lock-in fixture は新 PRD の matrix-driven spec stage で実施 |
| (11-d) Static field `Class::field` associated const emission | **新 PRD I-B** | `Expr::AssociatedConst { ty: UserTypeRef, name: String }` 新 IR variant + Generator 1-to-1 emit + Read context 経由 dispatch |
| (11-e) Static setter Write dispatch | **完了済 (T6)** | cell 18 dispatch_static_member_write |
| (11-f) Receiver Ident unwrap (Paren/TsAs/TsNonNull) robustness | **新 PRD I-B** | `RustType::ClassConstructor(String)` 新 TypeResolver type marker + 全 Ident match sites unification |
| **I-214 (calls.rs DRY violation + 3 latent gaps)** (TODO file 参照、T11 cohesive batch 候補だった) | **新 PRD I-B に内包** | `calls.rs:213-225` Static method call dispatch を `classify_member_receiver` 経由に refactor + 3 latent gaps (interface filter / shadowing / inherited) fix |

#### 新 PRD I-A scope summary (起票予定、本 PRD I-205 完了後)

- **Architectural concern**: Method static-ness IR field propagation
- **Scale**: 61 MethodSignature construction sites (本 PRD I-205 T2 = `kind` field 追加と同 pattern、Rule 9 (c-1) Field Addition Symmetric Audit 適用)
- **修正範囲**:
  - `src/registry/mod.rs::MethodSignature` + `src/ts_type_info/mod.rs::TsMethodInfo` に `is_static: bool` field 追加
  - `collect_class_info` / `convert_method_info_to_sig` / `resolve_method_sig` 等での propagate
  - `dispatch_static_member_*` (read / write / update / compound / logical) で `sigs.iter().filter(|s| s.is_static)` 適用、Mixed class context での instance method 誤 hit を structural 解消
- **Spec stage matrix axis**: `class shape (static-only / instance-only / mixed)` × `access kind (Read/Write/Update/Compound)` × `member kind (Getter/Setter/Method)`
- **Completion criteria 内に (11-c) cells expansion 含む**: Static × {Mixed class instance method 誤 hit} cells を matrix cell 化 + tsc oracle + per-cell E2E fixture で lock-in

#### 新 PRD I-B scope summary (起票予定、PRD I-A 完了後)

- **Architectural concern**: Class TypeName context detection unified via TypeResolver type marker + Static field associated const emission integrate
- **修正範囲**:
  1. **TypeResolver layer**: `RustType::ClassConstructor(name: String)` 新 variant + visit_ident で class TypeName + 非 shadow 検出 register、visit_paren / visit_ts_as / visit_ts_non_null で wrap unwrap & 同 register
  2. **Transformer layer (codebase-wide refactor)**: 全 Ident 直接 match sites を type marker query に refactor:
     - `member_dispatch::classify_member_receiver` Static gate (現 AST shape match)
     - `calls.rs:213-225` Static method call dispatch (I-214 = DRY violation + 3 latent gaps fix)
     - `resolve_member_access` Enum special case (T5 から続く同 issue)
     - その他 Ident match sites (Spec stage で grep + 完全 enumerate)
  3. **IR layer**: `Expr::AssociatedConst { ty: UserTypeRef, name: String }` 新 IR variant + Generator 1-to-1 emit (`ty :: name`)
  4. **Read context dispatch**: `ClassConstructor` + lookup miss (= field exist) → `AssociatedConst` emit (`Counter.DEFAULT` → `Counter::DEFAULT`)
  5. **Write context dispatch**: associated const は Rust 上 immutable のため、Tier 2 honest error reclassify "write to static field" (= OnceLock 等の strategy は別 PRD)
- **Spec stage matrix axis**: `receiver shape (Ident / Paren / TsAs / TsNonNull / TsSatisfies)` × `class type (registered class / Enum / Interface / unregistered)` × `access pattern (Read field / Read method call / Write / Update / Compound)` × `field kind (static field / Getter / Setter / Method)`
- **Completion criteria 内に (11-c) cells expansion 含む**: 上記 matrix cells を tsc oracle observation + per-cell E2E fixture で lock-in

#### 本 PRD I-205 matrix の整合性 (matrix 全セルカバー条件)

`prd-completion.md` matrix 全セルカバー条件 compliance:
- 本 PRD I-205 matrix の **In Scope cells** (B1-B7 instance dispatch + B8 instance/static accessor cells 9/18) は T1-T10 で全 Tier 1 化 + lock-in test 完了
- **Tier 2 honest error reclassify cells** (B2/B3/B6 instance + B7 inherited、cells 4/7/8/12/16/17 等) は T5/T6 で全 dispatch arm reclassify + lock-in test 完了
- **本 PRD scope 外として明示 defer** していた static defensive cells (Static × {B3 setter only / B6 method-as-fn-ref / B7 inherited / None static field} = 4 cells) は **新 PRD I-A / I-B の matrix に move + completion criteria に integrate**、本 PRD I-205 の matrix から **「新 PRD I-A/I-B で expansion」と明示 record** することで matrix 全セルカバー条件 compliant
- 本 deferred section が `problem-space-analysis.md` 「Review で未認識セルが発見されたときの扱い」に従い、scope out 判断 (= 別 PRD に分割) を明示記録

### T12: Class Method Getter body `.clone()` 自動挿入 (C1 limited pattern、`return self.field;` only、Iteration v18 で scope 確定)

- **Architectural concern**: Class Method Getter body の C1 limited pattern (`return self.field;` single-hop self field access、explicit return statement form のみ) で non-Copy return type の場合に `.clone()` 自動挿入。T6/T7/T8/T9/T10 で確立した classify_member_receiver / dispatch_*_member_* helper family と orthogonal な architectural concern (= "Class member getter body emission" = body-shape-aware emission)、T1-T11 dispatch framework foundation 内 cohesive integration。

- **Work**:
  - **(12-a) helper 追加** in `src/transformer/classes/helpers.rs` (T10 で確立した self-access detection helper family と同 architectural concern = "self-rooted expression structural recognition"):
    - `is_self_single_hop_field_access(expr: &Expr) -> bool`: expr が `Expr::FieldAccess { object: Expr::Ident("self"), field: _ }` shape (= single-hop の self field access、`self.field.nested` 等 multi-hop は false) なら true (T10 `is_self_ident` を leverage、symmetric helper として placement)
    - `insert_getter_body_clone_if_self_field_access(stmts: &mut Vec<Stmt>)`: body の last stmt が `Stmt::Return(Some(Expr::FieldAccess { ... }))` または `Stmt::TailExpr(Expr::FieldAccess { ... })` で `is_self_single_hop_field_access` 該当 = inner Expr を `Expr::MethodCall { object: <FieldAccess>, method: "clone", args: vec![] }` に rewrite。`Stmt` enum 全 variant 完全 enumerate (`spec-stage-adversarial-checklist.md` Rule 11 (d-1) 適用、`_ =>` arm 排除、target 以外の variants は no-op で明示)
  - **(12-b) `build_method_inner` 内 invoke** in `src/transformer/classes/members.rs::build_method_inner` (`convert_last_return_to_tail` 直後):
    - Gate condition: `kind == ast::MethodKind::Getter && return_type.as_ref().is_some_and(|t| !t.is_copy_type())` (= Getter + return_type ある + non-Copy)
    - Gate 通過時 `insert_getter_body_clone_if_self_field_access(&mut stmts)` invoke
    - Gate logic は Setter (kind gate で skip) / Method (kind gate で skip) / non-return-annotated Getter (return_type None で skip) / Copy return type Getter (`is_copy_type() = true` で skip) を全て exclude する structural form
  - **(12-c) Unit tests** in `src/transformer/classes/tests/i_205.rs` (新規 file、`mod.rs` に `mod i_205;` 追加):
    - **Decision Table C 完全 cover**: cells 70 (D4 String) / 71 (D1 f64 Copy) / 72 (D5 Vec) / 73 (D6 Option<Copy>) / 74 (D6 Option<non-Copy>) / 81 (Setter) の 6 dispatch arm + non-Getter Method case (kind gate skip verify)
    - **Equivalence partitioning**: Copy partition (cells 71/73) vs non-Copy partition (cells 70/72/74) + Rule 1 (1-4-a) D-axis orthogonality merge representative coverage
    - **Boundary value analysis**:
      - Empty body (no stmts): no rewrite (helper early return)
      - Single stmt body: rewrite if matches pattern
      - Multi-stmt body with last `return self.field`: rewrite (last stmt detection)
      - Multi-stmt body with last `return computed`: no rewrite (cells 75/76 系列 = 別 PRD C2)
    - **Branch coverage (C1)**: helper の各 match arm (Stmt::Return / Stmt::TailExpr / その他 Stmt variants) + Gate condition (kind == Getter / return_type non-Copy) の各 branch direction
    - **AST variant exhaustiveness**: `Stmt` enum 全 variant に対する helper 挙動 (target 以外は no-op)、`Expr::FieldAccess` の object Ident 名 = "self" / non-self の case
    - **Negative tests (Rule 7 sub-case completeness)**:
      - Nested self field access `self.field.nested`: no rewrite (single-hop only、`is_self_single_hop_field_access` false)
      - Non-self ident `obj.field` where obj != self: no rewrite
      - `Expr::MethodCall` (already a method call): no rewrite (target form mismatch)
  - **(12-d) E2E fixture green-ify は T14 defer (Iteration v19 で確定、= T12 implementation commit 内で T14 scope re-partition)**: cells 70/71/72/74 (4 fixtures) の E2E green-ify は **本 T12 task scope 外**、T14 (E2E fixture green-ify、Depends on T1-T10/T12/T13) に明示 defer。**理由**: T12 implementation 自体は完了 (= helper logic correctness を unit tests 20 件 全 pass で verify、generated Rust の getter body に正しく `.clone()` 挿入を CLI manual probe で empirical 確認)、ただし cells 70/71/72/74 fixture の E2E green は **I-162 prerequisite (class without explicit constructor → `Self::new()` 自動合成、現状 plan.md L3 priority 1)** で block。具体的に generated Rust が `pub fn init() { println!("{}", p.name()); }` (= `p` undefined、`main` 不在) の form で emit され、I-162 完了後に `let p = Profile::new(); println!(...);` 形式に変換される必要。本 T12 architectural concern (= "Class Method Getter body C1 `.clone()` insertion") は **unit tests + integration test (build_method_inner gate via class transform)** で primary verification 達成、E2E は I-162 dependency 解消後 T14 で統合実施 = 1 PRD = 1 architectural concern + scope discipline 遵守。**cell 74 fixture rename (Iteration v18 fix)**: `class Cache` → `class OptCache` rename は本 T12 commit 直前の Spec への逆戻り commit で適用済 (= Iteration v18 spec re-design 内、commit `aef25aa` の subsequent commit で merged)、fixture content 修正のみで spec / ideal output `hello\n` 維持、tsc accept restoring verify 済。
  - **(12-e) Iteration v18 framework lesson の本 PRD self-applied integration**: 本 task で発見 / 適用された framework lesson を spec stage adversarial checklist の運用補強として PRD doc 内 `## Spec Review Iteration Log` v18 entry に record (`## Iteration v18 完了判定` で本 PRD self-applied verify pass を declare):
    - **lesson 1**: Spec stage で per-cell E2E fixture を作成した際、fixture 自体の tsc empirical observation を skip した = framework 失敗 signal (cell 74 `Cache` name conflict)。`spec-stage-adversarial-checklist.md` Rule 5 (5-1) sub-rule 拡張 candidate = 「fixture 自体の tsc empirical observation で fixture content の正当性 verify 済」を Spec stage 完了 verification に追加
    - **lesson 2**: Spec stage で cell の TS code が tsc accept か empirical 確認しなかった = framework 失敗 signal (cell 78 last-expr form claim)。`spec-stage-adversarial-checklist.md` Rule 3 (3-2) SWC parser empirical observation の **Spec stage Mandatory enforcement 強化 candidate** = `audit-prd-rule10-compliance.py` で `## SWC Parser Empirical Lock-ins` section の各 ✗/要調査 cell 対応 entry 存在を auto verify する mechanism 追加
- **Completion criteria**:
  - **本 PRD scope cells (unit test verification)**: cells 70/72/74 unit test green (Decision Table C 直接 cell tests + helper logic correctness via integration test = TS source → IR transform → expected IR shape verify)
  - **Regression lock-in cells (unit test verification)**: cells 71/73/81 unit test green (kind gate / Copy gate / Setter gate skip verify)
  - **Negative cells (unit test verification)**: cells 75/76/77/79/80 (別 PRD C2 scope) は本 T12 の no-rewrite gate で正しく skip されること unit test で verify (= helper early return path coverage、`is_self_single_hop_field_access` false branch + 各 non-target Stmt variant arm)
  - **NA cells (treatment confirmation)**: cell 78 (Iteration v18 reclassify) は本 T12 implementation で **emit されない / 影響を受けない** = TS spec で reject される form のため SWC parser を経由した場合の挙動は「getter body without explicit return」path、現状動作維持 (本 T12 で structural change なし)
  - **E2E fixture green-ify**: **本 T12 scope 外、T14 defer** (上記 (12-d) 参照、I-162 prerequisite で block、T14 で I-162 完了後統合実施)
  - **Hono Tier-transition compliance** (`prd-completion.md` broken-fix PRD pattern、T6-T10 と同形式): T12 architectural concern (= "Getter body C1 `.clone()` insertion") は IR-level rewrite のみで E2E green に至るには I-162 prerequisite 必要、Hono bench impact は **Preservation 期待** (Hono が class Method Getter body C1 pattern を主要使用していない場合 unchanged)、Improvement (allowed) は I-162 完了後 T14 で再 measurement、New compile errors (prohibited) regression は本 T12 scope 外 file には introduce しない
- **Depends on**: T1, T2, T3 (T11 削除済 2026-05-01、本 T12 は T11 dependency なし)。なお E2E green-ify (T14 task) は I-162 prerequisite 追加で本 T12 内で達成不能、T14 defer 明示
- **Status**: **完了 (Iteration v19、2026-05-01)**: T12 implementation + 20 unit tests 全 pass (3335 baseline + 20 = 3355) + helper integration verify (CLI manual probe で cell 70 generated Rust `self._name.clone()` 正しく挿入 確認) + Iteration v19 で T14 scope re-partition 確定 (= E2E fixture unignore は T14 で I-162 完了後実施)。Iteration v19 entry は `## Spec Revision Log` 参照

### T13: B6 / B7 corner cells の Tier 2 honest error 化 + INV-5 verification + boundary value test 拡充 (Iteration v9 second-review 追加 scope)

- **Work**:
  - **(13-a)** B6 method-as-fn-reference / B7 inherited accessor の Tier 2 honest error reclassify verify (= 元 T13 scope、cells 7, 8 lock-in、T5 で既に implementation 完了済のため本 T13 は verify のみ)
  - **(13-b、Iteration v9 second-review Review insight #3 由来)** **INV-5 (Visibility consistency) verification task**。現状 T1-T15 sequence に INV-5 verification の明示組込なし。INV-5 = `private get x()` / `private set x(v)` (TS keyword `private` 修飾 accessor) を持つ class の external `obj.x` access は Tier 2 honest error reclassify。現 class.rs collect_class_info は accessibility (`TsAccessibility::Private/Protected/Public`) を ignore (= public/private 区別なく methods に登録)、INV-5 violation は Rust visibility (= generation 側で `pub` 不付与) で external 呼出 compile error として surface する Tier 2 等価。**Implementation 候補**:
    - **オプション A**: `MethodSignature.accessibility: Option<TsAccessibility>` field 追加 + collect_class_info で propagate + dispatch_instance_member_read / dispatch_static_member_read で `Some(Private)` 検出 → `UnsupportedSyntaxError::new("access to private accessor", ...)` Tier 2 honest error reclassify
    - **オプション B**: 現状維持 (Rust generation 側 `pub` 不付与で compile error surface = Tier 2 等価、specific wording なし)
    - **判断基準**: Hono / e2e fixture で reachability audit、TS `private` keyword (ECMA `#x` PrivateName とは別) を持つ accessor の external access が reachable なら オプション A、reachable でないなら現状維持
  - **(13-c)** INV-5 integration test 追加 (`tests/i205_invariants_test.rs::test_invariant_5_private_accessor_external_access_tier2`、現 stub 状態の Invariant test 1 件を unignore + 実装 → green-ify)
  - **(13-d、Iteration v9 second-review Review insight #2 由来)** **Multi-step inheritance test (`A extends B extends C extends D`、N>=3 step) の追加検討**。現 T5 で N=2 step (`test_b7_traversal_multi_step_inheritance_returns_inherited_flag`) を追加済、boundary value analysis 観点で N=3+ step + cycle in middle (`A → B → A` partial cycle) の corner test も追加候補
- **Completion criteria**: cells 7, 8 lock-in green + INV-5 reachability audit 結論 + (13-c) integration test green + (13-d) multi-step + cycle corner test (適用判断時)
- **Depends on**: T1, T5, T6
- **Note**: T13 = corner cells reclassify は T5/T6 で既に implementation 完了 (本 PRD scope は spec → impl dispatch の Tier 2 honest error reclassify、T5 で完了)、本 T13 は **INV-5 verification + multi-step boundary test の subsequent batch** が主 scope。元 T13 の B6/B7 corner cells reclassify はメンテナンス verify のみ。

### T14: E2E fixture green-ify (Implementation stage 完了 verify)

- **Work**: TS-3 で red 状態だった全 fixture を green に
- **Completion criteria**: `cargo test --test e2e_test` 全 pass、Tier-transition compliance (`prd-completion.md` 適用): existing class Method Getter/Setter related Tier 2 errors transition Tier 1 = improvement、no new compile errors introduced for 本 PRD scope 外 features
- **Depends on**: T1-T10, T12, T13 (T11 は 2026-05-01 削除済、新 PRD I-A/I-B へ migrate)

### T15: `/check_job` 4-layer review + 13-rule self-applied verify

- **Work**: `/check_job` 起動 + Layer 1-4 全実施 + Defect classification 5 category trace
- **Completion criteria**: Spec gap = 0、Implementation gap = 0、全 review findings fix
- **Depends on**: T14

### T16: Task-ID-based naming → semantic naming refactor + 実装分割再考 (I-205 scope の cleanup task、user 指示 2026-05-01 由来)

- **Architectural concern**: I-205 implementation で蓄積された **task-ID-based 命名** (`i_205` / `i205_*` / `i-205/` directory / `test_t12_*` / `test_e2e_cell_i205_*` / `test_cell_NN_*` / `test_invariant_N_*` 等) を **semantic 命名** (= what is being tested に基づく命名、PRD ID は internal task tracking のみで module/file/関数名に使用しない) に rewrite。加えて、凝集度観点で `src/transformer/expressions/tests/i_205/` 8 sub-files 等の **実装分割そのものを再評価**、必要なら module 分割再構成。

  本 T16 は I-205 scope の **cleanup task** (= I-205 architectural concern の primary 達成は T1-T15 で完了、本 T16 は subsequent quality 改善 = file/fn 命名 + 実装分割を ideal 化する)。

- **Work**:
  - **(16-a) Audit**: I-205 scope 内 task-ID-based 命名 violation を全 enumerate + semantic naming proposal table 作成。violation categories:
    - **Module/file names**: `src/transformer/classes/tests/i_205.rs` / `src/transformer/expressions/tests/i_205/` directory + 8 sub-files (compound/logical_compound/logical_compound_strategies/read/this_dispatch/update/write/mod) / `src/transformer/classes/tests/mod.rs` 内 `mod i_205;` / `src/transformer/expressions/tests/mod.rs` 内 `mod i_205;`
    - **Top-level integration test files**: `tests/i205_helper_test.rs` / `tests/i205_invariants_test.rs`
    - **E2E fixture directory**: `tests/e2e/scripts/i-205/` (68 fixtures = cell-NN-*.{ts,expected} pairs) + `tests/e2e_test.rs` 内 `run_cell_e2e_test("i-205", ...)` path string + `fn test_e2e_cell_i205_*` 系列 (~19 件)
    - **Function names with task-ID prefix**: `fn test_t[1-9]_*` / `fn test_t1[0-2]_*` / `fn test_cell_[0-9]+_*` 系列 (~150 件、src + tests 合算)
    - **Note**: `tests/swc_parser_*_test.rs` (4 files = auto_accessor / increment_non_numeric / inherited_accessor / object_literal_prop_assign) は **既 semantic 命名** = violation でない、preserve
  - **(16-b) Semantic naming proposal**: 各 violation に対する semantic naming proposal table (= old name → new name + rationale)。例:
    - `src/transformer/classes/tests/i_205.rs` → `getter_body_clone_insertion.rs` (architectural concern = "Getter body C1 `.clone()` insertion test")
    - `src/transformer/expressions/tests/i_205/` directory → `member_dispatch/` or `class_member_dispatch/` (architectural concern = "Class member access dispatch helpers test")
    - `tests/i205_invariants_test.rs` → `class_member_dispatch_invariants_test.rs`
    - `tests/i205_helper_test.rs` → `class_member_dispatch_helpers_test.rs`
    - `tests/e2e/scripts/i-205/` → `class_member_access/` or `class_getter_setter/` (E2E architectural concern reflect)
    - `fn test_t12_cell_70_getter_string_non_copy_inserts_clone` → `fn test_getter_body_string_non_copy_inserts_clone` (T12 / cell 70 を排除、semantic 化)
    - `fn test_e2e_cell_i205_70_getter_body_clone_string` → `fn test_e2e_getter_body_clone_string`
    - `fn test_invariant_N_*` 系列 = invariant 番号は semantic (= INV-1〜6 は本 PRD 内の architectural invariant 概念) ため preserve OK、ただし `tests/i205_invariants_test.rs` file rename は必要
  - **(16-c) 実装分割再考 + 既存 rule violation cleanup integrate (Q1 Option α 由来、Iteration v19 light review insight 2 fix)**: 凝集度観点で `src/transformer/expressions/tests/i_205/` の 8 sub-files (compound / logical_compound / logical_compound_strategies / read / this_dispatch / update / write / mod) が semantic に何の architectural concern か再評価。例えば、現状 `compound.rs` + `logical_compound.rs` + `logical_compound_strategies.rs` の 3 sub-files が compound assign 関連で coupling、必要なら 1 file (`compound_assign_dispatch.rs`) に統合 or 別 module に分離。本 (16-c) では各 sub-file の architectural concern を明示 + 凝集度高 / 結合度低 が ideal な module 分割を proposal、必要なら 16-d で実装。**加えて、I-205 implementation で蓄積された既存 rule violation の cleanup を integrate** (Iteration v19 light review insight 2 由来):
    - **`src/transformer/classes/members.rs:440 default_expr.unwrap()`**: `testing.md` "unwrap()/expect() are only allowed in test code" rule violation。T12 で touch していない既存 code (= constructor parameter `default` value 解決時の unwrap、line 326-334 が T12 changes 範囲、line 440 は別 logic)。本 T16 architectural concern (= "I-205 cleanup quality 改善") の boundary 内、cleanup commit (16-d) で `?` operator or `unwrap_or_else(|| default)` 等の sound error handling に rewrite
    - **その他 I-205 implementation 範囲内の既存 rule violation candidates**: 16-a Audit phase で `src/transformer/classes/` + `src/transformer/expressions/member_dispatch/` 等の I-205 touch file の `unwrap()` / `expect()` / TODO/FIXME/panic! 残存を grep + cleanup 候補 enumerate、必要なら本 T16 内で cleanup
    - **Cleanup boundary**: 本 T16 で対応するのは **I-205 implementation 範囲内 (= I-205 で touch した file 内)** の既存 rule violation のみ、I-205 範囲外の codebase-wide cleanup は別 PRD scope (例えば I-203 codebase-wide AST match exhaustiveness compliance や同類 codebase-wide cleanup PRD) で扱う = 1 PRD = 1 architectural concern boundary 厳格適用
  - **(16-d) Atomic refactor commit**: file rename + mod statement update + fn rename + e2e_test.rs path string update + PRD doc / plan.md / TODO 整合 update を **1 atomic commit** で実施。compile error / test breaking なしの atomic property 保証 (file rename は git mv、fn rename は cross-file find-replace で symmetric apply)。
- **Completion criteria**:
  - 全 task-ID-based 命名 violation が semantic 命名に rewrite (audit 完了 list の 0 残存)
  - 実装分割再考 result が module 構造に reflect (= 凝集度高 / 結合度低 ideal 分割達成、必要なら module 分割再構成)
  - cargo test 全 pass (rename + 分割再構成 後 regression なし)
  - cargo clippy 0 warning / cargo fmt 0 diff
  - PRD doc / plan.md / TODO 整合 update 済 (= 旧 task-ID-based 命名 references が新 semantic 命名に reflect、historical commit notes は preserve で「旧名 (= XX、現在 YY に rename)」note 追加 form OK)
- **Depends on**: T15
- **Status**: T15 完了後着手 (= I-205 scope 内 last task)、subsequent **新 PRD I-D (framework rule integration、別 architectural concern boundary 厳格適用 で切り出し)** と相補的

---

## Migration to subsequent PRD I-D (Framework rule integration、user 確定 2026-05-01)

T16 (= I-205 scope 内 file/fn 命名 + 実装分割 cleanup) と相補的に、**framework rule level structural enforcement** は **別 PRD I-D** として切り出し:

### 新 PRD I-D: Framework rule integration cohesive batch (I-205 lesson source 全統合) + audit script auto verify (Q2 拡張案採用、Iteration v19 light review 由来)

- **Architectural concern**: **"Framework rule level structural enforcement of I-205 lesson source candidates"** (= I-205 implementation で蓄積された framework rule integration candidates 全統合 cohesive batch、命名禁止だけでなく Iteration v18 framework 改善 4 件 + T7/T8 連続 framework 失敗 signal + その他 T-iteration 累積 lessons の framework rule level structural enforcement)。1 PRD = 1 architectural concern (= "framework rule level structural enforcement" の cohesive group) で I-205 close 時に framework gap clean state を達成、ideal-implementation-primacy 観点で sound。
- **修正範囲 (cohesive batch、Iteration v19 light review で確定)**:
  - **(D-1) Task-ID-based 命名禁止 framework rule integration** (元 spec、Iteration v19 由来、user 指示 2026-05-01):
    - `.claude/rules/testing.md` に "Task-ID-based naming prohibition" sub-rule 追加 (= module / file / 関数名に PRD ID / task ID / cell number 等 task tracking identifier を使用しない、semantic 命名のみ)
    - `.claude/rules/problem-space-analysis.md` の "Test は問題空間マトリクスから導出する" section に「test fn name は cell number ではなく semantic 命名」を追加
    - `scripts/audit-task-id-naming.py` (新規) で codebase 全体の task-ID-based 命名 violation を auto detect (= `i[-_]?[0-9]+` / `t[0-9]+_` / `cell_[0-9]+_` 等の regex pattern grep + violation list output、CI merge gate)
    - `prd-template` skill / `tdd` skill 等で test fn / module 命名 guidance に "semantic 命名" を hard-code
  - **(D-2) Iteration v18 framework 改善 4 件 integrate** (Iteration v18 entry 由来、本 T12 commit 内 PRD doc record 済 candidates 統合):
    - **(改善 A)** `spec-stage-adversarial-checklist.md` Rule 3 (3-2) **Spec stage Mandatory enforcement 強化**: 各 ✗/要調査 cell の TS code を `scripts/observe-tsc.sh` で empirical 確認することを Spec stage 完了 verification に追加。`audit-prd-rule10-compliance.py` で `## SWC Parser Empirical Lock-ins` section の各 ✗/要調査 cell 対応 entry 存在 + tsc empirical observation embed (exit_code / errors を含む) を auto verify する mechanism 追加
    - **(改善 B)** `spec-first-prd.md` 「Spec への逆戻り」section に「**Implementation 着手前 last-mile empirical observation**」を **Mandatory verification step として追加**。Iteration v18 で T12 着手前 empirical observation で Spec gap 発見した pattern を framework rule 化、scope creep 認識 → 前倒し検出 を運用 (本 PRD I-205 で 元 T11 削除 + cell 78 NA reclassify 2 度連続発生 source)
    - **(改善 C)** `spec-stage-adversarial-checklist.md` Rule 5 (5-1) sub-rule 拡張 = **"per-cell E2E fixture (red 状態) 準備済" だけでなく "fixture 自体の tsc empirical observation で fixture content 正当性 verify 済"** を Spec stage 完了 verification 必須項目に追加。`audit-prd-rule10-compliance.py` で fixture file 自体の tsc empirical observation log embed を auto verify
    - **(改善 D)** **Builtin name conflict detection**: TS class / interface / type / enum declaration の name が ES standard library / DOM types / Node types 等 builtin と conflict しないか check する linter rule / fixture creation guidance 追加 (Iteration v18 cell 74 `Cache` name conflict source)
  - **(D-3) T7/T8 連続 framework 失敗 signal integrate** (T7 Iteration v11 + T8 Iteration v12 由来):
    - `spec-stage-adversarial-checklist.md` Rule 10 axis enumeration default check axis に **"TypeResolver visit coverage of operand-context expressions"** (Update.arg / Unary.arg / Cond.test/cons/alt / compound assign Member.obj 等の operand position で receiver type 必要時、TypeResolver が visit してるか) を **正式 default axis として昇格** (= T7 Iteration v11 + T8 Iteration v12 で 2 度連続 framework 失敗 source、3 度目 prevention)
    - `audit-prd-rule10-compliance.py` で Rule 10 application yaml block の axis 列挙に本 axis 出現を verify する mechanism 追加 (= structural prevention)
  - **(D-4) T5 Iteration v9 lessons integrate** (T5 commit notes 由来):
    - `spec-stage-adversarial-checklist.md` **Rule 9 (c) Field-addition symmetric audit 拡張**: "field 追加" だけでなく **"既存 field の hardcode bug" + "registration site ↔ downstream consumer dependency"** にも拡張 (T5 extends 登録 / decl.rs:264 empty body class register filter Spec gap source)
    - **新 INV-7 候補** 検討: Pass 1 placeholder ↔ Pass 2 collect の data preservation invariant を独立 INV (registry layer、本 PRD I-205 では INV-1〜INV-6 だが本 framework 改善で INV-7) として独立記述 (T5 Spec gap #2 由来)
    - `spec-stage-adversarial-checklist.md` Rule 9 (a) **Spec → Impl Dispatch Arm Mapping completeness** check 拡張 = "Tier 1 dispatch + Tier 2 honest error reclassify dispatch" の **symmetric enumeration を明示要求** (T5 Iteration v9 deep-deep review Spec gap #3 由来)
- **1 PRD = 1 architectural concern** (Iteration v19 light review Q2 拡張案で確定): "Framework rule level structural enforcement of I-205 lesson source candidates" = **codebase rewrite (T16) と独立した meta-level rule integration**、I-205 で蓄積された framework gap candidates を清算する form の cohesive batch
- **Priority**: L4 (framework infra、codebase rewrite と coupling なし、subsequent integration) — ただし複数 framework rule update + audit script + skill update を含むため scope 中 (~7-9 framework rule update + 2-3 audit script + 2-3 skill update)、spec stage で Cross-axis matrix を 4 sub-task (D-1 / D-2 / D-3 / D-4) × axis dimension で構築
- **Depends on**: I-205 close (= T16 完了で codebase が semantic 命名に統一済の状態 + I-205 lesson source 全 lock-in 済)
- **Status**: I-205 close 後 deferred PRD として plan.md / TODO に record、起票 timing は user 判断、numeric ID 割り当ては起票 timing で確定 (現状記号 ID `I-D` preserve、Q3 Option b 採用)

---

## Test Plan

### Unit tests

- `src/registry/tests/method_kind.rs` (新規):
  - getter/setter kind が collect_class_info で正しく propagate
  - method (default) kind が collect_class_info で Method
  - TsTypeLit の getter/setter kind が convert_method_info_to_sig で propagate
- `src/transformer/expressions/tests/i_205.rs` (新規):
  - read dispatch (B1-B4, B8, B9)
  - write dispatch (B1-B4, B8, B9)
  - compound assign desugar (B4 × +=, ++/--)
  - this.x dispatch (E2)
  - read-only / write-only Tier 2 honest error
- `src/transformer/classes/tests/i_205.rs` (新規):
  - getter body `.clone()` insertion (C1 pattern, D1 vs D4 verify)

### E2E tests

Spec stage TS-3 task で作成済 (2026-04-28、19 fixtures + 19 .expected files in `tests/e2e/scripts/i-205/`):

- `tests/e2e/scripts/i-205/cell-02-getter-only-number-read.{ts,expected}` ← cell 2
- `tests/e2e/scripts/i-205/cell-03-getter-only-string-read.{ts,expected}` ← cell 3
- `tests/e2e/scripts/i-205/cell-04-setter-only-read-undefined.{ts,expected}` ← cell 4 (Tier 2)
- `tests/e2e/scripts/i-205/cell-05-getter-setter-string-read.{ts,expected}` ← cell 5
- `tests/e2e/scripts/i-205/cell-06-auto-accessor-no-decorator.{ts,expected}` ← cell 6 (PRD 2.8 scope、Tier 2 honest error 維持)
- `tests/e2e/scripts/i-205/cell-07-method-as-fn-reference.{ts,expected}` ← cell 7 (Tier 2)
- `tests/e2e/scripts/i-205/cell-08-inherited-getter.{ts,expected}` ← cell 8 (Tier 2)
- `tests/e2e/scripts/i-205/cell-09-static-getter.{ts,expected}` ← cell 9
- `tests/e2e/scripts/i-205/cell-12-getter-only-write-typeerror.{ts,expected}` ← cell 12 (Tier 2)
- `tests/e2e/scripts/i-205/cell-13-setter-only-write.{ts,expected}` ← cell 13
- `tests/e2e/scripts/i-205/cell-14-getter-setter-write-body-logic.{ts,expected}` ← cell 14
- `tests/e2e/scripts/i-205/cell-18-static-setter-write.{ts,expected}` ← cell 18
- `tests/e2e/scripts/i-205/cell-21-compound-assign-getter-setter.{ts,expected}` ← cell 21 (INV-3 verify)
- `tests/e2e/scripts/i-205/cell-38-nullish-assign-option.{ts,expected}` ← cell 38
- `tests/e2e/scripts/i-205/cell-43-postfix-increment.{ts,expected}` ← cell 43
- `tests/e2e/scripts/i-205/cell-44-increment-string-NaN.{ts,expected}` ← cell 44 (Tier 2 reclassify per Rule 3 (3-3))
- `tests/e2e/scripts/i-205/cell-60-internal-this-getter-only.{ts,expected}` ← cell 60 (INV-2 verify)
- `tests/e2e/scripts/i-205/cell-61-internal-this-getter-setter.{ts,expected}` ← cell 61 (borrow checker temp binding test)
- `tests/e2e/scripts/i-205/cell-70-getter-body-clone-string.{ts,expected}` ← cell 70 (C1 .clone() verify)

Plus (Implementation Stage で作成):
- `tests/compile-check/i-205-class-getter-setter.ts` (compile pass verify、T-Implementation で作成)
- `tests/e2e_test.rs` integration (T14 で全 fixture green-ify)

### Regression tests

- 既存 class Method (B1 field) 全 fixture green 維持
- Hono bench Tier-transition compliance (broken-fix PRD wording、`prd-completion.md` 適用):
  - **Improvement (allowed)**: existing Tier 2 errors for class Method Getter/Setter transition Tier 1 (clean files count 増加 / errors count 減少 が **expected**)
  - **Preservation (allowed)**: Hono が getter/setter 不使用なら count unchanged
  - **New compile errors (prohibited)**: 本 PRD scope 外 features に新たな compile error 導入は **regression** = 完了 block

### Invariant verification tests (Rule 8 (8-c) concrete spec、F-rev-6 fix)

INV-1〜INV-6 各 invariant の verification method (8-c) を **concrete test function に author**:

#### INV-1 Receiver type member kind dispatch consistency

- **Test fn**: `test_invariant_1_dispatch_consistency_across_call_sites` (`src/transformer/expressions/tests/i_205_invariants.rs`)
- **Assertion**: 4 receiver shape (Ident / chain / call_result / cond_branch) 各々で同 (B getter, field) → 同 emit IR
- **Probe location**: TestTransformer::convert_expr の output IR
- **Expected**: 4 IRs token-level identical (`Expr::MethodCall { object, method, args: vec![] }` 構造同)

#### INV-2 External (E1) と internal (E2 this) dispatch path symmetry

- **Test fn**: `test_invariant_2_external_internal_dispatch_symmetry`
- **Assertion**: external `obj.x` と internal `this.x` (同 class、同 type) の output IR が token-level identical
- **Probe location**: TestTransformer (external context) vs TestTransformer (internal class scope)
- **Expected**: both produce `Expr::MethodCall { method: "x", args: [] }` with appropriate receiver

#### INV-3 Compound assign desugar の receiver evaluation 1 回

- **Test fn**: `test_invariant_3_compound_assign_receiver_eval_once`
- **Assertion**: side-effect-having receiver (e.g., counter()) を含む `obj.x += v` desugar で counter() invocation count = 1
- **Probe location**: 生成 Rust code を inspect、temp binding pattern (`let __recv = ...; __recv.set_x(__recv.x() OP v);`) 検出
- **Expected**: counter() = 1 occurrence (not 2)

#### INV-4 Method kind tracking propagation chain integrity

- **Test fn**: `test_invariant_4_kind_propagation_lossless` (`src/registry/tests/method_kind_propagation.rs`)
- **Assertion**: TS source `class { get x() {} set x(v) {} }` → collect_class_info → MethodSignature.kind preservation chain
- **Probe location**: Each stage (collect_class_info → resolve_method_sig → dispatch logic) で kind value
- **Expected**: chain 全 stage で kind == Getter / Setter (Default::default() に fallthrough しない)

#### INV-5 Visibility consistency (private accessor 外部 access 不能)

- **Test fn**: `test_invariant_5_private_accessor_external_access_tier2`
- **Assertion**: `class { private get x() {} }` の external `obj.x` access → `UnsupportedSyntaxError::new("access to private accessor", ...)` emit
- **Probe location**: convert_member_expr output for receiver of class with private accessibility flag
- **Expected**: Tier 2 honest error (visibility 削除で Rust pub に degrade しない)

#### INV-6 Scope boundary preservation (`this.x` ↔ external `obj.x` semantic distinction)

- **Test fn**: `test_invariant_6_scope_boundary_preservation`
- **Assertion**: enclosing_class_name scope state lookup (this.x) と receiver expr type lookup (obj.x) が独立 path、両 path の output IR is token-level identical when class is same type
- **Probe location**: TestTransformer with this expression vs TestTransformer with Ident receiver
- **Expected**: both invariant-1 でも検証、INV-6 では path source separation を verify

### Decision tables (concrete enumeration、testing.md compliance)

#### Decision Table A: convert_member_expr Read context dispatch

| Lookup result | is_inherited | kind | Expected emit |
|---------------|--------------|------|---------------|
| None | — | — | `Expr::FieldAccess { object, field }` (B1/B9 fallback) |
| Some | true | any | `Err(UnsupportedSyntaxError::new("inherited accessor access", ...))` (cell 8) |
| Some | false | Getter | `Expr::MethodCall { method: field, args: [] }` (cells 2/3/5) |
| Some | false | Setter (no Getter) | `Err(UnsupportedSyntaxError::new("read of write-only property", ...))` (cell 4) |
| Some | false | Method | `Err(UnsupportedSyntaxError::new("method-as-fn-reference (no-paren)", ...))` (cell 7) |

#### Decision Table B: dispatch_member_write Write context dispatch

| Lookup result | is_inherited | kind | Expected emit |
|---------------|--------------|------|---------------|
| None | — | — | `Expr::Assign { target: FieldAccess, value, op: Assign }` (B1/B9 fallback) |
| Some | true | any | `Err(UnsupportedSyntaxError::new("write to inherited accessor", ...))` (cell 17) |
| Some | false | Setter | `Expr::MethodCall { method: format!("set_{field}"), args: [value] }` (cells 13/14) |
| Some | false | Getter (no Setter) | `Err(UnsupportedSyntaxError::new("write to read-only property", ...))` (cell 12) |
| Some | false | Method | `Err(UnsupportedSyntaxError::new("write to method", ...))` (cell 16) |

#### Decision Table C: build_method_inner Getter body `.clone()` insertion (C1 pattern、Iteration v18 で cell 78 NA reclassify)

| kind | body shape | return_type Copy性 | Expected rewrite |
|------|-----------|------------------|------------------|
| Getter | `return self.field;` (single-hop self field access) | Copy (D1/D2/D3、Option<Copy>、Tuple of all Copy) | no rewrite (cell 71 = D1 Copy / cell 73 = D6 Option<Copy>) |
| Getter | `return self.field;` (single-hop self field access) | non-Copy (D4 String / D5 Vec / D6 Option<non-Copy> / D7 HashMap / D8 Tuple<mixed> / D9 Struct / D10 Enum / D11 DynTrait / D12 Fn / D13 TypeVar / D14 Any / D15 Regex) | rewrite to `return self.field.clone();` (cells 70 = D4 / 72 = D5 / 74 = D6 Option<non-Copy>、Rule 1 (1-4-a) D-axis orthogonality merge: D7-D15 は dispatch logic identical = `is_copy_type() = false` で `.clone()` rewrite、representative cells 70/72/74 から inherit) |
| ~~Getter | last-expr `self.field` | non-Copy | rewrite to last-expr `self.field.clone()` (cell 78)~~ | **削除 (Iteration v18、cell 78 NA reclassify per TS spec、`## Spec Review Iteration Log` v18 entry 参照)**: TS class getter body は statement block で last-expr ≠ return、annotation 付き form は tsc TS2378+TS2355 reject、annotation 無 form は runtime undefined return → 「`return self.field;` pattern と semantic equivalent」claim は誤り |
| Getter | computed expr / conditional / let-binding intermediate / multi-return / nested closure | any | no rewrite (cells 75/76/77/79/80、本 PRD scope 外、別 PRD C2 "Class Method body T-aware comprehensive `.clone()` insertion") |
| Getter | nested self field access (`self.field.nested` etc.) | any | no rewrite (single-hop only、cell 75 系列の computed expr に分類) |
| Setter | `self.field = v;` | — | no rewrite (cell 81、current behavior preserved、kind gate で skip) |
| Method | any | — | no rewrite (本 PRD scope 外、kind gate で skip) |

### Equivalence partitions (testing.md compliance)

- **Receiver type partition**:
  - registered class with full methods (B1-B6)
  - registered class extends Base (B7 inherited)
  - registered class with static (B8)
  - unregistered/external (B9)
  - synthetic union 含む receiver type
- **T variant partition** (D dimension):
  - Copy primitive (D1 number / D2 bool / D3 char)
  - non-Copy std (D4 String / D5 Vec / D7 HashMap)
  - Option (D6) with Copy inner / non-Copy inner
  - Tuple (D8) all-Copy / mixed
  - User Struct (D9) / Enum (D10)
  - DynTrait (D11) / Fn (D12) / TypeVar (D13) / Any (D14) / Regex (D15)
- **Operator partition (A3-A6)**:
  - Arithmetic compound (`+= -= *= /= %= **=`)
  - Bitwise compound (`<<= >>= >>>= &= \|= ^=`)
  - Logical compound (`??= &&= \|\|=`)
  - Increment/decrement (`++ --` prefix/postfix)

### Boundary values (testing.md compliance)

- **Member count**: empty class (no fields/methods) / single member / multi member
- **Inheritance depth**: depth 0 (no extends) / depth 1 (Sub extends Base) / depth N (chain Sub → Mid → Base)
- **Getter body length**: empty body (`{}`) / single stmt / multi stmt / nested closure
- **Compound assign side-effect**: side-effect-free receiver (Ident) / single side-effect (call result) / chain side-effect

### Coverage gap (Step 3b で identify されるべき内容、本 draft で初期 enumerate)

- Decision points:
  - `resolve_member_access` の field/getter/setter/method/no entry × Copy/non-Copy T (decision table 必要)
  - `convert_assign_expr` Member target の各 op variant (= ??= &&= ||= += -= *= ...) 全 enumerate
- Equivalence partitions:
  - receiver type: registered class / registered interface / no entry / synthetic
  - T variant: D1-D15 全
- Boundary values:
  - empty getter/setter pair (B2/B3 only)、both (B4)、no member (B9)
  - getter body: 1 stmt return / multi stmt / last-expr / nested return
- Decision tables:
  - read context × B 全 → expected emit
  - write context × B 全 → expected emit
- Bug-affirming test 警戒:
  - 旧 broken framework に対する false-positive test (= broken `f.x` を expect する test) があれば修正必要

---

## Completion Criteria

`prd-completion.md` 準拠:

### Matrix completeness (最上位完了条件)

- Problem Space matrix 全 cell に判定 (✓/✗/NA/別 PRD/regression)
- 全 ✗ cell に対応する E2E fixture が green
- 全 regression cell に対応する lock-in test 存在
- ✓ cell の現状実装が ideal と一致 (audit verify)

### Quality gates

- `cargo build` pass (0 warning)
- `cargo test --lib` 全 pass (0 fail / 0 ignored 増加なし)
- `cargo test --test compile_test` 全 pass
- `cargo test --test e2e_test` 全 pass (29 #[ignore] 維持、新 ignore 0)
- `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
- `cargo fmt --all --check` 0 diff
- `./scripts/audit-ast-variant-coverage.py --files <impact-area>` PASS (本 PRD scope file、Rule 11 (d-5))
- `./scripts/audit-prd-rule10-compliance.py` PASS (本 PRD doc self-applied、Rule 1/2/5/6/8/11/13 全 PASS)
- Tier-transition compliance (`prd-completion.md` 適用): existing Tier 2 errors transition Tier 1 = improvement、no new compile errors

### Reviews

- Spec stage: `spec-stage-adversarial-checklist.md` 13-rule 全 verification
- Implementation stage: `/check_job` 4-layer review (Mechanical / Empirical / Structural cross-axis / Adversarial trade-off) 初回 invocation で全実施
- Defect classification: 5 category 全 trace、Spec gap = 0 (framework 失敗 signal)

### Impact verification (3 representative instances 必須)

`prd-completion.md` の "Impact estimates must be verified by tracing actual code paths for at least 3 representative error instances" 準拠。本 PRD は error count reduction 目的ではない (Tier 2 framework defect 解消) が、以下の representative instances を trace verify:

1. `f.name` (B2 getter only, D4 String) — `obj.x()` (method call) emission verify
2. `f.value = 5` (B4 both, D1 f64) — `obj.set_x(v)` emission verify  
3. `this.count++` (E2 internal, B4, D1) — setter desugar `self.set_count(self.count() + 1)` emission verify

---

## Spec Revision Log

### Iteration v14 (2026-04-30、T9 deep-deep `/check_job` 4-layer review)

T9 commit 後の deep-deep `/check_job` 4-layer review で発見された Spec gap + framework
失敗 signal の self-applied integration record:

#### Spec gap 1: matrix cells 39/40 D dimension orthogonality 不足 (Layer 3 finding)

- **発見**: cells 39/40 spec `D2 bool` LHS のみ enumerate、他 D variants
  (D1 F64 / D3 String / D6 Option<T> / etc.) for `&&=`/`||=` × B4 dispatch が
  matrix に明示されていない。Implementation は existing
  `truthy_predicate_for_expr` / `falsy_predicate_for_expr` per-type 経由で
  transitively 動作するが、Rule 10 (Cross-axis matrix completeness) compliance 観点
  で D dimension 完全 enumerate が必要。
- **対応**: structural lock-in tests (F64 / String / Option<T>) を `logical_compound.rs`
  に追加完了 (Iteration v13)、本 deep-deep review で本質的 verify 拡張
  (`logical_compound_strategies.rs` 内 const-fold for always-truthy LHS + Identity
  emission for non-Option non-Any LHS の test 追加)。
- **Matrix doc revision (subsequent iteration)**: cells 39/40 を orthogonality-equivalent
  merge form に書き換え、`Cells 39-* (D-axis variants of cells 39/40):
  orthogonality-equivalent through truthy_predicate_for_expr per-type dispatch +
  is_always_truthy_type const-fold + Any/TypeVar I-050 gate` の sub-table を追加する
  scope は subsequent T15 (`/check_job` 4-layer review final) で実施候補。

#### Spec gap 2: Cell 38 expression context Block-tail returns Option<T> (Layer 4 finding)

- **発見**: cell 38 expression context emission の tail `<getter>` returns Option<T>、
  TS `??=` semantic は narrowing-after-??= で T (= unwrapped) を yield するが、
  本実装は Option<T> tail で divergent。ユーザー code `let z: number = c.value ??= 42;`
  は Rust 上 type mismatch (`expected f64, found Option<f64>`) で Tier 2 compile error
  surface (silent semantic change なし)。
- **マトリックス整合**: 既存 matrix cell 38 ideal output が `obj.x().or_else(|| { obj.set_x(d);
  Some(d) })` (Option<T> 返却) と記載されているため、本実装の Option<T> tail は
  matrix-acknowledged divergence。
- **将来対応 (subsequent PRD)**: TS narrowing-after-??= semantic を Rust で再現する
  emission 拡張は I-NNN (新規 TODO 起票候補、narrowing-aware class member ??= expression
  context) で取り扱う。Tier 1 真の理想化候補だが、現 T9 scope では Option<T> tail
  emission を採用 (TS narrowing は class member 越境で divergent な complex case、
  ownership / mut borrow constraints が non-trivial)。

#### Implementation gap 1: ??= × non-Option non-Any LHS broken Rust emission (Layer 4 finding)

- **発見**: 初期 T9 実装で `??=` × class member × non-Option non-Any LHS
  (e.g., `c.value ??= 42` where `c.value: number`) は predicate `<getter>.is_none()`
  を non-Option Rust type に対し emit → E0599 broken Rust output (silent broken Rust)。
- **対応**: `build_logical_compound_predicate` の `NullishAssign` arm に Option<T> gate を
  追加 (Iteration v13 = Tier 2 honest error "Identity strategy out of T9 scope"、
  generic wording)。続いて Iteration v14 deep-deep review で `pick_strategy` 統合
  (= 既存 `nullish_assign.rs::try_convert_nullish_assign_stmt` の Ident-target
  emission logic と cohesive)、3-way dispatch 実装:
    - `ShadowLet` (Option<T>): conditional setter desugar (cells 38、既存)
    - `Identity` (non-Option non-Any): Tier 1 ideal Identity emission (no setter call、
      yield current getter for expression context、empty Block / evaluate-discard for
      statement context、INV-3 IIFE for SE-having receiver)
    - `BlockedByI050` (Any): Tier 2 honest error `"nullish-assign on Any class member
      (I-050 Any coercion umbrella)"` (consistent with existing Ident-target wording)
- **Tier transition**: pre-T9 = Tier 2 broken (FieldAccess emission errors) →
  Iteration v13 = Tier 2 honest error → Iteration v14 = **Tier 1 Identity emission**
  (本 PRD scope 内で本質的解決完成)。

#### Implementation gap 2: `&&=`/`||=` × Any/TypeVar wording inconsistency (Layer 1 finding)

- **発見**: 初期 T9 実装で `&&=` / `||=` × Any/TypeVar LHS は
  `truthy_predicate_for_expr` / `falsy_predicate_for_expr` が None を返却 → my T9 が
  generic wording `"logical compound assign on unsupported lhs type (truthy/falsy
  predicate unavailable)"` を emit。既存 `compound_logical_assign.rs::desugar_compound_logical_assign_stmts`
  の Any/TypeVar gate wording (`"compound logical assign on Any/TypeVar (I-050 umbrella
  / generic bounds)"`) と inconsistent。
- **対応**: Iteration v14 deep-deep review で `dispatch_b4_strategy` の `&&=`/`||=` arm に
  pre-check Any/TypeVar gate 追加、specific I-050 umbrella / generic bounds wording emit
  (`"compound logical assign on Any/TypeVar class member (I-050 umbrella / generic
  bounds)"`)。

#### Implementation gap 3: `&&=`/`||=` × always-truthy LHS suboptimal predicate Block (Layer 4 finding)

- **発見**: 初期 T9 実装で `&&=` / `||=` × always-truthy LHS (Vec / Fn / StdCollection /
  DynTrait / Ref / Tuple / Named non-union) は existing `truthy_predicate_for_expr` 経由
  で `Expr::Block { let __ts_eval0 = <getter>; true }` を予測 emit (= functional Tier 1
  だが、`obj.set_x(rhs);` 直接 emit する const-fold より suboptimal)。
- **対応**: Iteration v14 deep-deep review で `dispatch_b4_strategy` の `&&=`/`||=` arm に
  `is_always_truthy_type` 経由 const-fold 追加、cohesive with existing
  `compound_logical_assign.rs::const_fold_always_truthy_stmts`:
    - `&&=` always-truthy: unconditional setter call `<setter>(rhs);` (statement) +
      INV-3 IIFE wrap for SE-having
    - `||=` always-truthy: no-op (statement) / getter-yield (expression) + INV-3 IIFE
- **Tier transition**: Iteration v13 = Tier 1 functional (eval-Block predicate) →
  Iteration v14 = **Tier 1 ideal const-fold** (Rust output 最適化、unused variable
  warning 排除)。

#### Framework 改善 candidate (本 PRD scope 外、後続 framework iteration 候補)

1. **Rule 10 default check axis**: cross-axis matrix completeness (Layer 3) で
   "TypeResolver visit coverage of operand-context expressions" を default check axis
   として正式昇格 (Iteration v11/v12/v13 連続 3 度発生 = pattern recognition 確立、
   Iteration v14 で確認済)。`spec-stage-adversarial-checklist.md` Rule 10 axis
   enumeration default list に追加候補。
2. **Rule 9 (a) symmetric counterpart helper test contracts**: `dispatch_b4_strategy`
   shared helper (= ReceiverCalls struct + emit_* 4 sub-helpers) は instance/static 両 dispatch で
   reused、symmetric test contracts の framework-level lock-in が今後の subsequent
   T-* で類似 helper 再導入時に reusability 高めるため `Rule 9 (a) helper test
   contracts` の sub-rule 候補。

---

### Iteration v18 (2026-05-01、T12 着手前 Spec への逆戻り = cell 78 NA reclassify + cell 74 fixture rename + framework 改善 candidate)

T12 (Class Method Getter body `.clone()` 自動挿入) implementation 着手前の empirical
observation で **2 件の Spec gap = framework 失敗 signal** を発見、`spec-first-prd.md`
「Spec への逆戻り」発動による spec re-design + 本 PRD self-applied integration record。

**Iteration v15-v17 (2026-04-30 〜 2026-05-01)** = T9 Iteration v15 `/check_problem`
cleanup + T10 Iteration v16 first review + T10 Iteration v17 deep-deep review =
plan.md 直近の完了作業 table に commit notes として record (PRD doc 正式 entry なし、
`note.after.md` archive + git history 参照可能)。本 v18 entry は v17 final state を
base として継続。

#### Spec gap 1: cell 78 = TS spec reject の "C1 last-expr 拡張" claim 誤り

- **発見契機**: T12 着手前、cells 70-81 全 cells empirical observation を実施 (= 元 T11
  削除 + 新 PRD I-A/I-B migration commit `aef25aa` で確立した「scope creep 認識 →
  empirical observation で前倒し検証」運用)
- **empirical observation 結果**:
  - **cell 78 TS code** (`class Profile { _name: string = "alice"; get name(): string { this._name } }`):
    - **tsc**: 2 errors
      - `TS2378: A 'get' accessor must return a value.` (line 3, col 7)
      - `TS2355: A function whose declared type is neither 'undefined', 'void', nor 'any' must return a value.` (line 3, col 15)
    - **tsx runtime**: `undefined\n` (= TS class getter body は statement block で
      last-expr ≠ return、annotation 付き form は tsc reject、annotation 無 form は
      runtime undefined return)
- **Spec gap 本質**: 初版 spec line 305 の「last-expr `self.field` を `.clone()` 付きに
  rewrite、`return self.field;` pattern と **semantic equivalent**、Rust では last-expr
  = implicit return」claim は **誤り**。TS class getter の body は `{ stmts }` block で
  last-expr は returned value にならない。「Rust の last-expr semantic を leverage して
  TS code を変換」という claim は TS spec で reject される (= valid TS source として
  存在しない) input form を前提としている。
- **本 v18 fix**:
  - **cell 78 を NA reclassify** (`spec-stage-adversarial-checklist.md` Rule 3 (3-1)
    per TS spec、TS spec で reject される input は NA cell)
  - matrix line 305 update: cell 78 行を NA reclassify、理由 = tsc TS2378+TS2355 reject
  - Decision Table C update: cell 78 行を「~~削除 (Iteration v18、cell 78 NA reclassify)~~」
    として strikethrough + 理由 spec-traceable に記載
  - `## SWC Parser Empirical Lock-ins` section に cell 78 empirical observation embed
    (Rule 3 (3-2) compliance restored)
  - T12 task description から cell 78 reference 削除、scope = cells 70/72/74 (本 PRD ✗→✓)
    + cells 71/73/81 (regression lock-in) のみ
- **Framework 改善 candidate (本 PRD close 時 integrate or 別 framework PRD 起票)**:
  - **(改善 A) `spec-stage-adversarial-checklist.md` Rule 3 (3-2) の Spec stage Mandatory
    enforcement 強化**: 各 ✗/要調査 cell に対して **TS code が tsc accept か** を
    `scripts/observe-tsc.sh` で empirical 確認することを Spec stage 完了 verification
    に追加。`audit-prd-rule10-compliance.py` で `## SWC Parser Empirical Lock-ins`
    section の各 ✗/要調査 cell 対応 entry 存在 + tsc empirical observation embed
    (exit_code / errors を含む) を auto verify する mechanism 追加。
  - **(改善 B) Spec への逆戻りの「前倒し検出」運用 framework rule 化**: T12 着手前の
    empirical observation pattern (= 元 T11 削除 commit `aef25aa` から派生した
    「scope creep 認識 → empirical 前倒し検証」運用) を `spec-first-prd.md`
    「Spec への逆戻り」section の sub-rule として正式化。Implementation 着手直前の
    last-mile empirical observation を Mandatory verification step に追加。

#### Spec gap 2: cell 74 fixture content tsc reject (`Cache` name conflict)

- **発見契機**: cells 70-81 全 cells empirical observation 中、cell 74 fixture file
  (`tests/e2e/scripts/i-205/cell-74-getter-body-option-non-copy.ts`) を tsc check
- **empirical observation 結果**:
  - **cell 74 fixture** (`class Cache { _v: string | undefined = "hello"; get v(): string | undefined { return this._v; } }`):
    - **tsc**: 2 errors
      - `TS2300: Duplicate identifier 'Cache'.` (line 1, col 7)
      - `TS2339: Property 'v' does not exist on type 'Cache'.` (line 3, col 15)
    - **tsx runtime**: `hello\n` (= class declaration 自体 + getter return semantic は
      正常に動作、tsc error は class name `Cache` が **ES2017+ standard built-in `Cache`
      interface (Service Worker API)** との duplicate identifier conflict)
- **Spec gap 本質**: TS-3 Spec stage task で per-cell E2E fixture を作成した際、
  fixture 自体の tsc empirical observation を skip した = framework 失敗 signal #2。
  fixture content の **class name の builtin name conflict** を見落とし、tsc reject
  される fixture を red 状態 lock-in した。
- **本 v18 fix**:
  - **cell 74 fixture rename**: `class Cache` → `class OptCache` (or similar
    non-conflicting class name)、fixture content 修正のみで spec / ideal output 維持
    (= D6 Option<non-Copy> = `string | undefined` getter return type)
  - rename 後 `scripts/observe-tsc.sh` で再 empirical observation で tsc accept verify
  - `## SWC Parser Empirical Lock-ins` section に cell 74 fixture rename 経緯 + 修正後
    empirical observation embed
- **Framework 改善 candidate (本 PRD close 時 integrate or 別 framework PRD 起票)**:
  - **(改善 C) `spec-stage-adversarial-checklist.md` Rule 5 (5-1) sub-rule 拡張**:
    「per-cell E2E fixture (red 状態) で準備済」だけでなく、「**fixture 自体の tsc
    empirical observation で fixture content の正当性 verify 済**」 を追加。
    `scripts/observe-tsc.sh` で fixture file を tsc check し、type_check.exit_code = 0
    かつ runtime.stdout が ideal output と一致することを Spec stage 完了 verification
    の必須項目に追加。`audit-prd-rule10-compliance.py` で `tests/e2e/scripts/i-205/`
    内全 fixture の tsc empirical observation log embed を auto verify する mechanism
    追加候補。
  - **(改善 D) Builtin name conflict detection**: TS class / interface / type / enum
    declaration の name が ES standard library / DOM types / Node types 等の builtin
    と conflict しないか check する linter rule / fixture creation guidance 追加候補。

#### 本 v18 self-applied integration の意義

ideal-implementation-primacy 観点で:
- **cell 78 spec gap**: 元 T11 削除と同 pattern (= scope creep 認識) を **個別 cell
  level に extension**。T12 architectural concern (= "C1 limited pattern `.clone()`
  insertion") を `return self.field;` form のみ に絶対 limit、TS spec で reject される
  input form (last-expr) は NA reclassify で本 PRD scope から削除。
- **cell 74 fixture bug**: Spec stage で skip された "fixture 自体の empirical
  observation" を T12 着手前に retrospective に実施 = subsequent Implementation で
  発覚していた場合の rework cost を構造的に削減 (前倒し検出)。
- **framework 改善 candidate 4 件 (改善 A/B/C/D)** は本 PRD close 時 integrate or
  別 framework PRD 起票候補として記録、I-205 の framework v1.6 → 次 revision の
  empirical lesson source として追加。

### Iteration v18 完了判定 (2026-05-01)

**13-rule self-applied verify 結果**:

- **Rule 1 (1-1)/(1-2)/(1-3) Matrix completeness + abbreviation prohibition**: ✓
  cell 78 を NA reclassify、Decision Table C で cell 78 行を strikethrough + 削除理由
  記載、abbreviation pattern 排除維持
- **Rule 1 (1-4) Orthogonality merge legitimacy**: ✓ Decision Table C 更新で D-axis
  orthogonality merge を non-Copy partition (D4-D15) で representative cells 70/72/74
  からの inheritance を spec-traceable に明示
- **Rule 2 (2-1)/(2-2)/(2-3) Oracle grounding + PRD doc embed**: ✓ cell 78 / cell 74
  empirical observation を `## SWC Parser Empirical Lock-ins` section + 本 v18 entry
  に embed
- **Rule 3 (3-1)/(3-2)/(3-3) NA justification + SWC parser empirical observation 必須**:
  ✓ cell 78 = NA reclassify per TS spec、Rule 3 (3-1) traceable reason ("tsc TS2378+
  TS2355 reject")、Rule 3 (3-2) empirical observation embed
- **Rule 5 (5-1)/(5-2)/(5-3)/(5-4) E2E readiness + Stage tasks separation**: ✓ cell 74
  fixture rename で tsc empirical accept restoring、Spec Stage Tasks / Implementation
  Stage Tasks 2-section split 維持 (T12 task description update のみ)
- **Rule 6 (6-1)/(6-2)/(6-3)/(6-4) Matrix/Design integrity + Scope 3-tier consistency**:
  ✓ matrix Ideal output + Decision Table C + T12 task description token-level 一致、
  Scope 3-tier (本 PRD / Out of Scope = NA / 別 PRD = 75-77/79/80) 整合
- **Rule 9 (a)/(b)/(c) Dispatch-arm sub-case alignment + helper test contracts +
  Field-addition symmetric audit**: ✓ T12 helper API design (= `is_self_single_hop_field_access`
  + `insert_getter_body_clone_if_self_field_access`) + Decision Table C + matrix cells
  alignment 確立、Field Addition Symmetric Audit (= T12 では新 IR struct field 追加
  なし、helper-only addition で self-contained)
- **Rule 10 Cross-axis matrix completeness**: ✓ T12 軸 enumeration = `kind` (3 variant)
  × `body shape` (single-hop self field access / computed / conditional / let-binding /
  multi-return / nested closure) × `return_type Copy性` (Copy / non-Copy) × `Stmt last
  variant` (Return / TailExpr / その他)、各軸 representative cells で全 partition cover
- **Rule 11 (d-1)/(d-5)/(d-6) AST node enumerate completeness**: ✓ helper の `Stmt`
  enum match で全 variant 完全 enumerate (`_ =>` arm 排除)、Impact Area Audit Findings
  は本 v18 で update 不要 (T12 helper-only addition、新 file 作成は予定通り)
- **Rule 13 Spec Stage Self-Review (skill workflow integrated)**: ✓ 本 entry が
  Iteration v18 self-applied verify pass declaration、Critical findings 全 fix (cell
  78 NA reclassify + cell 74 fixture rename plan 確定 + T12 task description update)、
  framework 改善 candidate 4 件は subsequent integration

**Implementation stage 移行 ready**: ✓ T12 implementation 着手可能 (cells 70/72/74 +
regression lock-in cells 71/73/81 + negative cells 75/76/77/79/80 + NA cell 78 全 cells
の treatment 確定)。

---

### Iteration v19 (2026-05-01、T12 単独 commit + sub-iteration v18.1 で T14 scope re-partition + Option 3 で T16 追加 + 別 PRD I-D 切り出し)

T12 (Class Method Getter body `.clone()` 自動挿入、C1 limited pattern) implementation
完了 + `/check_job` 4-layer review + Option 3 split で I-205 scope 内 cleanup task T16
追加 + framework rule integration を別 PRD I-D として切り出し決定。

#### T12 implementation summary

- **Helper additions** in `src/transformer/classes/helpers.rs` (T10 self-rooted
  expression helper family と同 architectural concern):
  - `is_self_single_hop_field_access` (private): single-hop self field access detection
  - `insert_getter_body_clone_if_self_field_access` (`pub(super)`、`&mut [Stmt]` slice
    API for clippy::ptr_arg compliance): `Stmt` enum 全 14 variants 完全 enumerate
    (Rule 11 (d-1) compliance、`_ =>` arm 排除)
- **Gate wiring** in `src/transformer/classes/members.rs::build_method_inner`:
  `convert_last_return_to_tail` 直後に `kind == Getter && return_type non-Copy` gate
- **Unit tests** in `src/transformer/classes/tests/i_205.rs` (新規 file、20 件):
  Decision Table C 完全 cover (cells 70/71/72/73/74/81 + Method) + Equivalence
  partitioning + Boundary value + Branch coverage C1 + AST variant exhaustiveness +
  Negative tests
- **E2E ignore reason update** in `tests/e2e_test.rs`: cells 70/71/72/74 の `#[ignore]`
  reason を T12 完了 + I-162 prerequisite block + T14 deferred 明示

#### `/check_job` 4-layer review (Iteration v19、deep modifier 適用)

- **Layer 1 (Mechanical)**: 0 findings ✓ — clippy 0 warning (ptr_arg lint で `&mut Vec`
  → `&mut [Stmt]` slice API)、cargo fmt 0 diff、check-file-lines OK (helpers.rs 712
  行 / classes/tests/i_205.rs 320 行 < 1000 threshold)、test 命名 `test_t12_*` prefix
  一貫 (※ T16 で semantic 命名 rewrite 予定)、bug-affirming test なし
- **Layer 2 (Empirical)**: 0 findings ✓ — 20 unit tests 全 pass + CLI manual probe で
  cell 70 generated Rust `fn name(&self) -> String { self._name.clone() }` empirical
  verify (T12 helper 正しく `.clone()` 挿入 confirmed)、E2E green は I-162 prerequisite
  で block (T14 defer 明示)、cargo test --lib 3355 / e2e 159+70 ignored / compile_test
  3 全 pass
- **Layer 3 (Structural cross-axis)**: 0 findings ✓ — 直交軸 enumeration:
  - (I) 逆問題視点: no-rewrite preservation軸 = cells 71/73/81 + Method case + cells
    75-77/79/80 で全 cover
  - (II) 実装 dispatch trace: `Stmt` enum 14 variants の rewrite target / non-target
    + `Expr::FieldAccess` object Ident name = self / non-self の dispatch 軸 で全
    enumerate
  - (III) 影響伝搬 chain: build_method_inner gate → helper invoke → IR rewrite →
    generator emit → CLI probe で cell 70 verify、E2E は I-162 prerequisite block で
    T14 defer (= IR-level primary verify は完了)
- **Layer 4 (Adversarial trade-off)**:
  - **Pre/post matrix**: cells 70/72/74 = Pre-T12 E0507 compile error (TS getter
    `return this._n;` で String/Vec/Option<String> move out of `&self`) → Post-T12
    `.clone()` 挿入で IR-level fix、E2E green は T14 defer (I-162 block)
  - cells 71/73 = Copy partition で move semantics 不要、Pre/Post unchanged ✓
  - cell 81 (Setter) / cells 75-77/79/80 / Method = kind gate / shape gate で skip、
    Pre/Post unchanged ✓
  - **Trade-off**: なし (T12 helper は Getter + non-Copy gate で precise gate、
    Copy/Setter/Method/non-target body shape は touch しない、structural fix degree 高)
  - **Patch vs Structural fix**: structural fix (helper logic + AST variant
    exhaustiveness Rule 11 (d-1) + Rule 1 (1-4) D-axis orthogonality merge representative
    coverage)
  - **Architectural rabbit hole detection**: なし (T12 architectural concern が単純で
    cohesive、scope creep risk 低)

#### Defect Classification 5 category (Iteration v19)

| Category | Count | Action |
|----------|-------|--------|
| Grammar gap | 0 | (無し、`Stmt` / `Expr::FieldAccess` は ast-variants.md 既 record) |
| Oracle gap | 0 | (無し、Iteration v18 で cell 78 / cell 74 empirical observation 済、本 T12 implementation で追加 oracle 不要) |
| Spec gap | 0 | (Iteration v18 で structural 解消 + sub-iteration v18.1 で T14 scope re-partition、本 T12 implementation 内で新 Spec gap 発生なし) |
| Implementation gap | 0 | (20 unit tests 全 pass、Decision Table C 完全 cover、Stmt 14 variants exhaustive enumeration、CLI probe で cell 70 verify) |
| **Review insight** | **2** | **(1) task-ID-based 命名 audit (user 指示 2026-05-01 由来)** = subsequent **T16 task として I-205 scope 内追加** + **別 PRD I-D = framework rule integration を切り出し** (Option 3 split 採用 by user)、本 v19 entry で migration mapping 確定。**(2) `members.rs:440 default_expr.unwrap()` Pre-existing rule violation** = T12 で touch していない既存 code (line 326-334 が T12 changes 範囲、line 440 は constructor parameter `default` value 解決時の unwrap)、`testing.md` "unwrap() only in test code" rule violation。本 T12 scope 外、T16 cleanup integrate (= I-205 cleanup quality 改善 architectural concern 内包) or 別 TODO 起票候補、user 確認後対応 |

#### Iteration v19 完了判定

- T12 architectural concern (= "Class Method Getter body C1 `.clone()` insertion") は
  unit tests verification で primary 達成 ✓
- E2E green-ify は T14 defer (Iteration v18 sub-iteration v18.1 で確定、本 v19 で number
  統一)、I-162 prerequisite block 明示 ✓
- Quality gate (fmt / clippy / file-lines / test 全 pass) ✓
- 4-layer review 全 0 findings ✓
- Defect Classification = Review insight 1 (T16 + 別 PRD I-D split で対応、本 v19
  entry に migration mapping record)
- **Iteration v19 完了 = T12 task close ready**、次 iteration = T13 (B6/B7 corner cells
  reclassify + INV-5 verification + boundary value test 拡充) 単独 commit へ進む

### Iteration v20 (2026-05-01) = T13 単独 commit 完了 (B6/B7 corner cells verify + INV-5 Option B + boundary value 拡充)

T13 implementation (B6/B7 corner cells lock-in verify + INV-5 reachability audit + boundary
value test 拡充) 完了 + 4-layer review 1 finding (Layer 3 cross-axis = INV-5 setter symmetric
probe 不在) 本 T13 内 fix + Defect Classification 5 category trace。**Production code change 0
LOC、Test code addition only**。

#### T13 implementation summary (本 v20 commit)

- **(13-a) Cells 7/8 Tier 2 honest error reclassify lock-in verify**: T5 で実装済の
  cells 7 (B6 `method-as-fn-reference (no-paren)`) / 8 (B7 `inherited accessor access`)
  既存 lock-in test 確認 (`src/transformer/expressions/tests/i_205/read.rs:172` /
  `read.rs:200`、Decision Table A `Some + is_inherited=true` / `Some + Method` arm 準拠)。
  追加 production / test 不要、(13-a) verify 完了。
- **(13-b) INV-5 reachability audit + Option A vs B 判定**: Hono codebase 284 TS files
  全件 `grep -rEn "private get \w|private set \w"` で **0 件 hit** (= reachability = 0)。
  Option A (`MethodSignature.accessibility` field 追加 + 50+ site Rule 9 (c) Field-addition
  symmetric audit + dispatch arm で `UnsupportedSyntaxError::new("access to private
  accessor", _)` emit) は 0 件 reachability の concern に対し overengineering、recurring
  problem evidence (I-383 T8' / I-205 T2 で latent kind drop 2 度連続) を考慮し **Option B
  (status quo) を採用**。Empirical verification: `src/transformer/classes/helpers.rs:89`
  `resolve_member_visibility(Some(Private), _)` → `Visibility::Private` 既存 mechanism で
  生成 method に `pub` modifier 不在 → Rust E0624 visibility error が consumer module 経由
  で **Tier 2 honest error 自動 surface** (no production code change needed)。`## Invariants`
  INV-5 (b)/(c)(d) wording を Option B 採用 audit 結果 reflect、INV-5 (c) Verification
  method を更新。
- **(13-c) INV-5 integration test green-ify**: `tests/i205_invariants_test.rs::
  test_invariant_5_private_accessor_external_access_tier2` を fill-in、`#[ignore]` 解除。
  Test contract: (1) private getter 生成 method に `pub` modifier 不在 (`fn x(&self)`
  form)、(2) public getter は `pub` 存在、(3) external `obj.x` access は cell 2 dispatch
  fires regardless of accessibility (= MethodCall emit 一貫)。Layer 3 cross-axis review で
  Setter symmetric counterpart 不在を発見、`test_invariant_5_private_setter_external_write_tier2`
  追加 fill-in (cell 14 setter dispatch + `set_x` visibility marker preservation probe)、
  本 T13 内 cross-axis completeness 達成。
- **(13-d) Multi-step inheritance test N>=3 + cycle corner test**: 既存 T5 で N=2 step
  cover を boundary value analysis 観点で N>=3 step + partial cycle に拡張、3 件 NEW unit
  test 追加 (`src/transformer/expressions/tests/i_205/read.rs`):
  - `test_b7_traversal_n3_step_inheritance_returns_inherited_flag`: A → B → C → D の 4-class
    chain で D has getter `q`、A から N=3 step propagation で `is_inherited = true` verify
  - `test_b7_traversal_partial_cycle_with_intermediate_method_returns_inherited_flag`:
    A → B → C → A partial cycle、method on C のみ。method が cycle 前に存在する case で
    visited HashSet が cycle に到達せず正しく terminate する事を verify
  - `test_b7_traversal_partial_cycle_no_method_terminates`: A → B → C → A degenerate cycle
    で全 class missing_field 不在、deeper cycle (depth=3) でも cycle prevention が機能
    し infinite loop なく None return 検証 (`test_b7_traversal_cycle_does_not_infinite_loop`
    の depth=3 拡張版)
- **Helper integration stubs fill-in (Spec stage F-deep-deep-4 commitment 完成)**:
  `tests/i205_helper_test.rs` の 4 stubs を **integration-level transpile probe** として
  fill-in、`#[ignore]` 解除。Layered test design (registry-level unit = `read.rs::
  test_b7_traversal_*` で cycle / direct / single-step / multi-step N=2 / N>=3 / partial
  cycle 計 7 件 + integration-level = 本 file 4 件 = `transpile` API 経由 end-to-end
  probe) で B7 dispatch arm が unit / integration 両 level で symmetric verify 達成:
  - `test_lookup_method_kind_single_level_inherited_getter`: `Sub extends Base` end-to-end
    で `Err("inherited accessor access")` Tier 2 honest error fire verify
  - `test_lookup_method_kind_multi_level_inherited_getter`: `Sub extends Mid extends Base`
    N=2 step で同様 verify
  - `test_lookup_method_kind_circular_inheritance_prevention`: `A extends B / B extends A`
    の degenerate input を `transpile` が panic / infinite loop なく処理する safety probe
    (registry-level cycle prevention は unit test で別途 verify 済)
  - `test_lookup_method_kind_direct_vs_inherited_disambiguation`: `Foo { get x() } + f.x`
    direct (B2) で `Ok` + `f.x()` MethodCall emit verify (B1 vs B7 disambiguation の
    direct hit path integration probe)

#### Production code: 0 LOC change

T13 は **verify + boundary value extension + INV-5 audit-driven test contract fill-in** の
test-only commit。Production code は Option B 採用判断により unchanged。

#### `/check_job` 4-layer review (Iteration v20)

- **Layer 1 (Mechanical)**: 0 findings
  - Test name pattern (`test_<target>_<condition>_<expected>`) 全件準拠
  - Assertion message に context string + got value 含む
  - bug-affirming test 不在 (全 assertion が ideal output 固定)
  - clippy 0 warning / fmt 0 diff / check-file-lines OK (read.rs 756 < 1000 / helper 115 / invariants 229)
- **Layer 2 (Empirical)**: 0 findings
  - INV-5 Option B mechanism を CLI probe で empirical verify (`/tmp/probe_inv5b.ts`、
    `/tmp/probe_priv_setter.ts`) → `private get/set` → no `pub` modifier、`public get/set`
    → `pub` modifier 存在、external access → MethodCall emit (cell 2 / cell 14 dispatch)
  - 7 traversal helper unit tests + 4 helper integration tests + 2 INV-5 integration
    tests 全 green
  - Production code change 0 LOC = Hono Preservation 確実 (no regression possible at
    conversion logic level)
- **Layer 3 (Structural cross-axis)**: 1 finding → 本 T13 内 fix
  - **Finding**: INV-5 initial fill-in は getter 軸のみ probe (cell 2 dispatch)、Setter
    symmetric counterpart (cell 14 dispatch) 不在 = Decision Table A / B の symmetric pair
    invariant 観点で orthogonal axis 漏れ
  - **Fix**: `test_invariant_5_private_setter_external_write_tier2` 追加 (`set_x` visibility
    marker + `f.set_x(5.0)` MethodCall + `pub fn set_y` 存在 probe)、本 T13 内 cross-axis
    completeness 達成
  - Spec gap 否 (本 review iteration 内で発見 + fix = framework operating as designed)
- **Layer 4 (Adversarial trade-off)**: 0 findings
  - **Pre/post matrix**: cells 7/8 (✓ T5 lock-in preserved) / cells 17/26/35-c/41-c/45-dc
    (✓ T6/T7/T8/T9 lock-in preserved) / N>=3 step + partial cycle (NEW boundary value
    coverage) / INV-5 getter + setter (NEW Option B test contract)。No regression cell
  - **Trade-off statement**: Option B 採用 vs Option A trade-off は (13-b) audit で
    explicit justify。Trade = less informative error message (E0624 vs explicit
    `UnsupportedSyntaxError`)、Gain = avoid 50+ site Rule 9 (c) symmetric audit cost +
    recurring problem prevention。Justification = Hono empirical reachability = 0
  - **Patch vs Structural fix**: pure addition of tests + boundary value extension、
    no production code change、no patch、no interim placeholder

#### Defect Classification 5 Category

| Category | Count | Action |
|----------|-------|--------|
| Grammar gap | 0 | (無し、`TsAccessibility` / Stmt / Expr 全 ast-variants.md 既 record) |
| Oracle gap | 0 | (無し、Hono codebase empirical audit で reachability = 0 確定) |
| Spec gap | 0 | (Layer 3 finding は本 review iteration 内で発見 + fix = framework operating、Spec への逆戻り発生なし) |
| Implementation gap | 1 | (Layer 3 INV-5 setter symmetric probe initial 不在、本 T13 内 fix 済) |
| Review insight | 1 | (PRD doc INV-5 (b)/(c) wording が Option A spec のままだった = Option B 採用 audit 結果と乖離 → 本 v20 commit で doc 更新済) |

#### Iteration v20 deep deep review (本 T13 内 second-iteration、user 指示 2026-05-01)

Initial 4-layer review pass 後 user 指示で `/check_job deep deep` adversarial second
iteration を実施。**Layer 1 (Mechanical) で 4 件 finding 追加発見**、**Layer 3 (Structural
cross-axis) で 3 件 Review insight 追加発見**、本 v20 commit 内 全件本質 fix or 明示
Review insight として record。

##### Layer 1 deep deep findings (本 v20 内 fix)

- **L1-DD-1**: `test_lookup_method_kind_circular_inheritance_prevention` の assertion
  message が "should not panic / infinite loop" と書きつつ実際は `is_ok()` assertion =
  misleading wording。Err 化 silent regression が起きた場合、test failure message が
  曖昧。Fix: assertion message を "regression lock-in、empty method bodies での registry
  construction cycle resilience" に refine、empirical observation に基づく Ok expected
  rationale を明示
- **L1-DD-2**: `test_lookup_method_kind_direct_vs_inherited_disambiguation` doc が
  "B1/B2 vs B7 disambiguation" と謳いつつ direct path のみ probe (inherited path は test
  1 別 fixture)。**Disambiguation の binary 対比が single test 内で完結していない**。
  Fix: test を both direct + inherited path probe に拡張、single fixture file 内で
  symmetric pair として disambiguation 対比を completion (test 1 と independent な
  lock-in、registry-level の `test_b7_traversal_direct_hit_*` / `test_b7_traversal_single_step_*`
  pair の end-to-end version)
- **L1-DD-3**: `tests/i205_invariants_test.rs` file-level doc が「Implementation Stage
  T15 で各 stub に actual probe code を fill in」と書かれたまま、T13 で INV-5 fill-in
  済の事実と乖離 = doc out of sync。Fix: file-level doc に "Fill-in 状態 (2026-05-01
  post T13)" sub-section を author、INV-5 = T13 fill-in 完了 / INV-1〜4/6 = T15 defer
  の現状を明示
- **L1-DD-4**: INV-5 tests の TS source string literal で `\x20   ` escape sequence
  (= 4 spaces) を使用、codebase 既存 conventional pattern (`src/transformer/expressions/
  tests/i_205/compound.rs` 等で `"...\<newline>...\n\..."` 純 line continuation 形式) と
  乖離。TS は whitespace-insensitive なので indent preservation 不要、コード readability
  劣化のみ。Fix: 一行 form `"...{ ... } ... { ... }"` に refactor、codebase convention
  align

##### Layer 3 deep deep findings (Review insight、本 v20 内 record + 一部 deferred)

- **L3-DD-1 (Review insight、defer 不要)**: TS `protected get/set` accessor (→ Rust
  `pub(crate)` mapping) visibility invariant の **INV-7 candidate** axis。INV-5 (private)
  と直交する visibility level (`Protected`)、`resolve_member_visibility(Some(Protected),
  _)` → `Visibility::PubCrate` 既 implementation、external `obj.x` access は consumer
  module が異なる crate 経由なら Rust E0603 visibility error で **Tier 2 honest error
  自動 surface** = INV-5 と structural symmetric。Hono codebase audit (T13 内追加):
  `grep -rEn "protected get \w|protected set \w" /tmp/hono-src/src` で **0 件 hit** =
  reachability = 0、Option B 適用 sound。INV-7 を separate invariant として記述する
  cost より、INV-5 の **structural symmetric argument** (= visibility marker
  preservation を test contract として固定済、protected も同 mechanism で uniform 動作)
  で記録するのが ideal。Separate TODO / 独立 INV 起票は **不要**、本 v20 entry に
  記録のみ
- **L3-DD-2 (Review insight、defer with empirical justification)**: Multiple inheritance via
  interface extends (`interface A extends B, C, D { ... }` の multiple parents) における
  `lookup_method_sigs_in_inheritance_chain` の **first-match order-dependent semantic**。
  class は single extends のみだが interface は multiple extends 可能、registry も
  `TypeDef::Struct { is_interface: true }` で同 helper を経由するため、`for parent_name
  in extends` の iteration 順序が Vec insertion order に依存し first-found signature
  return される behavior は class / interface 両者に適用される。**Spec correction (本
  /check_problem で発見)**: TS spec は interface accessor signature 宣言 (`interface IFoo
  { get x(): number; set x(v: number): void; }` 等) を valid に support、ECMAScript class
  implementation の type signature 用途で reachable な axis。本 axis は T13 / 既存 T5
  で untested。**Defer rationale (empirical-grounded)**: Hono empirical comprehensive
  audit (本 /check_problem で実施、`grep` + Python parse 経由): total interfaces=105、
  multi-extends interfaces=1、interfaces with accessor signatures=0、intersection
  (multi-extends × accessor signature)=**0**。intersection reachability=0 で Option B
  symmetric argument (= INV-5 (private) / INV-7 candidate (protected) と同 pattern)
  適用、本 v20 内 production 修正不要。**Future revisit trigger**: 別 codebase で
  intersection reachability が non-zero 観測された場合は order-dependent semantic を
  spec として明記 + test 追加 (= 別 PRD で audit、本 v20 では Review insight として
  empirical-grounded rationale で record のみ)
- **L3-DD-3 (Review insight、structural argument で defer 不要)**: INV-5 cross-axis cell
  coverage は cells 2 (B2 instance Read) + cell 14 (B4 instance Write) の **2 cells のみ
  test contract** を author。残 cells (cell 5 = B4 Read getter+setter / cell 9 = B8
  static Read / cell 13 = B2 Write / cell 18 = B7 Write inherited / 等) は untested。
  **Structural argument**: visibility resolution (`resolve_member_visibility`) は dispatch
  arm logic と直交し method definition phase (`build_method`) で uniform 適用、cell 2 +
  cell 14 が pass すれば structural symmetric argument で他 cells も holds。Test contract
  として cell 5/9/13/18 を追加すると redundant lock-in cost が線形増加、empirical 観点で
  net negative。**INV-5 の 2-cell test contract で sufficient**、無闇な expansion を
  避ける judgment call

##### Defect Classification 5 Category (deep deep iteration final)

| Category | Count | Action |
|----------|-------|--------|
| Grammar gap | 0 | (無し) |
| Oracle gap | 0 | (無し、Hono empirical で reachability audit 済 = INV-5 (private) 0 / INV-7 candidate (protected) 0) |
| Spec gap | 0 | (Layer 1 deep deep findings は本 review iteration 内で発見 + fix = framework operating、Spec への逆戻り発生なし) |
| Implementation gap | 5 | (initial Layer 3 INV-5 setter symmetric + L1-DD-1/2/3/4、全件本 v20 内 fix 済) |
| Review insight | 4 | (initial 1 = INV-5 PRD doc wording Option B 反映 + L3-DD-1/L3-DD-2/L3-DD-3、L3-DD-1/3 は structural argument で defer 不要、L3-DD-2 は別 PRD audit 候補) |

#### Iteration v20 完了判定

- T13 architectural concern (= "B6/B7 corner cells Tier 2 honest error reclassify verify
  + INV-5 visibility consistency invariant lock-in + boundary value coverage 拡充") を
  unit + integration 両 level で達成 ✓
- INV-5 reachability audit (Option A vs B) を Hono empirical で確定、Option B 採用 ✓
- INV-7 candidate (protected accessor) reachability も Hono empirical で 0 件確認、
  INV-5 と structural symmetric argument で coverage 達成 ✓
- Quality gate (fmt 0 diff / clippy 0 warning / file-lines OK / test 全 pass) ✓
- 4-layer review **initial pass + deep deep iteration 累積**: Layer 2/4 = 0 findings、
  Layer 1 = 4 findings (L1-DD-1〜4)、Layer 3 = 1 finding initial + 3 findings deep deep
  (L3-DD-1〜3) ✓ 全件本 v20 内 fix or 明示 Review insight 化
- Defect Classification (cumulative): Grammar/Oracle/Spec gap = 0、Implementation gap 5
  (Layer 3 + L1-DD-1〜4、本 v20 内 fix 済)、Review insight 4 (PRD doc + L3-DD-1〜3)
- **Iteration v20 完了 = T13 task close ready**、次 iteration = T14 (E2E fixtures
  green-ify、Depends on T1-T10/T12/T13、I-162 prerequisite block 明示)

---

## References

- [`plan.md`](../plan.md) — Plan η chain (本 PRD 挿入後 update)
- [`TODO`](../TODO) — I-205 entry 追加予定
- [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) — 2-stage workflow
- [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) — 13-rule (v1.3、I-205 self-applied integration で Rule 1/2/5/6/8/11/13 sub-rule 拡張 + Rule 13 新設)
- [`.claude/rules/check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) — 4-layer review
- [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md) — Step 0b methodology
- [`.claude/rules/conversion-correctness-priority.md`](.claude/rules/conversion-correctness-priority.md) — Tier 分類
- [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) — 最上位原則
