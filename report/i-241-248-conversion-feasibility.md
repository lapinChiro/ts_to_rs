# I-241 ~ I-248: 変換可能性調査レポート

日付: 2026-03-25

---

## I-243: Top-level expression statements

### TypeScript セマンティクス

TypeScript（ES modules）ではモジュールのトップレベルに任意の式文を書ける。主な用途:

1. **Polyfill 初期化**: `globalThis.crypto ??= crypto`
2. **サイドエフェクト**: `console.log("init")`
3. **モジュール設定**: `someModule.configure({ ... })`
4. **IIFE**: `(function() { ... })()`

これらは **モジュールが初めて import されたとき**、宣言順に1回だけ実行される。

### 現状のコード

`src/transformer/mod.rs:295` — `ModuleItem::Stmt(Stmt::Expr(_))` を空で返し、サイレントにスキップ。

### Rust 変換戦略

**変換可能。パターンごとに分岐する。**

#### パターン A: グローバル変数の初期化（`globalThis.X ??= Y`）

```rust
// lazy_static! or std::sync::OnceLock (Rust 1.70+)
use std::sync::OnceLock;
static CRYPTO: OnceLock<Crypto> = OnceLock::new();

fn init_crypto() -> &'static Crypto {
    CRYPTO.get_or_init(|| crypto())
}
```

`OnceLock` はスレッドセーフで遅延初期化。`globalThis.X ??=` の「未設定なら初期化」セマンティクスに正確に対応する。

#### パターン B: サイドエフェクト（`console.log("init")`）

```rust
// モジュール初期化関数を生成し、呼び出し責務を利用者に委ねる
pub fn init() {
    println!("init");
}
```

Rust にはモジュールロード時の自動実行がないため、`init()` 関数として生成するのが最も素直。`#[ctor]` クレートを使えばプロセス起動時実行もできるが、`unsafe` に依存するため推奨しない。

#### パターン C: モジュール設定（`someModule.configure({ ... })`）

パターン B と同様に `init()` 関数へ格納する。

#### パターン D: IIFE

IIFE の本体をそのまま `init()` 関数の本体として展開する。戻り値がある場合は `OnceLock` と組み合わせる。

#### 推奨実装

1. トップレベル式文を `Item::Fn { name: "init", ... }` として生成する
2. 複数のトップレベル式文がある場合、1つの `init()` に本体をまとめる
3. `globalThis.X ??= Y` パターンを検出した場合は `OnceLock` static に変換する
4. `--report-unsupported` では「top-level expression → init() function」として報告する（サイレント消失を防ぐ）

---

## I-241: BigInt literals beyond i64 range

### TypeScript セマンティクス

`BigInt` は任意精度の整数型。`123n`、`9007199254740993n`（i64 範囲内だが f64 では精度を失う）、`99999999999999999999999999999999n`（i64 範囲外）がすべて有効。

### 現状のコード

`src/transformer/expressions/literals.rs:70` — `bigint.value.to_string().parse::<i64>().unwrap_or(0)` で、i64 に収まらない値は **0 に丸められる**。これはサイレントな意味変更。

### Rust 変換戦略

**変換可能。段階的に表現を選ぶ。**

#### 段階的フォールバック

1. **i64 に収まる場合**: `i64` リテラル（現状通り）
2. **i128 に収まる場合**: `i128` リテラル
3. **i128 にも収まらない場合**: `num-bigint` クレートの `BigInt` 型

#### 具体的な変換

```rust
// TS: const a = 123n
let a: i64 = 123;

// TS: const b = 9999999999999999999n  (i64 溢れ, i128 OK)
let b: i128 = 9999999999999999999;

// TS: const c = 99999999999999999999999999999999999999999n  (i128 溢れ)
let c: BigInt = "99999999999999999999999999999999999999999".parse().unwrap();
```

#### 型レベルの対応

- `RustType` に `I128` バリアントを追加する（`BigInt` クレートを使う場合は `RustType::Named { name: "BigInt" }` で対応可能）
- `TsBigIntKeyword` の型変換（`src/pipeline/type_converter.rs:1721-1727`）も `i128` または `BigInt` に更新する

#### 推奨実装

