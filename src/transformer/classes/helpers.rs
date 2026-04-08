//! visibility 解決、parent 探索、共通ヘルパー。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::ir::{
    AssocConst, Expr, Item, Method, RustType, Stmt, StructField, TraitRef, TypeParam, Visibility,
};

/// Creates an `Item::Struct` from a name, visibility, type parameters, and fields.
pub(super) fn make_struct(
    name: &str,
    vis: &Visibility,
    type_params: Vec<TypeParam>,
    fields: Vec<StructField>,
) -> Item {
    Item::Struct {
        vis: *vis,
        name: name.to_string(),
        type_params,
        fields,
    }
}

/// 型パラメータのリストから trait 参照を生成する。
///
/// 例: `type_params: [T, U]` → `TraitRef { name: "FooTrait", type_args: [TypeVar("T"), TypeVar("U")] }`
/// (I-387: 型パラメータ参照は構造化 `TypeVar` variant で表現)
pub(super) fn make_trait_ref(name: &str, type_params: &[TypeParam]) -> TraitRef {
    TraitRef {
        name: name.to_string(),
        type_args: type_params
            .iter()
            .map(|p| RustType::TypeVar {
                name: p.name.clone(),
            })
            .collect(),
    }
}

/// Strips visibility from methods for use in trait impl blocks.
pub(super) fn strip_method_visibility(methods: &[Method]) -> Vec<Method> {
    methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            ..m.clone()
        })
        .collect()
}

/// Creates an `Item::Impl` block from type parameters, constants, constructor, and/or methods.
///
/// Returns `None` if constants, constructor, and methods are all empty.
pub(super) fn make_impl(
    struct_name: &str,
    type_params: Vec<TypeParam>,
    for_trait: Option<TraitRef>,
    consts: Vec<AssocConst>,
    ctor: Option<&Method>,
    methods: Vec<Method>,
) -> Option<Item> {
    let mut all_methods = Vec::new();
    if let Some(c) = ctor {
        all_methods.push(c.clone());
    }
    all_methods.extend(methods);

    if all_methods.is_empty() && consts.is_empty() {
        return None;
    }

    Some(Item::Impl {
        struct_name: struct_name.to_string(),
        type_params,
        for_trait,
        consts,
        methods: all_methods,
    })
}

/// Resolves the effective visibility of a class member based on its TypeScript accessibility modifier.
///
/// `protected` maps to `pub(crate)`, `private` maps to `Private`, and `public` (or unspecified)
/// inherits the class-level visibility.
pub(super) fn resolve_member_visibility(
    accessibility: Option<ast::Accessibility>,
    class_vis: &Visibility,
) -> Visibility {
    match accessibility {
        Some(ast::Accessibility::Protected) => Visibility::PubCrate,
        Some(ast::Accessibility::Private) => Visibility::Private,
        _ => *class_vis,
    }
}

/// Returns `true` if the method body contains an assignment to `self.field`.
pub(super) fn body_has_self_assignment(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::Expr(Expr::Assign { target, .. }) => is_self_field_access(target),
        _ => false,
    })
}

/// Returns `true` if the expression is `self.field`.
pub(super) fn is_self_field_access(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FieldAccess {
            object,
            ..
        } if matches!(object.as_ref(), Expr::Ident(name) if name == "self")
    )
}

/// Pre-scans all interface declarations to collect method names per interface.
///
/// Used by `implements` processing to determine which class methods belong to
/// which trait impl block.
pub(in crate::transformer) fn pre_scan_interface_methods(
    module: &ast::Module,
) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();

    for module_item in &module.body {
        let decl = match module_item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::TsInterface(d))) => d,
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                if let ast::Decl::TsInterface(d) = &export.decl {
                    d
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let name = decl.id.sym.to_string();
        let method_names: Vec<String> = decl
            .body
            .body
            .iter()
            .filter_map(|member| {
                if let ast::TsTypeElement::TsMethodSignature(method) = member {
                    if let ast::Expr::Ident(ident) = method.key.as_ref() {
                        return Some(ident.sym.to_string());
                    }
                }
                None
            })
            .collect();

        if !method_names.is_empty() {
            map.insert(name, method_names);
        }
    }

    map
}

/// Identifies which classes are parents (are extended by another class).
pub(super) fn find_parent_class_names(
    class_map: &HashMap<String, super::ClassInfo>,
) -> std::collections::HashSet<String> {
    class_map
        .values()
        .filter_map(|info| info.parent.clone())
        .collect()
}
