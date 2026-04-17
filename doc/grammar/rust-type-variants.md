# IR RustType Variant Catalog (Beta)

**Version snapshot**: `src/ir/types.rs` (2026-04-17, post-I-387 restructuring)
**Pilot validated**: I-050-a (2026-04-17) — String/F64/Bool/Any の 4 variant を matrix 列挙に使用、漏れなし

本ドキュメントは `spec-first-prd.md` の grammar-derived matrix 作成時に参照する。
PRD の入力次元 (TS type / Rust type) を列挙する際、本カタログの全 variant について
「この機能に関与するか否か」を判定する。

**更新トリガー**: `RustType` / `PrimitiveIntKind` / `StdCollectionKind` に variant を
追加・削除した際に同時更新。

---

## 1. RustType (18 variants)

| # | Variant | Rust 型 | TS 由来型 | 典型用途 |
|---|---------|---------|----------|---------|
| 1 | `Unit` | `()` | `void` | 戻り値なし関数 |
| 2 | `String` | `String` | `string` | 文字列型 |
| 3 | `F64` | `f64` | `number` | 数値型 (TS は全て f64) |
| 4 | `Bool` | `bool` | `boolean` | 真偽値 |
| 5 | `Option(inner)` | `Option<T>` | `T \| undefined`, `T \| null`, `x?: T` | nullable / optional |
| 6 | `Vec(elem)` | `Vec<T>` | `T[]`, `Array<T>` | 配列 |
| 7 | `Fn { params, return_type }` | `impl Fn(P) -> R` / `Box<dyn Fn(P) -> R>` | `(x: T) => R`, function type | 関数型 |
| 8 | `Result { ok, err }` | `Result<T, E>` | throw を含む関数の戻り値 | エラーハンドリング |
| 9 | `Tuple(elems)` | `(T1, T2, ...)` | `[T1, T2]` (fixed-length) | タプル |
| 10 | `Any` | `serde_json::Value` | `any`, `unknown` | 動的型 |
| 11 | `Never` | `std::convert::Infallible` | `never` | 到達不能型 |
| 12 | `Named { name, type_args }` | user-defined struct/enum | interface, class, type alias | ユーザー定義型 |
| 13 | `TypeVar { name }` | `T` (generic param) | `<T>` type parameter | 型変数 (I-387) |
| 14 | `Primitive(kind)` | `i32`, `usize`, etc. | number subtype (cast 経由) | 整数型 (I-387) |
| 15 | `StdCollection { kind, args }` | `HashMap<K,V>`, `Box<T>`, etc. | Record, Map, etc. | std コレクション (I-387) |
| 16 | `Ref(inner)` | `&T` | 参照パラメータ | 参照型 |
| 17 | `DynTrait(name)` | `dyn Trait` | interface (trait 化時) | トレイトオブジェクト |
| 18 | `QSelf { qself, trait_ref, item }` | `<T as Trait>::Item` | conditional type infer | 限定パス型 |

### PRD 作成時のチェックポイント

matrix の「TS type」次元を列挙する際、以下の 18 variant **全て** について
「この機能で LHS / RHS / target / source として出現し得るか」を判定する:

```
Unit, String, F64, Bool, Option<T>, Vec<T>, Fn, Result, Tuple,
Any, Never, Named, TypeVar, Primitive, StdCollection, Ref, DynTrait, QSelf
```

特に以下の variant は見落としやすい:
- **`Option<Any>`**: `any | null` は `Option<serde_json::Value>` になる。`Option<T>` と `Any` の組合せ。
- **`TypeVar`**: generic 関数内の型変数。具象型と挙動が異なる場合がある。
- **`Never`**: 到達不能だが型 position に出現する (conditional type の false branch 等)。
- **`QSelf`**: associated type 参照。`T extends Promise<infer U> ? U : never` の `U`。

---

## 2. PrimitiveIntKind (13 variants)

`RustType::Primitive(kind)` の `kind` が取る値:

| # | Variant | Rust 型 | 用途 |
|---|---------|---------|------|
| 1 | `Usize` | `usize` | index, length |
| 2 | `Isize` | `isize` | signed index |
| 3 | `I8` | `i8` | narrow int |
| 4 | `I16` | `i16` | narrow int |
| 5 | `I32` | `i32` | general int |
| 6 | `I64` | `i64` | wide int |
| 7 | `I128` | `i128` | BigInt fallback |
| 8 | `U8` | `u8` | byte |
| 9 | `U16` | `u16` | unsigned |
| 10 | `U32` | `u32` | unsigned |
| 11 | `U64` | `u64` | unsigned |
| 12 | `U128` | `u128` | unsigned wide |
| 13 | `F32` | `f32` | single precision float |