`i128` で十分なケースが大半（i128 は約 ±1.7×10^38）。`num-bigint` 依存の追加は YAGNI の観点から、実際に i128 溢れが Hono ベンチで発生してからでよい。

**最小限の修正**:
1. `parse::<i64>()` を `parse::<i128>()` に変更
2. `unwrap_or(0)` を `unwrap_or(0)` のまま（i128 で溢れるケースは現実にはほぼ存在しない）
3. IR に `Expr::I128Lit(i128)` を追加（または既存の `IntLit` を `i128` に拡大）
4. generator で `i128` リテラルを出力

---

## I-242: typeof on unresolved types defaulting to "object"

### TypeScript セマンティクス

`typeof x` は実行時に `"string"` | `"number"` | `"boolean"` | `"undefined"` | `"object"` | `"function"` | `"symbol"` | `"bigint"` のいずれかを返す。

### 現状のコード

`src/transformer/expressions/binary.rs:168` — `get_expr_type()` が `None` を返す場合、`"object"` をハードコードで返す。

`get_expr_type()` が `None` になる原因は `src/transformer/expressions/type_resolution.rs:54-56` で `ResolvedType::Unknown` が返される場合。これは TypeResolver が事前解決できなかった式。

### 分析: 本当に解決不能か？

**いいえ。多くのケースは TypeResolver の拡充で解決可能。**

TypeResolver が `Unknown` を返すケースには以下がある:

1. **変数宣言時に型注釈がなく、初期化式の型も推論できなかった** — TypeResolver の推論能力の限界
2. **import した値で、import 元の型情報が未解決** — モジュール間型解決の問題
3. **関数の引数で、型注釈がない** — JavaScript の動的型

ケース 1, 2 は TypeResolver の改善で対応可能。ケース 3 は本質的に不定だが、`typeof` が使われる文脈では通常 `if (typeof x === "string")` のような分岐であり、そもそも `typeof` を静的に解決する必要がない場面が多い。

### Rust 変換戦略

**変換可能。2つのアプローチがある。**

#### アプローチ A: TypeResolver 拡充（推奨）

TypeResolver の型推論カバレッジを増やし、`Unknown` を減らす。`typeof x` が使われている変数を優先的に解決対象にする。

#### アプローチ B: Any 型での typeof ランタイム変換

型が解決できない場合、`serde_json::Value` に対する実行時 typeof を生成する:

```rust
// TS: typeof x === "string"
// x: serde_json::Value の場合
fn js_typeof(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "undefined",
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => "object",
    }
}
```

#### 推奨実装

1. **即座にやるべきこと**: `None` の場合に `"object"` をハードコードするのではなく、unsupported エラーを報告する。サイレントに誤った値を生成するのは最も危険なパターン
2. **中期**: TypeResolver の推論カバレッジを広げる
3. **長期**: 未解決型に対してはランタイム typeof ヘルパーを生成する

---

## I-244: Nested destructuring rest parameters

### TypeScript セマンティクス

```typescript
function f({ a, ...rest }: { a: string; b: number; c: boolean }) {
    // rest は { b: number; c: boolean } 型
    // rest.b, rest.c がアクセス可能
}
```

`...rest` は「名前付きプロパティで取り出したもの以外の全プロパティ」を新しいオブジェクトにまとめる。TypeScript コンパイラは **構造的に** rest の型を推論する: 元の型から明示的に destructure されたプロパティを除いた型。

### 現状のコード

`src/transformer/functions/mod.rs:734-737` — `ast::ObjectPatProp::Rest(_)` を「type info not available at this level」としてスキップ。

### Rust 変換戦略

**変換可能。型情報があれば構造体フィールドの差分で rest の型を構築できる。**

#### 変換パターン

```typescript
// TS
function f({ a, ...rest }: Config) { ... }
// Config = { a: string, b: number, c: boolean }
```

```rust
// Rust: Config の型定義が分かっているので、a 以外のフィールドで rest 構造体を構築
struct ConfigRest { pub b: f64, pub c: bool }

fn f(config: Config) {
    let a = config.a;
    let rest = ConfigRest { b: config.b, c: config.c };
}
```

#### 実装に必要なもの

