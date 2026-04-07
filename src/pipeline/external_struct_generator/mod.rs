//! 参照されるビルトイン外部型の struct 定義を自動生成する。
//!
//! 変換出力（IR）内で参照されているが定義が存在しない外部型を検出し、
//! `TypeRegistry` のフィールド情報から `Item::Struct` を生成する。

use std::collections::HashSet;

use crate::ir::{
    camel_to_snake, sanitize_field_name, CallTarget, ClosureBody, Expr, Item, MatchArm, Method,
    Pattern, RustType, Stmt, StructField, TypeParam, Visibility,
};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::resolve::typedef::monomorphize_type_params;

/// Rust の標準ライブラリ型・serde 型など、struct 生成が不要な型名のセット。
///
/// 型（type name）レベルのフィルタ。
///
/// # 歴史と責務分離
///
/// I-375 で `Expr::FnCall` が `CallTarget` で構造化され、FnCall 経由で `Some` /
/// `None` / `Ok` / `Err` が walker に登録されることはなくなった。I-377 で
/// `MatchPattern::Verbatim` と `Stmt::IfLet::pattern: String` を構造化 `Pattern`
/// に置換し、uppercase-head ヒューリスティックを廃止したことで、pattern 経由の
/// 流入も構造的に遮断された。`Some` / `None` / `Ok` / `Err` は型名ではなく
/// `Option` / `Result` の **variant コンストラクタ** であるため、型名フィルタ
/// である本定数からは除外する。
///
/// Pattern constructor の除外は `collect_type_refs_from_pattern` 内部の
/// `PATTERN_LANG_BUILTINS` 定数に閉じている（抽象レベル分離）。
const RUST_BUILTIN_TYPES: &[&str] = &[
    "String", "Vec", "HashMap", "HashSet", "Option", "Box", "Result", "Rc", "Arc", "Mutex", "bool",
    "f64", "i64", "i128", "u8", "u32", "usize",
];

/// `serde_json::Value` のフルパス。
const SERDE_JSON_VALUE: &str = "serde_json::Value";

/// IR items を走査し、参照されているが定義がない外部型名を収集する。
///
/// 外部型（JSON ビルトイン定義）のみを対象とし、ユーザー定義型（TS ソースから登録された型）は除外する。
/// `TypeRegistry::is_external` で外部型かどうかを判定する。
///
/// `scan_context` の役割:
/// - **定義済み判定**: scan_context 内の型は「定義済み」として扱われる。
/// - **参照走査**: scan_context 内の参照も undefined 候補に加える。
///
/// この関数は外部型 struct 生成 ([`generate_external_struct`]) のための候補名を返す。
/// `is_external` フィルタが効くため、ユーザー定義型が誤って取り込まれる心配はない。
/// 合成型のフィールドが参照する外部型を漏れなく検出する目的で scan_context をスキャンする。
///
/// 以下を除外する:
/// - `items`、`scan_context`、`defined_only` 内に既に定義が存在する型
///   （struct/enum/trait/type alias）
/// - Rust 標準ライブラリ型（`String`, `Vec`, `HashMap` 等）
/// - `serde_json::Value`
/// - 外部型でない型（ユーザー定義型）
///
/// `scan_context` は **定義+走査** の追加 items（per-file 合成型など）。
/// `defined_only` は **定義済み判定のみ** に使う items（他ファイルの合成型など）。
/// 走査対象から外すことで、無関係な型まで外部型生成の起点になる雪だるま現象を防ぐ。
pub fn collect_undefined_type_references(
    items: &[Item],
    scan_context: &[Item],
    defined_only: &[Item],
    registry: &TypeRegistry,
) -> HashSet<String> {
    let scope = UndefinedRefScope::new(items, scan_context, defined_only);
    scope
        .collect()
        .into_iter()
        .filter(|name| registry.is_external(name))
        .collect()
}

