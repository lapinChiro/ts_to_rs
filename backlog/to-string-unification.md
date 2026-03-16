# .to_string() 変換の統一

対象 TODO: I-92, I-88, I-67

## 背景・動機

3 件の変換バグが共通の根本原因を持つ: 文字列リテラルを `String` 型のパラメータ/フィールドに渡す際に `.to_string()` が付与されない。

1. **I-92**: intersection type で合成された struct の初期化で `.to_string()` が付かない
2. **I-88**: コンストラクタの String 引数に `.to_string()` が付かない
3. **I-67**: クロージャの String パラメータにリテラルを渡すと型不一致

根本原因は TypeRegistry/TypeEnv に型情報が不足しており、`convert_expr_with_expected` に正しい `expected` 型が渡されないこと。

## ゴール

- intersection type の struct 初期化で String フィールドに `.to_string()` が付く
- コンストラクタ呼び出しで String 引数に `.to_string()` が付く
- クロージャ呼び出しで String パラメータに `.to_string()` が付く
- 各修正に対応する E2E テストが PASS する

## スコープ

### 対象

- I-92: intersection type の struct 定義を TypeRegistry に登録するロジックの修正
- I-88: `convert_new_expr` で TypeRegistry からコンストラクタの引数型を取得し `convert_expr_with_expected` に渡す
- I-67: arrow 式変換時に `Fn { params, return_type }` 型を推論し `Stmt::Let` の `ty` に設定。TypeEnv に登録

### 対象外

- TypeRegistry の全面的な再設計
- 非 String 型の型ガイド変換（`number` → `as f64` キャスト等）

## 設計

### 技術的アプローチ

#### I-92: intersection struct の TypeRegistry 登録

`convert_intersection_type` で合成 struct を生成する際、そのフィールド情報を TypeRegistry に登録する。現在は struct の IR は生成しているが TypeRegistry への登録が欠落。

#### I-88: コンストラクタ引数型

`convert_new_expr` で TypeRegistry から型名のフィールド情報（= コンストラクタの引数型）を取得し、各引数を `convert_expr_with_expected` で変換。

#### I-67: クロージャ型の TypeEnv 登録

`convert_arrow_expr` で `Closure` を返す際、パラメータ型と戻り値型から `RustType::Fn` を構築し、親の `Stmt::Let` の `ty` フィールドに設定。これにより TypeEnv に `Fn` 型が登録され、呼び出しサイトで引数型の lookup が可能になる。

### 影響範囲

- `src/transformer/types/mod.rs` — I-92（intersection 型の TypeRegistry 登録）
- `src/transformer/expressions/mod.rs` — I-88（new 式）、I-67（arrow 式）
- `src/registry.rs` — TypeRegistry への登録 API（必要に応じて）
- `tests/e2e/scripts/intersection_type.ts` — I-92 E2E 拡充
- `tests/e2e/scripts/` — I-88, I-67 E2E テスト

## 作業ステップ

- [ ] ステップ 1: I-92 — intersection struct の TypeRegistry 登録。ユニットテスト + `intersection_type.ts` E2E 拡充
- [ ] ステップ 2: I-88 — コンストラクタ引数型 lookup。ユニットテスト + E2E テスト
- [ ] ステップ 3: I-67 — arrow 式の Fn 型推論 + TypeEnv 登録。ユニットテスト + E2E テスト
- [ ] ステップ 4: 全テスト退行チェック

## テスト計画

- **I-92**: `Person { name: "Alice".to_string(), age: 30.0 }` が生成されること（intersection 由来の struct）
- **I-88**: `Foo::new("hello".to_string())` が生成されること
- **I-67**: `greet("World".to_string())` が生成されること（クロージャ経由の呼び出し）

## 完了条件

- [ ] 3 件すべて修正済み、E2E テスト PASS
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