**備考**: `f64` / `bool` / `String` / `()` は専用の top-level variant (`F64`, `Bool`, `String`, `Unit`)
を使用し、`Primitive` には含まれない。production で主に使用されるのは `Usize` (index cast) と
`I128` (BigInt) のみ。他の variant は YAGNI 例外として維持 (plan.md 設計判断参照)。

---

## 3. StdCollectionKind (12 variants)

`RustType::StdCollection { kind, args }` の `kind` が取る値:

| # | Variant | Rust 型 | TS 由来型 | 用途 |
|---|---------|---------|----------|------|
| 1 | `Box` | `Box<T>` | (内部使用) | trait object boxing, recursive type |
| 2 | `HashMap` | `HashMap<K, V>` | `Record<K, V>`, `Map<K, V>` | key-value store |
| 3 | `BTreeMap` | `BTreeMap<K, V>` | `Map<K, V>` (ordered) | ordered map |
| 4 | `HashSet` | `HashSet<T>` | `Set<T>` | set |
| 5 | `BTreeSet` | `BTreeSet<T>` | `Set<T>` (ordered) | ordered set |
| 6 | `VecDeque` | `VecDeque<T>` | (未使用) | deque |
| 7 | `Rc` | `Rc<T>` | (未使用) | reference counting |
| 8 | `Arc` | `Arc<T>` | (未使用) | atomic ref counting |
| 9 | `Mutex` | `Mutex<T>` | (未使用) | mutex |
| 10 | `RwLock` | `RwLock<T>` | (未使用) | read-write lock |
| 11 | `RefCell` | `RefCell<T>` | (未使用) | interior mutability |
| 12 | `Cell` | `Cell<T>` | (未使用) | interior mutability |

**備考**: production で主に使用されるのは `Box` (trait object / recursive type) と
`HashMap` (Record/Map 変換) のみ。`HashSet`/`BTreeMap` は Set/ordered Map で使用。
`Rc`/`Arc`/`Mutex`/`RwLock`/`RefCell`/`Cell`/`VecDeque` は将来の所有権推論 (I-048) で
導入予定。

---

## 4. 補助型

### TraitRef (QSelf 内部)

```rust
pub struct TraitRef {
    pub name: String,
    pub type_args: Vec<RustType>,
}
```

`QSelf` variant で `<T as Trait<Args>>::Item` の `Trait<Args>` 部分を構造化。

---

## 5. RustType の主要メソッド (matrix 作成に関連するもの)

| メソッド | 用途 | matrix 作成時の relevance |
|---------|------|------------------------|
| `is_copy_type()` | Copy-ness 構造判定 | `??=` / NC の eager/lazy 選択 |
| `wrap_optional()` | idempotent Option wrap | optional param emission |
| `wrap_if_optional(bool)` | conditional Option wrap | 10 経路の optional 収束点 |
| `unwrap_promise()` | Promise\<T\> → T | async return type |
| `inner_option()` | Option\<T\> → Some(T) | NC / `??=` の inner 型取得 |
| `is_option()` | Option 判定 | 各種 Option-aware 処理 |

---

## 6. TS 型 → RustType 逆引き

PRD で「この TS 型はどの RustType variant にマップされるか」を確認する際の参照表:

| TS 型 | RustType |
|-------|---------|
| `string` | `String` |
| `number` | `F64` |
| `boolean` | `Bool` |
| `void` | `Unit` |
| `any` | `Any` |
| `unknown` | `Any` |
| `never` | `Never` |
| `null` | `Option(inner)` (context 依存) |
| `undefined` | `Option(inner)` (context 依存) |
| `T[]` / `Array<T>` | `Vec(elem)` |
| `[T, U]` (tuple) | `Tuple([T, U])` |
| `T \| null` / `T \| undefined` | `Option(T)` |
| `Record<K, V>` | `StdCollection { kind: HashMap, args: [K, V] }` |
| `Map<K, V>` | `StdCollection { kind: HashMap, args: [K, V] }` |
| `Set<T>` | `StdCollection { kind: HashSet, args: [T] }` |
| `Promise<T>` | `Named { name: "Promise", type_args: [T] }` → unwrap to `T` |
| `(x: T) => R` | `Fn { params: [T], return_type: R }` |
| `interface Foo {}` | `Named { name: "Foo" }` or `DynTrait("Foo")` |
| `class Bar {}` | `Named { name: "Bar" }` |
| `type Alias = T` | target RustType (alias 解決後) |
| `<T>` (type param) | `TypeVar { name: "T" }` |
| `bigint` | `Primitive(I128)` |