/// IR items を走査し、参照されているが定義がない型名を **全て** 収集する。
///
/// [`collect_undefined_type_references`] と異なり、`is_external` フィルタを適用しない。
/// shared_types.rs のスタブ生成で使用する — モジュール内の全未定義参照を解決するため。
///
/// `scan_context` は **定義+走査** の追加 items（per-file 合成型など）。
/// `defined_only` は **定義済み判定のみ** に使う items（他ファイルの合成型など）。
/// 走査対象から外すことで、無関係な型までスタブ化される雪だるま現象を防ぐ。
pub fn collect_all_undefined_references(
    items: &[Item],
    scan_context: &[Item],
    defined_only: &[Item],
) -> HashSet<String> {
    UndefinedRefScope::new(items, scan_context, defined_only).collect()
}

/// 未定義型参照の収集ロジック共通骨格。
///
/// `collect_undefined_type_references` と `collect_all_undefined_references` は
/// 「`is_external` フィルタを最後に追加で掛けるかどうか」のみ異なる。骨格は同一:
/// 1. 定義済み・インポート済み・型パラメータ名・標準型・`serde_json::Value`・パス形式
///    (`A::B`) の型名を除外集合に集める
/// 2. `items + scan_context` を walker で歩いて参照名を収集
/// 3. 除外集合を引いた残りを返す
struct UndefinedRefScope<'a> {
    items: &'a [Item],
    scan_context: &'a [Item],
    defined_only: &'a [Item],
}

impl<'a> UndefinedRefScope<'a> {
    fn new(items: &'a [Item], scan_context: &'a [Item], defined_only: &'a [Item]) -> Self {
        Self {
            items,
            scan_context,
            defined_only,
        }
    }

