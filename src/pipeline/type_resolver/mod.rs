//! TypeResolver: pre-computes type information for all expressions in a file.
//!
//! Walks the AST independently of the Transformer, resolving expression types,
//! expected types, narrowing events, and variable mutability. The results are
//! stored in [`FileTypeResolution`] which the Transformer reads as immutable data.

mod call_resolution;
mod du_analysis;
mod expected_types;
mod expressions;
mod fn_exprs;
mod helpers;
mod narrowing;
mod visitors;

use helpers::*;

use std::collections::HashMap;

use crate::ir::RustType;
use crate::pipeline::type_resolution::{AnyEnumOverride, FileTypeResolution, Span, VarId};
use crate::pipeline::ResolvedType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};

/// Pre-computes type information for a single file.
///
/// The resolver walks the AST top-down, maintaining a scope stack for variable
/// types and a parent stack for expected type computation. It produces a
/// [`FileTypeResolution`] that the Transformer can query.
pub struct TypeResolver<'a> {
    registry: &'a TypeRegistry,
    synthetic: &'a mut SyntheticTypeRegistry,

    // Internal state during resolution
    scope_stack: Vec<Scope>,
    current_fn_return_type: Option<RustType>,
    result: FileTypeResolution,

    /// Any-enum overrides from the any_enum_analyzer (computed before TypeResolver).
    /// When a variable is declared with `Any` type, this is checked for an override
    /// with a more specific enum type, so `expr_types` records the correct type
    /// from the start.
    any_enum_overrides: Vec<AnyEnumOverride>,

    /// Type parameter constraints in the current scope.
    /// `E extends Env` → {"E": Named("Env")}.
    /// Populated when entering generic functions/classes, cleared on scope exit.
    type_param_constraints: HashMap<String, RustType>,

    /// End byte position of the current enclosing block.
    /// Used for early-return complement narrowing: when an if-statement's then-block
    /// always exits, the complement type is valid from after the if-statement
    /// to the end of this enclosing block.
    current_block_end: Option<u32>,
}

/// A scope containing variable bindings.
#[derive(Debug, Default)]
struct Scope {
    vars: HashMap<String, VarInfo>,
}

/// Information about a variable in scope.
#[derive(Debug, Clone)]
struct VarInfo {
    ty: ResolvedType,
    var_id: VarId,
}

impl<'a> TypeResolver<'a> {
    /// Creates a new TypeResolver.
    pub fn new(registry: &'a TypeRegistry, synthetic: &'a mut SyntheticTypeRegistry) -> Self {
        Self {
            registry,
            synthetic,
            scope_stack: vec![Scope::default()],
            current_fn_return_type: None,
            result: FileTypeResolution::empty(),
            any_enum_overrides: Vec::new(),
            type_param_constraints: HashMap::new(),
            current_block_end: None,
        }
    }

    /// Sets the any-enum overrides computed by `any_enum_analyzer`.
    ///
    /// When set, `declare_var` will replace `Any` types with the corresponding
    /// enum type, so all subsequent `expr_types` entries reflect the correct type.
    pub fn set_any_enum_overrides(&mut self, overrides: Vec<AnyEnumOverride>) {
        self.any_enum_overrides = overrides;
    }

    /// Resolves type information for an entire file.
    pub fn resolve_file(&mut self, file: &crate::pipeline::ParsedFile) -> FileTypeResolution {
        for item in &file.module.body {
            self.visit_module_item(item);
        }
        let mut result = std::mem::replace(&mut self.result, FileTypeResolution::empty());
        // Transfer any_enum_overrides to the result for Transformer access
        // (used by convert_var_decl and param type override)
        result.any_enum_overrides = std::mem::take(&mut self.any_enum_overrides);
        result
    }

    // --- Scope management ---

    fn enter_scope(&mut self) {
        self.scope_stack.push(Scope::default());
    }

    fn leave_scope(&mut self) {
        if self.scope_stack.len() > 1 {
            self.scope_stack.pop();
        }
    }

    fn declare_var(&mut self, name: &str, ty: ResolvedType, span: Span, mutable: bool) {
        // Apply any_enum_override: if the variable type is Any and there's a
        // matching override at this position, use the enum type instead.
        let ty = if matches!(&ty, ResolvedType::Known(RustType::Any)) {
            self.any_enum_overrides
                .iter()
                .rfind(|o| o.var_name == name && o.scope_start <= span.lo && span.lo < o.scope_end)
                .map(|o| ResolvedType::Known(o.enum_type.clone()))
                .unwrap_or(ty)
        } else {
            ty
        };

        let var_id = VarId {
            name: name.to_string(),
            declared_at: span,
        };
        self.result.var_mutability.insert(var_id.clone(), mutable);
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.vars.insert(name.to_string(), VarInfo { ty, var_id });
        }
    }

    fn lookup_var(&self, name: &str) -> ResolvedType {
        for scope in self.scope_stack.iter().rev() {
            if let Some(info) = scope.vars.get(name) {
                return info.ty.clone();
            }
        }
        ResolvedType::Unknown
    }

    fn mark_var_mutable(&mut self, name: &str) {
        for scope in self.scope_stack.iter().rev() {
            if let Some(info) = scope.vars.get(name) {
                self.result.var_mutability.insert(info.var_id.clone(), true);
                return;
            }
        }
    }

    /// Records the declared type of an assignment-target identifier at its
    /// SWC span in `expr_types` and returns the resolved type.
    ///
    /// Unlike an `Expr::Ident` used as a value, the `BindingIdent` inside an
    /// `AssignTarget` is not visited by `resolve_expr`, so its span otherwise
    /// carries no entry. I-142's `??=` handler reads the LHS type from the
    /// ident's span to decide between shadow-let and `get_or_insert_with`, so
    /// this bridge closes the gap for every assignment (plain `=` already
    /// stored it via `lookup_var`, compound `??=`/`+=`/... did not).
    pub(super) fn record_assign_target_ident_type(
        &mut self,
        ident: &swc_ecma_ast::BindingIdent,
    ) -> ResolvedType {
        let ty = self.lookup_var(ident.id.sym.as_ref());
        if matches!(ty, ResolvedType::Known(_)) {
            let span = crate::pipeline::type_resolution::Span::from_swc(ident.id.span);
            self.result
                .expr_types
                .entry(span)
                .or_insert_with(|| ty.clone());
        }
        ty
    }
}

#[cfg(test)]
mod tests;
