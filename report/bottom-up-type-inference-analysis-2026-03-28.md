# ボトムアップ型推論: Sink-Source 逆伝播モデル

**初回作成**: 2026-03-28  
**最終更新**: 2026-04-05（レポート最適化）

---

## 概要

パイプライン全体で `Any`（→`serde_json::Value`）、`HashMap<String, V>`、`Unknown`、`todo!()` 等の不正確な型が付与されるケースを統一原理で改善するための設計分析。

> **統一原理**: 「値が最終的に流れ込む先の型（sink type）から、その値の expected type を逆伝播する」

---

## Sink-Source 逆伝播モデル

```
Source（値の生成箇所）  ─── 値の流れ ───→  Sink（値の消費箇所）
                                              │
                                              │ 型情報の逆伝播
                                              ↓
                                        Source の expected type を推論
```

---

## 10 の Sink パターン

| # | Sink パターン | 型情報の源泉 | 状態 |
|---|-------------|------------|------|
| **S1** | return 文 | 関数戻り値型注釈 | 部分実装（明示的注釈のみ。callback 戻り値型の伝播なし） |
| **S2** | `\|\|`/`??` の右辺 | 左辺の解決済み型 | 部分実装（I-286c S2: 型パラメータ制約解決） |
| **S3** | 関数/メソッド引数 | パラメータ型シグネチャ | 部分実装（I-286c S3: 明示的/推論型引数。ジェネリクス具体化は不完全） |
| **S4** | 配列メソッド引数 | `Vec<T>` の要素型 | ✅ 実装済み（I-289, B-0a） |
| **S5** | 代入 RHS | LHS の型（変数型 or フィールド型） | 未着手 |
| **S6** | `as T` 式 | T の型 | 未着手 |
| **S7** | typeof/instanceof ガード | ガード文字列/クラス名 | 未着手（narrowing 基盤は Batch 5b で整備済み） |
| **S8** | プロパティアクセス | フィールド名 + TypeRegistry | 未着手 |
| **S9** | 三項演算子の対分岐 | もう片方の分岐型 | 未着手 |
| **S10** | コールバックパラメータ | 親関数のパラメータ型のシグネチャ | 未着手 |

### 実装優先度

| 優先度 | Sink | 理由 |
|--------|------|------|
| **最高** | S10 | 単一サイト修正で callback 本体全体の型精度向上。Hono ミドルウェアパターンに広く適用。S1 の大部分も自動解消 |
| **高** | S1 | S10 と重複（callback 内 return）。S10 で大部分解消後、残りは少数 |
| **高** | S5 | 低難易度で即効果（代入 RHS → LHS 型伝播） |
| **高** | S6 | 低難易度（`as T` の T を逆伝播） |
| **中** | S9 | 低難易度（三項演算子の対分岐型） |
| **中** | S7 | narrowing 基盤（Batch 5b）上に構築 |
| **中** | S8 | フィールド名 + TypeRegistry でプロパティ型推論 |
| **低** | S2 | ジェネリクス制約解決は高難易度 |
| **低** | S3 | 型引数推論のフィードバック（I-311）に依存 |

---

## カスケード分析

フォールバック型は連鎖的に伝播する。1 箇所の改善が広範囲に効く構造。

### カスケード 1: パラメータ → 関数本体全体

```
パラメータ注釈なし → RustType::Any
  → スコープに Any として登録
  → param.field → Unknown（Any のフィールド型不明）
  → param.method() → Unknown（Any のメソッド不明）
  → fn(param) → 引数の expected type が Any → 下流も Any
  → 最終出力: serde_json::Value が連鎖
```

**S10（コールバックパラメータ）で改善**: 親関数のシグネチャからパラメータ型を推論すれば、本体全体の型が正確に。

### カスケード 2: null リテラル → Option の inner 型

```
null → Option(Any)
  → const x = expr || null → Union(T, Option(Any))
  → 不要に複雑な union enum 生成
```

**expected type が利用可能なら**: `null as string | null` → `Option<String>` で正確に。

### カスケード 3: mapped type → 全使用箇所

```
TsMappedType → HashMap<String, V>
  → Simplify<{ name: string }> → HashMap<String, Value>（正解は T）
  → x.name → HashMap の get → Option<&Value>（正解は String）
```

**identity 検出（実装済み）で P1 は解消**。P2-P5 は残存。

### カスケード 4: コールバックパラメータ → 本体全体

```
arr.map(item => item.name)
  → arr: Vec<SomeStruct>（解決済み）
  → item の型注釈なし → Any
  → item.name → Unknown
```

**S10 の実装で解消可能**: `map` のシグネチャから callback パラメータ型を推論。

---

## 設計方針

### シングルパス内で実装可能

既存の TypeResolver のシングルパスを維持しつつ、`propagate_expected` の拡張として実装。2 パス目は不要。

各 Sink パターンは既存の visit ポイントに分岐を追加するだけ:
- S1: `visit_return_stmt` で callback 戻り値型を設定
- S3: `set_call_arg_expected_types` でジェネリクス具体化
- S5: `visit_assign_expr` で LHS 型 → RHS expected
- S10: `set_call_arg_expected_types` で callback パラメータ型を設定

### 2 パスが必要なケース

`const x = {}; x.field = value` のように宣言後の使用パターンから逆推論する場合のみ。Hono ベンチマークでは 2 件のみで費用対効果が低い。

---

## 関連

- expected type 設定: `src/pipeline/type_resolver/call_resolution.rs` (`set_call_arg_expected_types`)
- 型解決: `src/pipeline/type_resolver/expressions.rs`
- スコープ管理: `src/pipeline/type_resolver/visitors.rs`
- TODO: I-311（型引数推論フィードバック）、I-300/I-301/I-306（OBJECT_LITERAL_NO_TYPE）