    /// 定義+判定+定義のみ をまとめた iterator。
    fn definition_pool(&self) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .chain(self.scan_context.iter())
            .chain(self.defined_only.iter())
    }

    /// `items + scan_context` を返す（参照走査と型パラメータ収集の共通入力）。
    fn scan_pool(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().chain(self.scan_context.iter())
    }

    fn collect(&self) -> HashSet<String> {
        let defined_types: HashSet<String> = self
            .definition_pool()
            .filter_map(|item| match item {
                Item::Struct { name, .. }
                | Item::Enum { name, .. }
                | Item::Trait { name, .. }
                | Item::TypeAlias { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();

        let imported_types: HashSet<String> = self
            .definition_pool()
            .filter_map(|item| match item {
                Item::Use { names, .. } => Some(names.clone()),
                _ => None,
            })
            .flatten()
            .collect();

        let type_param_names: HashSet<String> = self
            .scan_pool()
            .flat_map(|item| match item {
                Item::Struct { type_params, .. }
                | Item::Trait { type_params, .. }
                | Item::Fn { type_params, .. }
                | Item::Impl { type_params, .. }
                | Item::TypeAlias { type_params, .. } => type_params
                    .iter()
                    .map(|tp| tp.name.clone())
                    .collect::<Vec<_>>(),
                _ => vec![],
            })
            .collect();

        let mut referenced_types = HashSet::new();
        for item in self.scan_pool() {
            collect_type_refs_from_item(item, &mut referenced_types);
        }

        let builtin_set: HashSet<&str> = RUST_BUILTIN_TYPES.iter().copied().collect();

        referenced_types
            .into_iter()
            .filter(|name| !defined_types.contains(name))
            .filter(|name| !imported_types.contains(name))
            .filter(|name| !type_param_names.contains(name))
            .filter(|name| !builtin_set.contains(name.as_str()))
            .filter(|name| name != SERDE_JSON_VALUE)
            // パス形式の型名（例: E::Bindings, serde_json::Value）は struct 名にならない
            .filter(|name| !name.contains("::"))
            .collect()
    }
}

/// 未定義型に対する空スタブ struct を生成し、items に追加する。
///
/// types.rs のコンパイルを通すため、参照されているが定義がない型にスタブを追加する。
/// TypeRegistry に struct 情報がある型はフル生成（[`generate_external_struct`] 経由）、
/// それ以外は空のユニット struct `pub struct TypeName;` を生成する。
/// フル生成した struct が新たな未定義参照を生む場合に備え、固定点に達するまで反復する。
pub fn generate_stub_structs(
    items: &mut Vec<Item>,
    scan_context: &[Item],
    defined_only: &[Item],
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) {
    for _ in 0..10 {
        let undefined = collect_all_undefined_references(items, scan_context, defined_only);
        if undefined.is_empty() {
            break;
        }
        // 出力順序を決定的にするためソート
        let mut sorted: Vec<String> = undefined.into_iter().collect();
        sorted.sort();
        for name in sorted {
            if let Some(full) = generate_external_struct(&name, registry, synthetic) {
                items.push(full);
            } else {
                items.push(Item::Struct {
                    vis: Visibility::Public,
                    name,
                    type_params: vec![],
                    fields: vec![],
                });
            }
        }
    }
}

/// `TypeRegistry` のフィールド情報から外部型の `Item::Struct` を生成する。
///
/// 非 trait 制約を持つ型パラメータはモノモーフィゼーションで除去し、
/// フィールド型に制約型を置換する。
///
/// `TypeDef::Struct` 以外（`TypeDef::Enum`, `TypeDef::Function`）の場合は `None` を返す。
pub fn generate_external_struct(
    name: &str,
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> Option<Item> {
    let typedef = registry.get(name)?;
    match typedef {
        TypeDef::Struct {
            type_params,
            fields,
            ..
        } => {
            // モノモーフィゼーション: 非 trait 制約の型パラメータを具象型に置換
            let (mono_params, mono_subs) =
                monomorphize_type_params(type_params.clone(), registry, synthetic);

            let struct_fields: Vec<StructField> = fields
                .iter()
                .map(|field| {
                    let ty = field.ty.substitute(&mono_subs);
                    // 自己参照フィールドを Box でラップ（再帰型の infinite size 防止）
                    let ty = if references_type_name(&ty, name) {
                        RustType::Named {
                            name: "Box".to_string(),
                            type_args: vec![ty],
                        }
                    } else {
                        ty
                    };
                    StructField {
                        vis: Some(Visibility::Public),
                        name: sanitize_field_name(&camel_to_snake(&field.name)),
                        ty,
                    }
                })
                .collect();

            Some(Item::Struct {
                vis: Visibility::Public,
                name: name.to_string(),
                type_params: mono_params,
                fields: struct_fields,
            })
        }
        TypeDef::Enum { .. } | TypeDef::Function { .. } | TypeDef::ConstValue { .. } => None,
    }
}

/// `RustType` が指定された型名を直接参照しているか判定する。
fn references_type_name(ty: &RustType, target: &str) -> bool {
    match ty {
        RustType::Named { name, type_args } => {
            name == target || type_args.iter().any(|a| references_type_name(a, target))
        }
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            references_type_name(inner, target)
        }
        RustType::Result { ok, err } => {
            references_type_name(ok, target) || references_type_name(err, target)
        }
        RustType::Tuple(elems) => elems.iter().any(|e| references_type_name(e, target)),
        _ => false,
    }
}

/// `Item` 内で参照されている `RustType::Named` 等の型名を再帰的に収集する。
///
/// 走査対象:
/// - `Enum`: variant の data 型と fields
/// - `Struct`: fields
/// - `Fn`: signature (return_type, params) **および body** (Stmt/Expr 内の型参照および
///   `StructInit` の struct 名)
/// - `TypeAlias`: aliased type
/// - `Impl`: for_trait, consts, 各 method の signature と body
/// - `Trait`: supertraits と各 method の signature（trait method 本体は通常 None）
///
/// `Comment` / `RawCode` / `Use` は走査しない。
pub(crate) fn collect_type_refs_from_item(item: &Item, refs: &mut HashSet<String>) {
    match item {
        Item::Enum {
            type_params,
            variants,
            ..
        } => {
            collect_type_refs_from_type_params(type_params, refs);
            for variant in variants {
                if let Some(data) = &variant.data {
                    collect_type_refs_from_rust_type(data, refs);
                }
                for field in &variant.fields {
                    collect_type_refs_from_rust_type(&field.ty, refs);
                }
            }
        }
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            collect_type_refs_from_type_params(type_params, refs);
            for field in fields {
                collect_type_refs_from_rust_type(&field.ty, refs);
            }
        }
        Item::Fn {
            type_params,
            return_type,
            params,
            body,
            ..
        } => {
            collect_type_refs_from_type_params(type_params, refs);
            if let Some(rt) = return_type {
                collect_type_refs_from_rust_type(rt, refs);
            }
            for param in params {
                if let Some(ty) = &param.ty {
                    collect_type_refs_from_rust_type(ty, refs);
                }
            }
            collect_type_refs_from_stmts(body, refs);
        }
        Item::TypeAlias {
            type_params, ty, ..
        } => {
            collect_type_refs_from_type_params(type_params, refs);
            collect_type_refs_from_rust_type(ty, refs);
        }
        Item::Impl {
            type_params,
            for_trait,
            consts,
            methods,
            ..
        } => {
            collect_type_refs_from_type_params(type_params, refs);
            if let Some(tref) = for_trait {
                refs.insert(tref.name.clone());
                for arg in &tref.type_args {
                    collect_type_refs_from_rust_type(arg, refs);
                }
            }
            for c in consts {
                collect_type_refs_from_rust_type(&c.ty, refs);
                collect_type_refs_from_expr(&c.value, refs);
            }
            for method in methods {
                collect_type_refs_from_method(method, refs);
            }
        }
        Item::Trait {
            type_params,
            methods,
            supertraits,
            ..
        } => {
            collect_type_refs_from_type_params(type_params, refs);
            for sup in supertraits {
                refs.insert(sup.name.clone());
                for arg in &sup.type_args {
                    collect_type_refs_from_rust_type(arg, refs);
                }
            }
            for method in methods {
                collect_type_refs_from_method(method, refs);
            }
        }
        Item::Use { .. } | Item::Comment(_) | Item::RawCode(_) => {}
    }
}

/// 型パラメータ列の constraint（trait bound）から型参照を収集する。
///
/// 例: `<T: SomeTrait>` の `SomeTrait`、`<T: Container<Inner>>` の `Container` と `Inner`
/// を refs に登録する。型パラメータ名 (`T`) 自体は `UndefinedRefScope` の
/// `type_param_names` で後段除外されるため、ここでは登録しない。
fn collect_type_refs_from_type_params(type_params: &[TypeParam], refs: &mut HashSet<String>) {
    for tp in type_params {
        if let Some(constraint) = &tp.constraint {
            collect_type_refs_from_rust_type(constraint, refs);
        }
    }
}

fn collect_type_refs_from_method(method: &Method, refs: &mut HashSet<String>) {
    if let Some(rt) = &method.return_type {
        collect_type_refs_from_rust_type(rt, refs);
    }
    for param in &method.params {
        if let Some(ty) = &param.ty {
            collect_type_refs_from_rust_type(ty, refs);
        }
    }
    if let Some(body) = &method.body {
        collect_type_refs_from_stmts(body, refs);
    }
}

/// `Vec<Stmt>` を走査して全ての型参照を収集する。
pub(crate) fn collect_type_refs_from_stmts(stmts: &[Stmt], refs: &mut HashSet<String>) {
    for stmt in stmts {
        collect_type_refs_from_stmt(stmt, refs);
    }
}

fn collect_type_refs_from_stmt(stmt: &Stmt, refs: &mut HashSet<String>) {
    match stmt {
        Stmt::Let { ty, init, .. } => {
            if let Some(t) = ty {
                collect_type_refs_from_rust_type(t, refs);
            }
            if let Some(e) = init {
                collect_type_refs_from_expr(e, refs);
            }
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            collect_type_refs_from_expr(condition, refs);
            collect_type_refs_from_stmts(then_body, refs);
            if let Some(eb) = else_body {
                collect_type_refs_from_stmts(eb, refs);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            collect_type_refs_from_expr(condition, refs);
            collect_type_refs_from_stmts(body, refs);
        }
        Stmt::WhileLet {
            pattern,
            expr,
            body,
            ..
        } => {
            collect_type_refs_from_pattern(pattern, refs);
            collect_type_refs_from_expr(expr, refs);
            collect_type_refs_from_stmts(body, refs);
        }
        Stmt::ForIn { iterable, body, .. } => {
            collect_type_refs_from_expr(iterable, refs);
            collect_type_refs_from_stmts(body, refs);
        }
        Stmt::Loop { body, .. } | Stmt::LabeledBlock { body, .. } => {
            collect_type_refs_from_stmts(body, refs);
        }
        Stmt::Break { value, .. } => {
            if let Some(v) = value {
                collect_type_refs_from_expr(v, refs);
            }
        }
        Stmt::Continue { .. } => {}
        Stmt::Return(opt) => {
            if let Some(e) = opt {
                collect_type_refs_from_expr(e, refs);
            }
        }
        Stmt::Expr(e) | Stmt::TailExpr(e) => collect_type_refs_from_expr(e, refs),
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            collect_type_refs_from_pattern(pattern, refs);
            collect_type_refs_from_expr(expr, refs);
            collect_type_refs_from_stmts(then_body, refs);
            if let Some(eb) = else_body {
                collect_type_refs_from_stmts(eb, refs);
            }
        }
        Stmt::Match { expr, arms } => {
            collect_type_refs_from_expr(expr, refs);
            for arm in arms {
                collect_type_refs_from_match_arm(arm, refs);
            }
        }
    }
}

/// 構造化 `Pattern` から型参照を収集する。
///
/// `TupleStruct` / `Struct` / `UnitStruct` の `path` 先頭セグメントが型名であり、
/// refs に登録する。ただし `Some` / `None` / `Ok` / `Err` は Rust 言語レベル
/// 組み込み（`Option` / `Result` のコンストラクタ）であり、外部型 struct 生成の
/// 対象ではないため除外する。
///
/// I-377 以前は `pattern: String` の uppercase-head ヒューリスティックに依存
/// していたが、構造化 `Pattern::TupleStruct { path: Vec<String>, .. }` への
/// 移行により `path.first()` の直接比較になり、lowercase クラス名でも正しく
/// 捕捉できるようになった（false negative 消滅）。
fn collect_type_refs_from_pattern(pattern: &Pattern, refs: &mut HashSet<String>) {
    /// `Option` / `Result` の variant コンストラクタ。これらは Rust 組み込みで
    /// あり外部型として stub 生成する対象ではない。`RUST_BUILTIN_TYPES`（型名
    /// フィルタ）とは抽象レベルが異なるため別定数として管理する。
    const PATTERN_LANG_BUILTINS: &[&str] = &["Some", "None", "Ok", "Err"];

    let head = match pattern {
        Pattern::TupleStruct { path, .. }
        | Pattern::Struct { path, .. }
        | Pattern::UnitStruct { path } => path.first(),
        _ => None,
    };
    if let Some(first) = head {
        if !PATTERN_LANG_BUILTINS.contains(&first.as_str()) {
            refs.insert(first.clone());
        }
    }
    // 子ノードを再帰走査
    match pattern {
        Pattern::Literal(e) => collect_type_refs_from_expr(e, refs),
        Pattern::Binding { subpat, .. } => {
            if let Some(sub) = subpat {
                collect_type_refs_from_pattern(sub, refs);
            }
        }
        Pattern::TupleStruct { fields, .. } => {
            for f in fields {
                collect_type_refs_from_pattern(f, refs);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_type_refs_from_pattern(p, refs);
            }
        }
        Pattern::Or(pats) | Pattern::Tuple(pats) => {
            for p in pats {
                collect_type_refs_from_pattern(p, refs);
            }
        }
        Pattern::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_type_refs_from_expr(s, refs);
            }
            if let Some(e) = end {
                collect_type_refs_from_expr(e, refs);
            }
        }
        Pattern::Ref { inner, .. } => collect_type_refs_from_pattern(inner, refs),
        Pattern::Wildcard | Pattern::UnitStruct { .. } => {}
    }
}