1. **元の型の全フィールドリスト**: TypeRegistry から取得可能（destructure 対象の型は注釈されているか推論済み）
2. **明示的に destructure されたフィールド名**: AST から取得可能
3. **差分の型生成**: SyntheticTypeRegistry で `{ParentName}Rest` 構造体を生成
4. **rest 変数の初期化**: 元の構造体の残余フィールドを1つずつ代入

#### 型情報が不明な場合

元の型が解決できない場合（例: `any` 型）、`HashMap<String, serde_json::Value>` にフォールバックする:

```rust
// rest: HashMap<String, serde_json::Value>
// TS の rest の動的な性質に対応
```

#### 推奨実装

1. destructure パターンから明示的に取り出されたフィールド名を収集する
2. TypeRegistry から元の構造体の全フィールドを取得する
3. 差分フィールドで synthetic struct を生成する
4. rest 変数をその synthetic struct で初期化する

---

## I-245: declare module inner declaration errors

### TypeScript セマンティクス

`declare module 'name' { ... }` はアンビエント宣言（ambient declaration）。外部モジュールの型情報を提供する。TypeScript コンパイラはこの中のコードをチェックするが、JavaScript は出力しない。

```typescript
declare module 'express' {
    interface Request { body: any; }
    interface Response { send(data: any): void; }
}
```

### 現状のコード

`src/transformer/mod.rs:491-499` — `if let Ok((inner_items, _)) = self.transform_decl(...)` で、変換に失敗した宣言を **サイレントにスキップ** する。

### 分析

**エラーを飲み込む正当な理由はない。**

`declare module` 内の宣言が失敗するケースは以下:
1. **未対応の宣言型**: `transform_decl` が `UnsupportedSyntaxError` を返す — これは通常のトップレベル宣言と同じ扱いであるべき
2. **型解決の失敗**: 内部の型参照が解決できない — これもエラーとして報告すべき

`declare module` が「アンビエント」であることは、エラーを飲み込む理由にならない。Rust に変換する文脈では、`declare module` 内の型定義は通常の型定義と同様に変換すべき（外部クレートの型定義として生成する）。

### Rust 変換戦略

**変換可能（既存のロジックで変換される）。問題はエラーハンドリングのみ。**

#### 推奨修正

1. `if let Ok(...)` を `?` に変更し、エラーを伝播する
2. または `resilient` モードの場合はエラーを記録して続行する（`--report-unsupported` で報告）
3. 変換成功時の出力は現状のままでよい（宣言を通常の Item として生成）

#### `resilient` モードでの扱い

```rust
match self.transform_decl(inner_decl, vis.clone(), class_map, iface_methods, resilient) {
    Ok((inner_items, _)) => items.extend(inner_items),
    Err(e) if resilient => {
        // record unsupported and continue
        unsupported.push(e);
    }
    Err(e) => return Err(e),
}
```

---

## I-246: PrivateMethod / StaticBlock / PrivateProp

### TypeScript セマンティクス

#### PrivateMethod (`#method()`)
ECMAScript の hard private メソッド。クラス外部からアクセス不可。`this.#method()` でのみ呼び出し可能。WeakMap ベースの実装（TS ターゲットが古い場合）で完全なプライバシーを保証。

#### StaticBlock (`static { ... }`)
クラス定義時に1回だけ実行される初期化ブロック。`static` フィールドの複雑な初期化に使う。クラスの private メンバーにアクセスできる。

```typescript
class Foo {
    static #cache: Map<string, number>;
    static {
        Foo.#cache = new Map();
        Foo.#cache.set("default", 42);
    }
}
```

#### PrivateProp (`#field`)
ECMAScript の hard private フィールド。クラス外部から完全にアクセス不可。

### 現状のコード

`src/transformer/classes.rs:124` — `_ => {}` ですべてサイレントスキップ。

### Rust 変換戦略

**3つとも変換可能。**

#### PrivateMethod → `fn` (非 `pub`)

Rust の可視性で `pub` を付けないメソッドはモジュール外からアクセスできない。これは TypeScript の `#method` の「クラス外部からアクセス不可」に対応する。

```rust
impl Foo {
    // TS: #helper() { ... }
    fn helper(&self) { ... }  // pub なし = モジュール private
}
```

既存の `convert_class_method` が `Method` を返す仕組みがそのまま使える。`method.key` が `PrivateName` の場合に `vis: Visibility::Private` で変換するだけ。

