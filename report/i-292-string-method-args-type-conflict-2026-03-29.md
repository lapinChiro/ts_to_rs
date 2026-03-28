# I-292: String メソッド引数型の衝突 — 根本原因分析

**日付**: 2026-03-29
**Base commit**: 2e87934（未コミット変更あり）

## 要約

`string-methods` fixture のビルトインありコンパイルエラーの根本原因は、**TypeResolver の expected type 伝播と Transformer のメソッド変換が独立に引数型を決定し、矛盾する出力を生む設計上の責務分離欠陥**である。

## 問題の再現

```typescript
function hasContent(s: string): boolean {
    return s.includes("x") && !s.endsWith("z");
}
```

ビルトインなし: `s.contains(&"x") && !s.ends_with("z")` → **コンパイル OK**
ビルトインあり: `s.contains(&"x".to_string()) && !s.ends_with("z".to_string())` → **コンパイルエラー**

## 問題発生のメカニズム

### パス 1: TypeResolver → expected type 設定

```
1. set_call_arg_expected_types(callee=s.includes, args=["x"])
2. lookup_method_params(String, "includes") → [("searchString", RustType::String)]
   ※ ecmascript.json: includes(searchString: string) → MethodSignature.params[0] = RustType::String
3. propagate_arg_expected_types(args, [RustType::String])
4. expected_types["x"] = RustType::String
```

### パス 2: Transformer → 引数変換

```
5. convert_lit("x", expected=Some(RustType::String))
6. matches!(expected, Some(RustType::String)) → true
7. → Expr::MethodCall { object: StringLit("x"), method: "to_string" }
   = "x".to_string()
```

### パス 3: Transformer → メソッド変換

```
8. map_method_call(object, "includes", args=["x".to_string()])
9. "includes" → Expr::MethodCall { method: "contains", args: [Ref(arg)] }
10. → s.contains(&"x".to_string())
```

### 結果

`s.contains(&"x".to_string())` — `&String` は `Pattern` trait を実装しないためコンパイルエラー。

## 根本原因: 責務の衝突

**2 つの独立したシステムが同じ引数に対して矛盾する変換を適用している。**

| システム | 責務 | 判断 | 出力 |
|---------|------|------|------|
| TypeResolver + convert_lit | expected type に基づき `.to_string()` 付加 | 「String 型が期待される → 文字列リテラルを String に昇格」 | `"x".to_string()` |
| map_method_call | メソッド名に基づき `&` 付加 | 「`contains` は `&str` を取る → 引数に `&` を付加」 | `&(引数)` |

**問題の本質**: `map_method_call` は「Rust の `contains` メソッドは `&str` を取る」というドメイン知識を持ち、引数に `&` を付加する。しかし TypeResolver は「TS の `includes` メソッドは `string` を取る」という別のドメイン知識で引数を `String` に昇格させる。この 2 つが合成されると `&String` になり、コンパイルエラー。

## 設計分析

### DRY 違反

String メソッドの引数型は 2 箇所で独立に定義されている:
1. **ecmascript.json** (`src/builtin_types/`): `includes(searchString: string)` → `RustType::String`
2. **map_method_call** (`src/transformer/expressions/methods.rs:49-53`): `includes` → `contains` + `Ref` で `&str` を期待

これらは「同じ知識（String メソッドの引数型）」の二重定義であり、互いに矛盾する。

### 直交性の欠如

TypeResolver の expected type 伝播と Transformer のメソッド変換が直交していない:
- TypeResolver は「引数の expected type を設定する」責務
- Transformer は「Rust の API に合わせてメソッド変換する」責務
- しかし、Transformer の `map_method_call` は引数に `&` や `.to_string()` 等の型変換を行い、TypeResolver の expected type に基づく変換と衝突する

### 問題の波及範囲

影響を受ける String メソッド:

| TS メソッド | map_method_call の変換 | 引数処理 | ビルトインあり |
|-----------|----------------------|---------|-------------|
| `includes` | `contains` | `Ref(arg)` | ❌ `&"x".to_string()` |
| `startsWith` | `starts_with` | そのまま | ❌ `"z".to_string()` — `Pattern` for `String` は stable 未実装 |
| `endsWith` | `ends_with` | そのまま | ❌ 同上 |
| `indexOf` | `iter().position()` | 条件式 | ⚠️ 要確認 |
| `split` | `split` | そのまま | ❌ `Pattern` for `String` は stable 未実装 |
| `replace` | `replacen` | そのまま | ⚠️ 1引数目が `Pattern` |

`contains`, `starts_with`, `ends_with`, `split` は全て `Pattern` trait に依存。Rust stable では `&str` は `Pattern` を実装するが `String` は実装**しない**（nightly の feature gate `pattern` が必要）。

### 問題の所在: どちらが「正しい」か

**map_method_call が正しく、TypeResolver の expected type が不適切。**

理由:
- Rust の `str::contains` は `P: Pattern` を取り、`&str` は `Pattern` を満たすが `String` は満たさない
- TS の `string.includes(string)` は TS の型システム上 `string` だが、Rust に変換した時のメソッドシグネチャは `&str` を期待する
- TypeResolver が TS のメソッドシグネチャ（`string` → `RustType::String`）をそのまま expected type として伝播するのは、**TS のドメイン知識を Rust のコード生成に直接適用している**誤り

