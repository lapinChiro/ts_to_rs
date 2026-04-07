//! 構造化された Rust pattern IR ノード。
//!
//! `MatchArm::patterns` および `Stmt::IfLet` / `Stmt::WhileLet` /
//! `Expr::IfLet` / `Expr::Matches` の `pattern` フィールドで使用される。
//!
//! # 設計方針
//!
//! - IR は **構造化データ** のみを保持し、display-formatted 文字列は保存しない
//!   （`.claude/rules/pipeline-integrity.md`）。文字列化は generator の
//!   `render_pattern` の責務
//! - `path` は `Vec<String>`（`::` 結合前のセグメント列）。これにより walker が
//!   `path.first()` / `path.last()` で enum 名 / variant 名に直接アクセス可能
//! - `UnitStruct` と `TupleStruct { fields: vec![] }` は区別する:
//!   前者 → `None`、後者 → `None()` と rendering 差が明示される
//! - `Binding::subpat` は `x @ 1..=5` 等のサブパターン束縛用
//!
//! I-377 以前は `MatchPattern::Verbatim(String)` / `Stmt::IfLet::pattern: String`
//! 等として文字列を保持しており、walker が uppercase-head ヒューリスティックで
//! type 参照を抽出していた。本モジュールはその broken window を構造化により解消する。

use super::Expr;

/// Rust pattern grammar を構造化表現した IR ノード。
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` — ワイルドカード。
    Wildcard,

    /// 値リテラルパターン（`1`, `"hello"`, `true`, `3.14`）。
    ///
    /// # 不変条件
    ///
    /// `Literal` は **純粋な値リテラル** (`Expr::IntLit` / `Expr::NumberLit` /
    /// `Expr::StringLit` / `Expr::BoolLit`) のみを保持する。enum variant 名や
    /// 修飾パス (`Direction::Up`, `Color::Red`) は `UnitStruct` / `TupleStruct`
    /// で表現すること。
    ///
    /// # 根拠
    ///
    /// `Expr::Ident(String)` に修飾パス文字列を埋め込む設計は
    /// pipeline-integrity ルール「IR に display-formatted 文字列を保存禁止」
    /// に違反する broken window であり、I-377 で撲滅される。`Literal` に
    /// `Expr::Ident` が入る過渡的コード（switch.rs の `try_convert_*`）は
    /// Phase 2 で `UnitStruct` / `TupleStruct` に置換される。
    Literal(Expr),

    /// 変数束縛（`x`, `mut x`, `x @ 1..=5`）。
    Binding {
        /// 束縛する変数名
        name: String,
        /// `mut` 修飾子の有無
        is_mut: bool,
        /// `@` サブパターン（`x @ Foo(_)` 等）
        subpat: Option<Box<Pattern>>,
    },

    /// タプル構造体 / タプル variant（`Some(x)`, `Color::Red(x, y)`, `Ok(v)`, `Err(e)`）。
    ///
    /// `path` は `::` 結合前のセグメント列。`fields` はタプル要素のサブパターン。
    TupleStruct {
        /// `::` 結合前のパスセグメント列（例: `["Some"]`, `["Color", "Red"]`）
        path: Vec<String>,
        /// タプル要素のサブパターン
        fields: Vec<Pattern>,
    },

    /// 構造体 / struct variant（`Shape::Circle { radius, .. }`, `Foo { x, y }`）。
    Struct {
        /// `::` 結合前のパスセグメント列
        path: Vec<String>,
        /// 名前付きフィールドとサブパターンの対
        fields: Vec<(String, Pattern)>,
        /// 末尾の `..` 有無
        rest: bool,
    },

    /// Unit variant / unit struct（`None`, `Color::Green`）。
    UnitStruct {
        /// `::` 結合前のパスセグメント列
        path: Vec<String>,
    },

    /// Or パターン（`a | b | c`）。
    Or(Vec<Pattern>),

    /// Range パターン（`1..=5`, `..10`）。
    Range {
        /// 範囲開始（`None` = 開始省略）
        start: Option<Box<Expr>>,
        /// 範囲終了（`None` = 終了省略）
        end: Option<Box<Expr>>,
        /// `..=`（inclusive）なら `true`、`..`（exclusive）なら `false`
        inclusive: bool,
    },

    /// 参照パターン（`&x`, `&mut x`）。
    Ref {
        /// `&mut` なら `true`
        mutable: bool,
        /// 参照されるサブパターン
        inner: Box<Pattern>,
    },

    /// タプルパターン（`(a, b, c)`）。
    Tuple(Vec<Pattern>),
}

impl Pattern {
    /// Unit variant `None` パターンかどうかを判定する。
    ///
    /// `resolve_complement_pattern` が `None` を返すかどうかを構造的に
    /// チェックする用途。従来の `pattern_string == "None"` 文字列比較の
    /// 置き換え。
    pub fn is_none_unit(&self) -> bool {
        matches!(self, Pattern::UnitStruct { path } if path.len() == 1 && path[0] == "None")
    }

    /// 単一セグメントの `Binding` ショートカット（`mut` なし、subpat なし）。
    pub fn binding(name: impl Into<String>) -> Pattern {
        Pattern::Binding {
            name: name.into(),
            is_mut: false,
            subpat: None,
        }
    }

    /// `Some(binding_name)` パターン構築ショートカット。
    pub fn some_binding(name: impl Into<String>) -> Pattern {
        Pattern::TupleStruct {
            path: vec!["Some".to_string()],
            fields: vec![Pattern::binding(name)],
        }
    }

    /// `None` パターン構築ショートカット。
    pub fn none() -> Pattern {
        Pattern::UnitStruct {
            path: vec!["None".to_string()],
        }
    }
}