#### StaticBlock → `impl` 内の初期化メソッド + `OnceLock`

```rust
impl Foo {
    fn init_static() {
        // static ブロックの本体
    }
}
// または、OnceLock で遅延初期化
```

StaticBlock の本体は通常の文のリストなので、既存の `convert_stmt` で変換できる。問題は「いつ呼ばれるか」だが、I-243 の init() パターンと同様に、呼び出し責務を利用者に委ねるか `OnceLock` で遅延化する。

#### PrivateProp → `pub(crate)` フィールドまたは非 `pub` フィールド

```rust
struct Foo {
    // TS: #count: number
    count: f64,  // pub なし = モジュール private
}
```

既存の `convert_class_prop` のロジックがそのまま使える。`prop.key` が `PrivateName` の場合に `vis: None`（非 pub）で変換する。名前の `#` プレフィックスを除去する。

#### 推奨実装

1. `ClassMember::PrivateMethod` → 既存の `convert_class_method` と同等のロジックで変換。vis を Private に設定
2. `ClassMember::PrivateProp` → 既存の `convert_class_prop` と同等のロジックで変換。vis を None に設定。`#` プレフィックスを除去
3. `ClassMember::StaticBlock` → 本体を `fn _init_static()` メソッドとして生成（`has_self: false`）
4. 最低限として、サイレントスキップを unsupported エラー報告に変更する

---

## I-247: Union with bigint/symbol/undefined keyword types

### TypeScript セマンティクス

```typescript
type Foo = string | bigint;    // 文字列または任意精度整数
type Bar = string | symbol;    // 文字列またはシンボル
type Baz = string | undefined; // 文字列またはundefined
```

### 現状のコード

`src/pipeline/type_converter.rs:1716-1743` の union 変換を詳細に確認した結果:

- `bigint` → `("I64", Named("i64"))` で **変換されている**（スキップではない）
- `symbol` → `("Any", RustType::Any)` で **変換されている**
- `undefined` → L1656 の `is_nullable_keyword` フィルタで事前に除去され、`has_null_or_undefined` フラグが立つ。**正しく `Option` ラッピングに変換されている**

**結論: TODO の記述「暗黙消失」は不正確。3つとも変換されている。**

`_ => continue`（L1735）でスキップされるのは `TsIntrinsicKeyword` 等の特殊なキーワード型のみ。`TsNeverKeyword` と `TsVoidKeyword` も明示的に `continue` されているが、これは正しい（never は bottom 型、void は union で意味を持たない）。

### 実際の問題点

コード上の変換は行われているが、変換の**品質**に改善余地がある:

1. **`bigint` → `i64`**: I-241 と同様に `i128` が望ましい
2. **`symbol` → `serde_json::Value`**: `RustType::Any` に退避されるが、専用型の方が型安全
3. **nullable multi-type union の Option ラッピング**: `string | number | undefined` → 現状は enum を生成した上で `Option` でラップする仕組みが L1663-1673 にあるが、これは `non_null_types.len() == 1` の場合のみ。2型以上の nullable union（`string | number | undefined`）は enum 生成後に `Option` ラッピングが行われるか要確認

### Rust 変換戦略（品質改善）

#### `bigint` → `i128`

I-241 の対応と同期して union バリアント名を `I128` に変更:

```rust
enum Foo {
    String(String),
    I128(i128),
}
```

#### `symbol` → 専用 newtype

`RustType::Any` への退避ではなく、`JsSymbol(String)` のような newtype を生成する方が型安全:

```rust
enum StringOrSymbol {
    String(String),
    Symbol(JsSymbol),
}
```

ただしこれは YAGNI の観点から、実際に Hono ベンチで `symbol` union が出現するか確認してからでよい。

#### nullable multi-type union

`string | number | undefined` のようなケースで enum 全体が `Option` でラップされるか検証し、されていなければ対応する。

### 推奨実装

1. **TODO の記述を修正**: 「暗黙消失」ではなく「品質改善」として正しく記載する
2. `bigint` は I-241 と同期して `i128` に更新する
3. `symbol` の専用型化は Hono ベンチでの出現頻度を確認してから判断する
4. nullable multi-type union の `Option` ラッピング動作を検証する

