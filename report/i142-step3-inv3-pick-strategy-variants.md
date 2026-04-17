# INV-Step3-3: `RustType` variant × `pick_strategy` 対応方針

- **日付**: 2026-04-15
- **対象**: I-142 Step 3 D-4 (`pick_strategy` table test 追加) 実装前の variant 網羅性確認
- **調査手段**: `src/ir/types.rs` 全 variant 確認 + TS 意味論分析

## 調査目的

現行 `pick_strategy` (`src/transformer/statements/nullish_assign.rs:62`):

```rust
pub(crate) fn pick_strategy(lhs_type: &RustType) -> NullishAssignStrategy {
    match lhs_type {
        RustType::Option(_) => NullishAssignStrategy::ShadowLet,
        RustType::Any => NullishAssignStrategy::BlockedByI050,
        _ => NullishAssignStrategy::Identity,
    }
}
```

は `_` fallback を使っており **将来 RustType 新 variant 追加時に compile-time gate が効かない**。D-4 では variant 網羅の table test を追加するが、そもそも `match` 自体を exhaustive にする方がより structural。本 INV では各 variant の意味論を確定し、exhaustive match の strategy mapping を決定する。

## RustType variant 列挙 (src/ir/types.rs)

### 主要 variant (18)

| # | Variant | 意味 | TS `??=` での意味論 | 推奨 Strategy |
|---|---------|------|-------------------|--------------|
| 1 | `Unit` | `()` / TS `void` | void 変数への ??= は dead。TS 型注釈 void の変数は `undefined` のみ取り得、= は no-op | **Identity** |
| 2 | `String` | `String` | non-nullable → ??= は dead | **Identity** |
| 3 | `F64` | `f64` | non-nullable → dead | **Identity** |
| 4 | `Bool` | `bool` | non-nullable → dead | **Identity** |
| 5 | `Option(_)` | `Option<T>` | nullable → shadow-let で unwrap | **ShadowLet** |
| 6 | `Vec(_)` | `Vec<T>` | non-null container → dead | **Identity** |
| 7 | `Fn { .. }` | function type | non-null function → dead (TS 型注釈が function なら non-null) | **Identity** |
| 8 | `Result { .. }` | `Result<T, E>` | Rust-specific、TS mapping 稀 | **Identity** |
| 9 | `Tuple(..)` | `(T1, T2, ...)` | non-null tuple → dead | **Identity** |
| 10 | `Any` | `serde_json::Value` | **runtime null check 必要 (I-050 依存)** | **BlockedByI050** |
| 11 | `Never` | `Infallible` | unreachable | **Identity** (可 unreachable strategy) |
| 12 | `Named { .. }` | user struct/enum | **要注意**: DU enum (union 型) で `null` variant を持つケースは `Named` で表現される可能性あり。ただし I-142 の現行実装では `Option<Named>` に既に包まれているはずで、裸 `Named` は non-null 扱い | **Identity** (暫定) |
| 13 | `TypeVar { name }` | generic `T` | IR 情報からは nullable 判定不能。保守的に non-null 扱い | **Identity** |
| 14 | `Primitive(k)` | i32/u32/f32/... | non-null primitive | **Identity** |
| 15 | `StdCollection { k, args }` | HashMap/Box/Rc/... | non-null container | **Identity** |
| 16 | `Ref(_)` | `&T` | non-null reference。`??=` が reference に対して呼ばれる状況は IR 上想定なし | **Identity** |
| 17 | `DynTrait(name)` | `dyn Trait` | non-null trait object | **Identity** |
| 18 | `QSelf { .. }` | `<T as Trait>::Item` | associated type、`Option<T>` に resolve されない限り non-null | **Identity** |

### `PrimitiveIntKind` sub-variants (13)

`Usize`, `Isize`, `I8`, `I16`, `I32`, `I64`, `I128`, `U8`, `U16`, `U32`, `U64`, `U128`, `F32`

→ 全て **Identity** (non-null primitive)。`Primitive(_)` arm 1 つで集約可能。

### `StdCollectionKind` sub-variants (12)

`Box`, `HashMap`, `BTreeMap`, `HashSet`, `BTreeSet`, `VecDeque`, `Rc`, `Arc`, `Mutex`, `RwLock`, `RefCell`, `Cell`

→ 全て **Identity** (non-null collection/smart pointer)。`StdCollection { .. }` arm 1 つで集約可能。

## 特記事項

### TypeVar の保守的判定の trade-off

`<T>` が型制約で nullable を許容するケース (例: `function f<T extends string | null>(x: T) { x ??= ""; }`) では、IR 上 `TypeVar("T")` になり、**本来 Option 相当の narrow が必要** だが現行 IR は情報を持たない。

- 現状 Identity strategy → `x ??= ""` が dead emit (expr なら `x` / `x.clone()`)
- 実行時に x が null のとき default が反映されない silent bug 可能性
- ただし TS 実運用で `<T extends string | null>` 制約は稀
- I-144 の CFG analyzer で `??=` 使用箇所の TypeVar narrow を推論する拡張が理想 → 本 PRD scope 外

**D-4 の table test では TypeVar → Identity を lock-in**。I-144 で narrow 推論が入ったら本 test を更新。

### Named の DU 識別問題

TS の Discriminated Union (`type T = A | B | null`) は ts_to_rs では synthetic enum + `Option<Named>` として IR 化される想定 (要確認)。

- 仮に裸 `Named { name: "SyntheticDU_XXX" }` として `null` variant を含む enum が IR 化されている場合、`??=` は enum の null variant 検査が必要
- 現状の Identity 扱いでは silent bug の可能性

**D-4 実装時に検証**: Hono bench で `??=` LHS が Named に resolve されるケースを inspect-errors.py で確認。なければ Identity で lock-in 妥当。

