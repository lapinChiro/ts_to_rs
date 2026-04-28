//! Member access type resolution (`Member` arm of `resolve_expr_inner`).
//!
//! [`TypeResolver::resolve_member_type`] is the shared field/index lookup primitive
//! consumed by [`super::Member`], [`super::OptChain`], assignments, and any other arm
//! that needs "given an obj type and a `MemberProp`, what's the field type?". It is
//! `pub(super)` so all sibling expression files (and the parent `type_resolver`
//! module via `pub(super)` reachability through `expressions::mod`) can call it.

use swc_ecma_ast as ast;

use super::super::*;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_member_expr(&mut self, member: &ast::MemberExpr) -> ResolvedType {
        let obj_type = self.resolve_expr(&member.obj);
        match &obj_type {
            ResolvedType::Known(ty) => self.resolve_member_type(ty, &member.prop),
            ResolvedType::Unknown => ResolvedType::Unknown,
        }
    }

    /// Resolves the type of a member access given the object's type and property.
    pub(super) fn resolve_member_type(
        &self,
        obj_rust_type: &RustType,
        prop: &ast::MemberProp,
    ) -> ResolvedType {
        // Array/tuple/HashMap indexing
        if let ast::MemberProp::Computed(computed) = prop {
            match obj_rust_type {
                RustType::Vec(elem_ty) => return ResolvedType::Known(elem_ty.as_ref().clone()),
                RustType::Tuple(elems) => {
                    if let ast::Expr::Lit(ast::Lit::Num(num)) = &*computed.expr {
                        let idx = num.value as usize;
                        if idx < elems.len() {
                            return ResolvedType::Known(elems[idx].clone());
                        }
                    }
                    return ResolvedType::Unknown;
                }
                // HashMap<K, V>[key] → V
                RustType::Named { name, type_args }
                    if name == "HashMap" && type_args.len() == 2 =>
                {
                    return ResolvedType::Known(type_args[1].clone());
                }
                // I-387: StdCollection 版 HashMap
                RustType::StdCollection {
                    kind: crate::ir::StdCollectionKind::HashMap,
                    args,
                } if args.len() == 2 => {
                    return ResolvedType::Known(args[1].clone());
                }
                _ => {}
            }
        }

        // Named field access (Ident and PrivateName)
        let field_name = match prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            ast::MemberProp::PrivateName(private) => private.name.to_string(),
            _ => return ResolvedType::Unknown,
        };

        // Special case: .length on String/Vec (hardcoded for performance — avoids registry lookup)
        if field_name == "length" && matches!(obj_rust_type, RustType::String | RustType::Vec(_)) {
            return ResolvedType::Known(RustType::F64);
        }

        // 1. TypeRegistry (handles Vec→Array, String, Named, DynTrait, etc.)
        if let Some(ty) = self.registry.lookup_field_type(obj_rust_type, &field_name) {
            return ResolvedType::Known(ty);
        }

        // 2. Struct fields fallback (SyntheticTypeRegistry + type parameter constraints)
        if let RustType::Named { name, type_args } = obj_rust_type {
            if let Some(fields) = self.resolve_struct_fields_by_name(name, type_args) {
                if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &field_name) {
                    return ResolvedType::Known(ty.clone());
                }
            }
        }
        // I-387: TypeVar の member access は constraint lookup で解決。
        if let RustType::TypeVar { name } = obj_rust_type {
            if let Some(fields) = self.resolve_struct_fields_by_name(name, &[]) {
                if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &field_name) {
                    return ResolvedType::Known(ty.clone());
                }
            }
        }

        ResolvedType::Unknown
    }
}