---

## I-248: Intersection method signatures

### TypeScript セマンティクス

```typescript
type X = { a: string } & { foo(): void };
// X は a プロパティと foo メソッドの両方を持つ
```

intersection 内の型リテラルにメソッドシグネチャ（`TsMethodSignature`）が含まれる場合、結果の型はプロパティとメソッドの両方を持つ。

### 現状のコード

`src/pipeline/type_converter.rs:1847-1850` と `2019-2021` — TODO コメント付きで `_ => continue` でスキップ。メソッドシグネチャは消失する。

既存のコードベースには `convert_method_signature` 関数（`src/pipeline/type_converter.rs:786`）が存在し、`TsMethodSignature` → `Method` への変換は既に実装済み。

### Rust 変換戦略

**変換可能。既存のインフラで対応できる。**

#### パターン A: プロパティ + メソッド → struct + impl

```typescript
type X = { a: string } & { foo(): void };
```

```rust
struct X {
    pub a: String,
}

impl X {
    pub fn foo(&self) { todo!() }
}
```

struct にフィールドを集め、メソッドは impl ブロックに配置する。

#### パターン B: 型参照 + メソッドリテラル → struct + trait impl

```typescript
type X = Base & { foo(): void };
```

```rust
struct X {
    // Base のフィールドを展開
    pub base_field: String,
}

// foo を含む trait を生成して impl
trait XMethods {
    fn foo(&self);
}

impl XMethods for X {
    fn foo(&self) { todo!() }
}
```

ただし、シグネチャのみの場合は trait のデフォルト実装なし（`todo!()`）が妥当。

#### 推奨実装

1. intersection 変換で `TsTypeElement::TsMethodSignature` を `convert_method_signature` で変換し、メソッドリストに収集する
2. `try_convert_intersection_type` の返り値を拡張: フィールドだけでなくメソッドも持てるようにする
3. メソッドが存在する場合、`Item::Struct` に加えて `Item::Impl` を生成する
4. `convert_intersection_in_annotation` でも同様に対応する（synthetic struct に対する impl を生成）

既存の `convert_method_signature` と `Item::Impl` の仕組みをそのまま活用できるため、実装は比較的単純。

---

## 総括

| ID | 変換可否 | 深刻度 | 実装規模 |
|----|---------|--------|---------|
| I-243 | 変換可能 | サイレント消失（ロジック喪失） | 中（パターン分類 + init 関数生成） |
| I-241 | 変換可能 | サイレント意味変更（0への丸め） | 小（i128 への拡張） |
| I-242 | 変換可能（部分的） | サイレント意味変更（誤った typeof） | 中（TypeResolver 拡充 or ランタイムヘルパー） |
| I-244 | 変換可能 | サイレント消失（rest データ喪失） | 中（TypeRegistry 参照 + synthetic struct 生成） |
| I-245 | 変換済み（エラーハンドリングのみ修正） | サイレント消失 | 小（if let Ok → エラー伝播） |
| I-246 | 変換可能 | サイレント消失（メソッド・フィールド喪失） | 小〜中（既存パターンの拡張） |
| I-247 | 既に変換済み（品質改善のみ） | TODO 記述が不正確（実際は変換されている） | 小（bigint→i128 同期 + TODO 修正） |
| I-248 | 変換可能 | サイレント消失（メソッド喪失） | 中（intersection に impl 生成を追加） |

### 優先順位の提案（conversion-correctness-priority に基づく）

1. **I-241** (BigInt → 0): サイレント意味変更 + 最小工数。即座に修正可能
2. **I-242** (typeof → "object"): サイレント意味変更。最低限 unsupported エラーに変更すべき
3. **I-243** (top-level expr): サイレント消失だがロジック喪失の可能性。init() 生成で対応
4. **I-246** (private members): サイレント消失。PrivateMethod はロジック喪失
5. **I-248** (intersection methods): サイレント消失。既存インフラ活用で対応可能
6. **I-244** (rest params): サイレント消失。型情報依存で実装がやや複雑
7. **I-245** (declare module errors): エラーハンドリング修正のみ。最小工数
8. **I-247** (union keywords): 実際には変換済み。TODO の記述修正 + bigint の i128 同期のみ