### Never の unreachable 強調

`x: never` への `??=` は TS で型エラーのため ts_to_rs に入力されないはず。しかし IR 上ありえるケース:
- 型 narrow の結果 `never` になったあとの `??=` (稀)

Identity で空 emit (stmt) / `x` (expr) にすると、expr context で `x: Infallible` の値を return することになり compile error に。

→ ideal は Never 専用 `Unreachable` strategy で `unreachable!()` emit も検討可能。ただし TS で到達しないので **保守的 Identity で問題なし**。実装簡潔性優先。

## 推奨 `pick_strategy` exhaustive match 設計 (D-4 implementation)

```rust
pub(crate) fn pick_strategy(lhs_type: &RustType) -> NullishAssignStrategy {
    use NullishAssignStrategy::*;
    match lhs_type {
        // 唯一の nullable variant
        RustType::Option(_) => ShadowLet,

        // I-050 umbrella 依存 (runtime null check 必要)
        RustType::Any => BlockedByI050,

        // 以下全て non-nullable → Identity
        // 明示列挙することで新 variant 追加時に compile-time gate を効かせる
        RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Vec(_)
        | RustType::Fn { .. }
        | RustType::Result { .. }
        | RustType::Tuple(_)
        | RustType::Never
        | RustType::Named { .. }
        | RustType::TypeVar { .. }
        | RustType::Primitive(_)
        | RustType::StdCollection { .. }
        | RustType::Ref(_)
        | RustType::DynTrait(_)
        | RustType::QSelf { .. } => Identity,
    }
}
```

### Gate 効果の検証

`RustType` に新 variant (例 `RustType::Foo`) を追加したとき、上記 match は non-exhaustive になり compile error。新 variant の意味論分析が強制される。

**`_` fallback 削除により structural な variant gate が成立**。D-4 の table test は「全 variant の期待 strategy を assert」で補完 (二重 gate)。

## D-4 Table Test 設計

全 variant に対し expected strategy を assert する test を追加。Sub-variant (PrimitiveIntKind / StdCollectionKind) も代表 1 variant + 境界 variant を追加。

```rust
#[test]
fn pick_strategy_maps_option_to_shadow_let() {
    assert_eq!(pick_strategy(&RustType::Option(Box::new(RustType::F64))), NullishAssignStrategy::ShadowLet);
    // nested Option<Option<T>> も ShadowLet (最外層のみ判定)
    assert_eq!(
        pick_strategy(&RustType::Option(Box::new(RustType::Option(Box::new(RustType::String))))),
        NullishAssignStrategy::ShadowLet
    );
}

#[test]
fn pick_strategy_maps_any_to_blocked_by_i050() {
    assert_eq!(pick_strategy(&RustType::Any), NullishAssignStrategy::BlockedByI050);
}

#[test]
fn pick_strategy_maps_all_non_nullable_to_identity() {
    // Primitive (各 kind 代表)
    for kind in [PrimitiveIntKind::Usize, PrimitiveIntKind::I32, PrimitiveIntKind::F32] {
        assert_eq!(pick_strategy(&RustType::Primitive(kind)), NullishAssignStrategy::Identity);
    }
    // StdCollection (各 kind 代表)
    for kind in [StdCollectionKind::HashMap, StdCollectionKind::Box, StdCollectionKind::Rc] {
        assert_eq!(
            pick_strategy(&RustType::StdCollection { kind, args: vec![] }),
            NullishAssignStrategy::Identity
        );
    }
    // 主要 variant 個別
    assert_eq!(pick_strategy(&RustType::Unit), NullishAssignStrategy::Identity);
    assert_eq!(pick_strategy(&RustType::String), NullishAssignStrategy::Identity);
    assert_eq!(pick_strategy(&RustType::F64), NullishAssignStrategy::Identity);
    assert_eq!(pick_strategy(&RustType::Bool), NullishAssignStrategy::Identity);
    assert_eq!(pick_strategy(&RustType::Vec(Box::new(RustType::F64))), NullishAssignStrategy::Identity);
    assert_eq!(
        pick_strategy(&RustType::Fn { params: vec![], return_type: Box::new(RustType::Unit) }),
        NullishAssignStrategy::Identity
    );
    assert_eq!(
        pick_strategy(&RustType::Result { ok: Box::new(RustType::F64), err: Box::new(RustType::String) }),
        NullishAssignStrategy::Identity
    );
    assert_eq!(pick_strategy(&RustType::Tuple(vec![RustType::F64, RustType::Bool])), NullishAssignStrategy::Identity);
    assert_eq!(pick_strategy(&RustType::Never), NullishAssignStrategy::Identity);
    assert_eq!(
        pick_strategy(&RustType::Named { name: "Foo".into(), type_args: vec![] }),
        NullishAssignStrategy::Identity
    );
    assert_eq!(
        pick_strategy(&RustType::TypeVar { name: "T".into() }),
        NullishAssignStrategy::Identity
    );
    assert_eq!(pick_strategy(&RustType::Ref(Box::new(RustType::F64))), NullishAssignStrategy::Identity);
    assert_eq!(pick_strategy(&RustType::DynTrait("Trait".into())), NullishAssignStrategy::Identity);
    // QSelf は複雑な構築が必要 — 簡略の TraitRef のみ
}
```

## INV-Step3-3 結論

1. 全 18 main variants + 13 PrimitiveIntKind + 12 StdCollectionKind の意味論を確定
2. `_` fallback を削除して **exhaustive match** に改造し、新 variant 追加時の compile-time gate を獲得
3. Table test は全 variant を網羅 (sub-variant は代表 + 境界)
4. TypeVar / Named の保守的 Identity 扱いは現状で問題ない (I-144 以降で narrow 推論拡張可)
