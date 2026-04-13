//! `convert_call_expr` 系テスト。カテゴリ別に分割:
//!
//! - [`basic_tests`]: 単純な関数呼び出し / メソッド呼び出し / `new` 式 / 基本 console.log
//! - [`console_log_tests`]: `console.log` の `Option<T>` アンラップ動作
//! - [`rest_params_tests`]: rest parameter packing / default 引数補完 / typeenv 経由の expected type 伝搬
//! - [`type_ref_tests`]: paren/chained/opt-chain + I-375 `CallTarget.type_ref` 分類

mod basic_tests;
mod callable_interface_tests;
mod console_log_tests;
mod rest_params_tests;
mod type_ref_tests;
