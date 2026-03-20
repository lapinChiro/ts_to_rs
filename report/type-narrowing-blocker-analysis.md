# 型 Narrowing (I-69) 実装ブロッカー分析

**基準コミット**: `7756458`
**注記**: 未コミットの narrowing 基盤コード（patterns.rs, statements/mod.rs）がある状態で調査

## 要約

I-69 の narrowing 基盤は構造的に正しく実装されている（TypeEnv の push_scope/pop_scope で分岐内の型を更新）。しかし **4 つのギャップ** により、生成コードに一切の変化が生じない状態で WIP 停止した。

## ギャップ一覧

### ギャップ 1: `resolve_typeof_match` が `Any` 型を `False` として扱う（I-203）

**場所**: `src/transformer/expressions/patterns.rs:216-262`

```rust
fn resolve_typeof_match(ty: &RustType, typeof_str: &str) -> TypeofMatch {
    match typeof_str {
        "string" => {
            if matches!(ty, RustType::String) {
                TypeofMatch::True
            } else {
                TypeofMatch::False  // ← Any がここに落ちる
            }
        }
        // 他の全ケースも同様に Any → False
    }
}
```

**問題**: `x: any` のとき `typeof x === "string"` が `false` に評価される。`Any` は「型不明」であり「string でない」ではない。`TypeofMatch::Placeholder` は `resolve_expr_type` が `None` を返す場合のみ使われ、`Some(RustType::Any)` では使われない。

**影響**: typeof ガード後の then 分岐がデッドコード化し、narrowing が効いても意味がない。

**修正方針**: `resolve_typeof_match` で `RustType::Any` を受け取ったとき `TypeofMatch::Placeholder` を返す。

### ギャップ 2: `narrowed_type_for_else` の NonNullish ロジックが逆

**場所**: `src/transformer/expressions/patterns.rs:474-498`（未コミット）

```rust
NarrowingGuard::NonNullish { is_neq, .. } => {
    if !*is_neq {
        // x === null → else branch: unwrap Option<T> → T  ← 正しいが混乱しやすい
    }
}
```

`x === null` の **else** 分岐で `T` に narrowing するロジック自体は正しいが、コメントが誤解を招く。ただし `instanceof` の追加（NarrowingGuard に InstanceOf バリアントがない）等の不足があり、PRD のスコープを完全にカバーしていない。

### ギャップ 3: TypeEnv の型変更が生成コードに反映される箇所が限定的

調査で特定した **TypeEnv の型情報が生成コードに影響する 11 箇所**:

| # | 場所 | 影響 | narrowing で変化するか |
|---|------|------|----------------------|
| A | `calls.rs:50` 関数パラメータ型 | 引数の型強制 | ✅ 関数名の型が変わればパラメータ型が変わる |
| B | `calls.rs:87` console.log フォーマット | `{:?}` vs `{}` | ✅ String→Display, Any→Debug |
| C | `calls.rs:117` メソッドパラメータ型 | メソッド引数の強制 | ✅ 型が判明すればメソッドルックアップ可能に |
| D | `calls.rs:584` trait deref | `&*expr` 挿入 | ✅ Box<dyn Trait> 判定に影響 |
| E | `assignments.rs:36` 代入 RHS 型 | ExprContext 伝播 | ✅ 型が判明すれば expected 伝播 |
| F | `member_access.rs:82` Optional chain | `.unwrap_or()` 生成 | ✅ Option<T>→T で挙動変化 |
| G | `member_access.rs:253` タプルインデックス | `pair.0` vs `pair[0]` | △ Tuple→Named 等の変化は稀 |
| H | `member_access.rs:296` DU フィールド | match ベース抽出 | ✅ Named 型判定に影響 |
| I | `member_access.rs:317` フィールド clone | `.clone()` 挿入 | △ narrowing で通常は発生しない |
| J | `literals.rs:32` 文字列 .to_string() | `.to_string()` 追加 | ❌ expected 依存、TypeEnv 直接不使用 |
| K | `data_literals.rs:68` オブジェクトフィールド | expected 伝播 | ❌ expected 依存、TypeEnv 直接不使用 |

**結論**: narrowing で TypeEnv を更新すれば、11 箇所中 **7 箇所** で生成コードが変化しうる。基盤は正しく、ギャップ 1（typeof の Any 判定）を修正すれば narrowing の効果が発現する。

### ギャップ 4: `NarrowingGuard` に `InstanceOf` / `Truthy` バリアントがない

**場所**: `src/transformer/expressions/patterns.rs:423-434`（未コミット）

PRD のスコープに含まれる以下が未実装:
- `instanceof` ガード: `x instanceof Foo` → `x: Foo`
- truthy ガード: `if (x)` で `x: Option<T>` → `x: T`
- `typeof_string_to_rust_type` が `"object"` / `"function"` を処理しない

## 根本原因

I-69 が WIP 停止した直接的な原因は **ギャップ 1**（typeof の Any 判定問題）です。テストケース `typeof x === "string"` が `false` に評価されるため、narrowing が効いても then 分岐自体が消滅し、効果が確認できませんでした。

ギャップ 1 を修正すれば:
- typeof ガードの条件が `false` → `true`（Placeholder）に変わる
- then 分岐が生存する
- narrowing で TypeEnv 内の `x` が `String` に更新される
- `x.trim()` 等の変換で型依存コードパスが正しく動作する

## 推奨: PRD の分割

| PRD | 内容 | 依存関係 |
|-----|------|----------|
| **I-203** | `resolve_typeof_match` の Any 型修正 + `instanceof` の Any 型修正 | なし（前提条件） |
| **I-69** | 型 narrowing 本体（既存基盤の完成 + テスト + InstanceOf/Truthy 追加） | I-203 が前提 |

I-203 を先に解消すれば、I-69 のテストが成立し、RED → GREEN サイクルを回せるようになる。