### 理想的な解決方向

**選択肢 A: map_method_call が引数の expected type を上書きする**

`map_method_call` で `includes` → `contains` に変換する際、引数の `.to_string()` を除去し `&str` に戻す。

問題: map_method_call は IR レベルで動作し、TypeResolver の expected type に遡及できない。

**選択肢 B: TypeResolver が Rust のメソッドシグネチャを考慮する**

String メソッドの expected type を `RustType::String` ではなく、Rust の実際のシグネチャに合わせた型にする。例: `contains` は `&str` → expected type を付与しない（`&str` リテラルのまま）。

問題: TypeResolver は Rust のメソッドシグネチャを知らない。TS のシグネチャから Rust のシグネチャへのマッピングが必要。

**選択肢 C: map_method_call を TypeResolver の expected type を考慮して変換する**

`map_method_call` が引数の expected type を見て、不要な `.to_string()` を除去する。

問題: map_method_call は Transformer の一部で、TypeResolver の情報に直接アクセスしない。

**選択肢 D: ビルトイン型のメソッドシグネチャを Rust 視点で定義する**

ecmascript.json から変換された MethodSignature のパラメータ型が `RustType::String` ではなく、Rust の実際の受け入れ型（`&str`）を反映するようにする。

問題: `RustType` に `&str` を表現する型がない（`Ref(String)` は `&String` であって `&str` ではない）。

**選択肢 E: メソッドが既に Transformer で変換される場合、TypeResolver は引数の expected type を設定しない**

`map_method_call` で変換されるメソッド（`includes`, `startsWith`, `endsWith` 等）については、TypeResolver が引数に expected type を設定しないようにする。

問題: TypeResolver は `map_method_call` の存在を知らない。

**推奨: 選択肢 A の変形 — Transformer の `convert_call_args_with_types` で String メソッドの引数型変換を抑制**

`convert_call_args_with_types` は引数を変換する際に expected type を使って `.to_string()` を付加する。String 型のメソッド呼び出しの引数については、**map_method_call が独自の引数変換を行うことが分かっているため**、expected type による `.to_string()` 付加を抑制する。

具体的には: `convert_lit` の `expected` パラメータを `None` にして呼び出す（String メソッドの引数として呼ばれる場合）。

## 関連する既存の問題

### I-290: Transformer/TypeResolver のオーバーロード選択二重実装

I-292 は I-290 と**同根の問題**。TypeResolver と Transformer が独立してメソッド引数に型判断を適用し、結果が矛盾する。I-290 はオーバーロード選択の二重実装、I-292 は引数型変換の二重適用。

根本的には「**TypeResolver が TS ベースのメソッドシグネチャで expected type を設定し、Transformer が Rust ベースのメソッド変換で引数を加工する**」という設計が、2 つの異なるドメイン知識（TS と Rust）を別々のパイプラインステージに分散させていることが問題。

### 波及分析: String 以外の型

**map_method_call で引数を加工する全メソッドを体系的に分析した結果、String 固有ではない。**

影響を受けるメソッド（ビルトインあり expected type = `RustType::String` が `.to_string()` を誘発し、Rust の `Pattern` trait 要件と矛盾）:

| メソッド | Rust 変換先 | 引数処理 | 衝突 |
|---------|-----------|---------|------|
| `includes` | `contains` | `Ref(arg)` | ❌ `&String` は `Pattern` 未実装 |
| `startsWith` | `starts_with` | そのまま | ❌ `String` は `Pattern` 未実装（stable） |
| `endsWith` | `ends_with` | そのまま | ❌ 同上 |
| `split` | `split` | そのまま | ❌ 同上 |
| `replace` | `replacen` | そのまま | ❌ 同上 |
| `replaceAll` | `replace` | そのまま | ❌ 同上 |
| regex `test` | `is_match` | `Ref(arg)` | ❌ `&String` ≠ `&str` |
| regex `exec` | `captures` | `Ref(arg)` | ❌ 同上 |

**配列メソッドへの波及**:
- `indexOf` の比較対象 `x` に `.to_string()` が付加される場合 — `*item == "x".to_string()` は `String == String` で動作するが**不要な allocation**（品質問題）
- `reduce` の `init` 引数 — 型によっては不要な変換
- `push`/`unshift` — map_method_call で変換されないため問題なし

**問題の本質は「Pattern trait を使う Rust メソッド」全般**: String に限らず、Rust のメソッドが `impl Pattern` や `&str` を期待するのに、TypeResolver が TS のシグネチャから `RustType::String` を expected type として設定する全ケースで発生する。

## 参照

- `src/transformer/expressions/literals.rs:34` — `.to_string()` 付加の判断
- `src/transformer/expressions/methods.rs:49-63` — `includes`/`startsWith`/`endsWith` の Rust マッピング
- `src/pipeline/type_resolver/call_resolution.rs:168-197` — `propagate_call_arg_expected_types`
- `src/external_types/mod.rs:324` — `ExternalType::String => RustType::String`
- `src/builtin_types/ecmascript.json` — String.includes シグネチャ
