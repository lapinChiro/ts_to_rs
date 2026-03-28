# I-290: メソッド呼び出しの TypeResolver/Transformer 二重ロジック — 設計分析

**日付**: 2026-03-29
**Base commit**: 2e87934（未コミット変更あり）

## 要約

メソッド呼び出しの変換において、TypeResolver と Transformer が独立した判断ロジックを持ち、3 つの具体的な問題を引き起こしている:
1. オーバーロード選択の二重実装
2. メソッドシグネチャ取得の二重実装
3. 引数型変換の衝突（I-292）

## 問題の構造

### 現在のメソッド呼び出しフロー

```
TypeScript: s.includes("x")

Phase 1: TypeResolver
  set_call_arg_expected_types → lookup_method_params(String, "includes")
  → params = [("searchString", RustType::String)]
  → expected_types["x"] = RustType::String

Phase 2: Transformer
  convert_call_expr → method_sig = lookup from Named only (no Vec/String mapping)
  → convert_call_args_with_types → convert_expr("x") reads expected=String → "x".to_string()
  → map_method_call("includes", ["x".to_string()]) → s.contains(&"x".to_string())
  → コンパイルエラー: &String は Pattern trait 未実装
```

### 問題 1: オーバーロード選択

| 項目 | TypeResolver | Transformer |
|------|-------------|-------------|
| コード | `helpers.rs:select_overload` | `calls.rs:117-120` |
| ロジック | 5 段階選択 | `params.len() == arg_count` + first fallback |
| rest 対応 | あり | なし |
| 型互換チェック | Stage 4（ただし引数未解決で無効） | なし |

### 問題 2: メソッドシグネチャ取得

| 項目 | TypeResolver | Transformer |
|------|-------------|-------------|
| コード | `call_resolution.rs:lookup_method_sigs` | `calls.rs:112-124` |
| Vec 対応 | あり（Vec→Array マッピング） | なし（Named 型のみ） |
| String 対応 | あり（"String"→String interface） | なし（Named 型のみ） |
| instantiate | あり | なし（get のみ） |

### 問題 3: 引数型変換の衝突（I-292）

`map_method_call` が引数を加工し、かつ TypeResolver の expected type と矛盾するメソッド:

| TS メソッド | Rust 変換先 | 引数加工 | Rust 期待型 | TS expected → 変換 | 衝突 |
|-----------|-----------|---------|-----------|-------------------|------|
| includes | contains | Ref(arg) | impl Pattern | String → .to_string() | Y |
| startsWith | starts_with | そのまま | impl Pattern | String → .to_string() | Y |
| endsWith | ends_with | そのまま | impl Pattern | String → .to_string() | Y |
| split | split | そのまま | impl Pattern | union | Y |
| replace | replacen | そのまま | impl Pattern | union | Y |
| replaceAll | replace | そのまま | impl Pattern | String → .to_string() | Y |
| test(regex) | is_match | Ref(arg) | &str | String → .to_string() | Y |
| exec(regex) | captures | Ref(arg) | &str | String → .to_string() | Y |
| join | join | 条件付き Ref | &str | String → .to_string() | Y |

全 9 メソッドが影響。Pattern trait は Rust stable で String に未実装。

## 根本原因

メソッド引数の型は 2 箇所で独立に決定される:
- TypeResolver: TS のメソッドシグネチャ（ecmascript.json）から expected type 設定 → `.to_string()` 誘発
- Transformer の `map_method_call`: Rust API に基づき `Ref()` 等を追加

同じ知識（メソッド引数の最終的な Rust 型）が 2 つのパイプラインステージに分散。

## 修正設計

### Step 1: メソッドシグネチャ取得の統一

Transformer の `calls.rs:112-124` を、TypeResolver と同じ能力を持つ統一ヘルパーに置き換え:
- Vec→Array マッピング（TypeResolver の `lookup_method_sigs` と同等）
- String→String interface（同上）
- `registry.instantiate(name, type_args)` 対応
- `select_overload` 使用（Transformer 独自の簡易ロジック廃止）

具体的には `Transformer::lookup_method_sig` メソッドを新設。`TypeResolver::lookup_method_sigs` と`select_overload` のロジックは registry モジュールに移動して共有するのが理想だが、まずは Transformer 側で同等ロジックを呼ぶ。

### Step 2: 引数 expected type の抑制（I-292 解消）

`map_method_call` が Rust API にマッピングするメソッドの引数については、TypeResolver が設定した expected type による `.to_string()` 付加を抑制する。

方法: `convert_call_args_with_types` に `suppress_string_expected: bool` パラメータを追加。
- true の場合、引数が文字列リテラルなら expected type を `None` として扱う
- `map_method_call` で引数を加工するメソッド（Pattern trait 系 + Ref 系）の場合に true を設定

### Step 3: TypeResolver の引数解決順序修正

`call_resolution.rs:72` の `collect_resolved_arg_types` が引数解決前に呼ばれる問題を修正:
- Member callee の場合: `set_call_arg_expected_types` → 引数 resolve → `collect_resolved_arg_types` → `resolve_method_return_type`

## 参照

- `src/transformer/expressions/calls.rs:112-134` — Transformer のメソッド呼び出し処理
- `src/transformer/expressions/methods.rs:46-435` — map_method_call
- `src/transformer/expressions/literals.rs:34` — .to_string() 付加
- `src/pipeline/type_resolver/call_resolution.rs:100-200` — TypeResolver のメソッド引数処理
- `src/pipeline/type_resolver/helpers.rs:136` — select_overload
- `report/i-292-string-method-args-type-conflict-2026-03-29.md` — I-292 根本原因分析