/// `MatchArm` を走査して型参照を収集する。
fn collect_type_refs_from_match_arm(arm: &MatchArm, refs: &mut HashSet<String>) {
    for pattern in &arm.patterns {
        collect_type_refs_from_pattern(pattern, refs);
    }
    if let Some(g) = &arm.guard {
        collect_type_refs_from_expr(g, refs);
    }
    collect_type_refs_from_stmts(&arm.body, refs);
}

fn collect_type_refs_from_expr(expr: &Expr, refs: &mut HashSet<String>) {
    match expr {
        // 型情報を含むリーフ
        Expr::Cast { expr, target } => {
            collect_type_refs_from_expr(expr, refs);
            collect_type_refs_from_rust_type(target, refs);
        }
        Expr::StructInit { name, fields, base } => {
            // `Self` は impl 文脈の implicit type. RustType walker と同じ方針で除外する。
            if name != "Self" {
                refs.insert(name.clone());
            }
            for (_, e) in fields {
                collect_type_refs_from_expr(e, refs);
            }
            if let Some(b) = base {
                collect_type_refs_from_expr(b, refs);
            }
        }
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            for p in params {
                if let Some(t) = &p.ty {
                    collect_type_refs_from_rust_type(t, refs);
                }
            }
            if let Some(rt) = return_type {
                collect_type_refs_from_rust_type(rt, refs);
            }
            match body {
                ClosureBody::Expr(e) => collect_type_refs_from_expr(e, refs),
                ClosureBody::Block(stmts) => collect_type_refs_from_stmts(stmts, refs),
            }
        }
        // 再帰サブ式
        Expr::FieldAccess { object, .. } => collect_type_refs_from_expr(object, refs),
        Expr::MethodCall { object, args, .. } => {
            collect_type_refs_from_expr(object, refs);
            for a in args {
                collect_type_refs_from_expr(a, refs);
            }
        }
        Expr::Assign { target, value } => {
            collect_type_refs_from_expr(target, refs);
            collect_type_refs_from_expr(value, refs);
        }
        Expr::UnaryOp { operand, .. } => collect_type_refs_from_expr(operand, refs),
        Expr::BinaryOp { left, right, .. } => {
            collect_type_refs_from_expr(left, refs);
            collect_type_refs_from_expr(right, refs);
        }
        Expr::Range { start, end } => {
            if let Some(s) = start {
                collect_type_refs_from_expr(s, refs);
            }
            if let Some(e) = end {
                collect_type_refs_from_expr(e, refs);
            }
        }
        Expr::FnCall { target, args } => {
            // I-375: structural call-target classification.
            //
            // The Transformer records every `Expr::FnCall` with a structured
            // `CallTarget`:
            //   - `CallTarget::Path { type_ref: Some(t), .. }` — the call references
            //     the user-defined type `t` (e.g. `Color::Red(x)`, `MyClass::new(x)`).
            //     Register `t` in the reference graph.
            //   - `CallTarget::Path { type_ref: None, .. }` — free function, module
            //     path call, `Option`/`Result` builtin variant, or local variable
            //     invocation. No type reference to record.
            //   - `CallTarget::Super` — parent constructor call in class inheritance
            //     context. No type reference.
            //
            // The previous implementation used an uppercase-head heuristic on the
            // joined path string, which produced false negatives for lowercase
            // class names (`class myClass {}`) and relied on hardcoding
            // `Some/None/Ok/Err` into `RUST_BUILTIN_TYPES`. Both band-aids are
            // removed now that the classification is structural.
            if let CallTarget::Path {
                type_ref: Some(t), ..
            } = target
            {
                refs.insert(t.clone());
            }
            for a in args {
                collect_type_refs_from_expr(a, refs);
            }
        }
        Expr::Vec { elements } | Expr::Tuple { elements } => {
            for e in elements {
                collect_type_refs_from_expr(e, refs);
            }
        }
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_type_refs_from_expr(condition, refs);
            collect_type_refs_from_expr(then_expr, refs);
            collect_type_refs_from_expr(else_expr, refs);
        }
        Expr::IfLet {
            pattern,
            expr,
            then_expr,
            else_expr,
        } => {
            collect_type_refs_from_pattern(pattern, refs);
            collect_type_refs_from_expr(expr, refs);
            collect_type_refs_from_expr(then_expr, refs);
            collect_type_refs_from_expr(else_expr, refs);
        }
        Expr::FormatMacro { args, .. } | Expr::MacroCall { args, .. } => {
            for a in args {
                collect_type_refs_from_expr(a, refs);
            }
        }
        Expr::Await(e) | Expr::Deref(e) | Expr::Ref(e) => collect_type_refs_from_expr(e, refs),
        Expr::Index { object, index } => {
            collect_type_refs_from_expr(object, refs);
            collect_type_refs_from_expr(index, refs);
        }
        Expr::RuntimeTypeof { operand } => collect_type_refs_from_expr(operand, refs),
        Expr::Matches { expr, pattern } => {
            collect_type_refs_from_expr(expr, refs);
            collect_type_refs_from_pattern(pattern, refs);
        }
        Expr::Block(stmts) => collect_type_refs_from_stmts(stmts, refs),
        Expr::Match { expr, arms } => {
            collect_type_refs_from_expr(expr, refs);
            for arm in arms {
                collect_type_refs_from_match_arm(arm, refs);
            }
        }
        // 型参照を持たないリーフ
        Expr::NumberLit(_)
        | Expr::IntLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::RawCode(_)
        | Expr::Regex { .. } => {}
    }
}

/// `RustType` を再帰的に走査し、`Named` の型名を収集する。
///
/// `Self` は impl block 文脈の implicit type であり、struct 名として stub 生成しても
/// 意味がない（`pub struct Self {}` は Rust の予約語衝突でコンパイル不可）。よって
/// 走査結果から除外する。
pub(crate) fn collect_type_refs_from_rust_type(ty: &RustType, refs: &mut HashSet<String>) {
    match ty {
        RustType::Named { name, type_args } => {
            if name != "Self" {
                refs.insert(name.clone());
            }
            for arg in type_args {
                collect_type_refs_from_rust_type(arg, refs);
            }
        }
        RustType::QSelf {
            qself,
            trait_ref,
            item: _,
        } => {
            // 限定パス `<qself as Trait<args>>::Item` は、Trait 名と qself / 引数を
            // それぞれ参照として収集する。Item 名は trait 内 associated type であり
            // 独立した型ではないため refs には追加しない。
            collect_type_refs_from_rust_type(qself, refs);
            refs.insert(trait_ref.name.clone());
            for arg in &trait_ref.type_args {
                collect_type_refs_from_rust_type(arg, refs);
            }
        }
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            collect_type_refs_from_rust_type(inner, refs);
        }
        RustType::Result { ok, err } => {
            collect_type_refs_from_rust_type(ok, refs);
            collect_type_refs_from_rust_type(err, refs);
        }
        RustType::Tuple(elems) => {
            for elem in elems {
                collect_type_refs_from_rust_type(elem, refs);
            }
        }
        RustType::Fn {
            params,
            return_type,
        } => {
            for param in params {
                collect_type_refs_from_rust_type(param, refs);
            }
            collect_type_refs_from_rust_type(return_type, refs);
        }
        RustType::DynTrait(name) => {
            refs.insert(name.clone());
        }
        RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Any
        | RustType::Never => {}
    }
}

#[cfg(test)]
mod tests;
