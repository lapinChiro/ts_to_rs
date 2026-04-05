//! `substitute` implementations for IR nodes.
//!
//! These methods replace type parameter references with concrete types,
//! used during monomorphization.

use super::*;
use std::collections::HashMap;

impl TypeParam {
    /// 型パラメータの制約内の型パラメータ参照を具体型で置換した新しい TypeParam を返す。
    ///
    /// `name` は型パラメータの識別子であり置換対象ではない。
    /// `constraint` 内の `RustType` のみ置換する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> TypeParam {
        TypeParam {
            name: self.name.clone(),
            constraint: self.constraint.as_ref().map(|c| c.substitute(bindings)),
        }
    }
}

impl RustType {
    /// 型パラメータ名を具体型に置換する。
    ///
    /// `bindings` は型パラメータ名 → 具体型のマッピング。
    /// `Named { name: "T" }` が `bindings` に存在すれば具体型に置換し、
    /// それ以外のバリアントは再帰的に処理する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> RustType {
        match self {
            RustType::Named { name, type_args } => {
                if type_args.is_empty() {
                    if let Some(concrete) = bindings.get(name.as_str()) {
                        return concrete.clone();
                    }
                }
                RustType::Named {
                    name: name.clone(),
                    type_args: type_args.iter().map(|a| a.substitute(bindings)).collect(),
                }
            }
            RustType::Vec(inner) => RustType::Vec(Box::new(inner.substitute(bindings))),
            RustType::Option(inner) => RustType::Option(Box::new(inner.substitute(bindings))),
            RustType::Ref(inner) => RustType::Ref(Box::new(inner.substitute(bindings))),
            RustType::Result { ok, err } => RustType::Result {
                ok: Box::new(ok.substitute(bindings)),
                err: Box::new(err.substitute(bindings)),
            },
            RustType::Tuple(elems) => {
                RustType::Tuple(elems.iter().map(|e| e.substitute(bindings)).collect())
            }
            RustType::Fn {
                params,
                return_type,
            } => RustType::Fn {
                params: params.iter().map(|p| p.substitute(bindings)).collect(),
                return_type: Box::new(return_type.substitute(bindings)),
            },
            other => other.clone(),
        }
    }
}

impl StructField {
    /// 型パラメータを具体型で置換した新しい StructField を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> StructField {
        StructField {
            vis: self.vis,
            name: self.name.clone(),
            ty: self.ty.substitute(bindings),
        }
    }
}

impl Param {
    /// 型パラメータを具体型で置換した新しい Param を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Param {
        Param {
            name: self.name.clone(),
            ty: self.ty.as_ref().map(|t| t.substitute(bindings)),
        }
    }
}

impl Method {
    /// 型パラメータを具体型で置換した新しい Method を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Method {
        Method {
            vis: self.vis,
            name: self.name.clone(),
            has_self: self.has_self,
            has_mut_self: self.has_mut_self,
            params: self.params.iter().map(|p| p.substitute(bindings)).collect(),
            return_type: self.return_type.as_ref().map(|t| t.substitute(bindings)),
            body: self
                .body
                .as_ref()
                .map(|stmts| stmts.iter().map(|s| s.substitute(bindings)).collect()),
        }
    }
}

impl EnumVariant {
    /// 型パラメータを具体型で置換した新しい EnumVariant を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> EnumVariant {
        EnumVariant {
            name: self.name.clone(),
            value: self.value.clone(),
            data: self.data.as_ref().map(|d| d.substitute(bindings)),
            fields: self.fields.iter().map(|f| f.substitute(bindings)).collect(),
        }
    }
}

impl Item {
    /// 型パラメータを具体型で置換した新しい Item を返す。
    ///
    /// 全バリアントの `RustType` フィールドを再帰的��置換する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Item {
        if bindings.is_empty() {
            return self.clone();
        }
        match self {
            Item::Struct {
                vis,
                name,
                type_params,
                fields,
            } => Item::Struct {
                vis: *vis,
                name: name.clone(),
                type_params: type_params.clone(),
                fields: fields.iter().map(|f| f.substitute(bindings)).collect(),
            },
            Item::Enum {
                vis,
                name,
                type_params,
                serde_tag,
                variants,
            } => Item::Enum {
                vis: *vis,
                name: name.clone(),
                type_params: type_params.clone(),
                serde_tag: serde_tag.clone(),
                variants: variants.iter().map(|v| v.substitute(bindings)).collect(),
            },
            Item::Trait {
                vis,
                name,
                type_params,
                supertraits,
                methods,
                associated_types,
            } => Item::Trait {
                vis: *vis,
                name: name.clone(),
                type_params: type_params.clone(),
                supertraits: supertraits.clone(),
                methods: methods.iter().map(|m| m.substitute(bindings)).collect(),
                associated_types: associated_types.clone(),
            },
            Item::Impl {
                struct_name,
                type_params,
                for_trait,
                consts,
                methods,
            } => Item::Impl {
                struct_name: struct_name.clone(),
                type_params: type_params.clone(),
                for_trait: for_trait.clone(),
                consts: consts.clone(),
                methods: methods.iter().map(|m| m.substitute(bindings)).collect(),
            },
            Item::TypeAlias {
                vis,
                name,
                type_params,
                ty,
            } => Item::TypeAlias {
                vis: *vis,
                name: name.clone(),
                type_params: type_params.clone(),
                ty: ty.substitute(bindings),
            },
            Item::Fn {
                vis,
                attributes,
                is_async,
                name,
                type_params,
                params,
                return_type,
                body,
            } => Item::Fn {
                vis: *vis,
                attributes: attributes.clone(),
                is_async: *is_async,
                name: name.clone(),
                type_params: type_params.clone(),
                params: params.iter().map(|p| p.substitute(bindings)).collect(),
                return_type: return_type.as_ref().map(|t| t.substitute(bindings)),
                body: body.iter().map(|s| s.substitute(bindings)).collect(),
            },
            other => other.clone(),
        }
    }
}

impl Stmt {
    /// 型パラメータを具体型で置換した新しい Stmt を返す。
    ///
    /// `RustType` を含むバリアント（`Let::ty`, `Closure::return_type`, `Cast::target` 等）
    /// を再帰的に置換する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Stmt {
        match self {
            Stmt::Let {
                mutable,
                name,
                ty,
                init,
            } => Stmt::Let {
                mutable: *mutable,
                name: name.clone(),
                ty: ty.as_ref().map(|t| t.substitute(bindings)),
                init: init.as_ref().map(|e| e.substitute(bindings)),
            },
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => Stmt::If {
                condition: condition.substitute(bindings),
                then_body: then_body.iter().map(|s| s.substitute(bindings)).collect(),
                else_body: else_body
                    .as_ref()
                    .map(|stmts| stmts.iter().map(|s| s.substitute(bindings)).collect()),
            },
            Stmt::While {
                label,
                condition,
                body,
            } => Stmt::While {
                label: label.clone(),
                condition: condition.substitute(bindings),
                body: body.iter().map(|s| s.substitute(bindings)).collect(),
            },
            Stmt::WhileLet {
                label,
                pattern,
                expr,
                body,
            } => Stmt::WhileLet {
                label: label.clone(),
                pattern: pattern.clone(),
                expr: expr.substitute(bindings),
                body: body.iter().map(|s| s.substitute(bindings)).collect(),
            },
            Stmt::ForIn {
                label,
                var,
                iterable,
                body,
            } => Stmt::ForIn {
                label: label.clone(),
                var: var.clone(),
                iterable: iterable.substitute(bindings),
                body: body.iter().map(|s| s.substitute(bindings)).collect(),
            },
            Stmt::Loop { label, body } => Stmt::Loop {
                label: label.clone(),
                body: body.iter().map(|s| s.substitute(bindings)).collect(),
            },
            Stmt::Break { label, value } => Stmt::Break {
                label: label.clone(),
                value: value.as_ref().map(|e| e.substitute(bindings)),
            },
            Stmt::Continue { label } => Stmt::Continue {
                label: label.clone(),
            },
            Stmt::Return(expr) => Stmt::Return(expr.as_ref().map(|e| e.substitute(bindings))),
            Stmt::Expr(expr) => Stmt::Expr(expr.substitute(bindings)),
            Stmt::TailExpr(expr) => Stmt::TailExpr(expr.substitute(bindings)),
            Stmt::IfLet {
                pattern,
                expr,
                then_body,
                else_body,
            } => Stmt::IfLet {
                pattern: pattern.clone(),
                expr: expr.substitute(bindings),
                then_body: then_body.iter().map(|s| s.substitute(bindings)).collect(),
                else_body: else_body
                    .as_ref()
                    .map(|stmts| stmts.iter().map(|s| s.substitute(bindings)).collect()),
            },
            Stmt::Match { expr, arms } => Stmt::Match {
                expr: expr.substitute(bindings),
                arms: arms.iter().map(|a| a.substitute(bindings)).collect(),
            },
            Stmt::LabeledBlock { label, body } => Stmt::LabeledBlock {
                label: label.clone(),
                body: body.iter().map(|s| s.substitute(bindings)).collect(),
            },
        }
    }
}

impl MatchArm {
    /// 型パラメータを具体型で置換した新しい MatchArm を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> MatchArm {
        MatchArm {
            patterns: self.patterns.clone(),
            guard: self.guard.as_ref().map(|e| e.substitute(bindings)),
            body: self.body.iter().map(|s| s.substitute(bindings)).collect(),
        }
    }
}

impl Expr {
    /// 型パラメータを具体型で置換した新しい Expr を返す。
    ///
    /// `RustType` を含むバリアント（`Closure::return_type`, `Cast::target`）を置換する。
    /// それ以外のバリアントは再帰的にサブ式を処理する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Expr {
        match self {
            Expr::Closure {
                params,
                return_type,
                body,
            } => Expr::Closure {
                params: params.iter().map(|p| p.substitute(bindings)).collect(),
                return_type: return_type.as_ref().map(|t| t.substitute(bindings)),
                body: match body {
                    ClosureBody::Expr(e) => ClosureBody::Expr(Box::new(e.substitute(bindings))),
                    ClosureBody::Block(stmts) => {
                        ClosureBody::Block(stmts.iter().map(|s| s.substitute(bindings)).collect())
                    }
                },
            },
            Expr::Cast { expr, target } => Expr::Cast {
                expr: Box::new(expr.substitute(bindings)),
                target: target.substitute(bindings),
            },
            // Recursive cases for sub-expressions
            Expr::FieldAccess { object, field } => Expr::FieldAccess {
                object: Box::new(object.substitute(bindings)),
                field: field.clone(),
            },
            Expr::MethodCall {
                object,
                method,
                args,
            } => Expr::MethodCall {
                object: Box::new(object.substitute(bindings)),
                method: method.clone(),
                args: args.iter().map(|a| a.substitute(bindings)).collect(),
            },
            Expr::StructInit { name, fields, base } => Expr::StructInit {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(n, e)| (n.clone(), e.substitute(bindings)))
                    .collect(),
                base: base.as_ref().map(|b| Box::new(b.substitute(bindings))),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: Box::new(target.substitute(bindings)),
                value: Box::new(value.substitute(bindings)),
            },
            Expr::UnaryOp { op, operand } => Expr::UnaryOp {
                op: *op,
                operand: Box::new(operand.substitute(bindings)),
            },
            Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
                left: Box::new(left.substitute(bindings)),
                op: *op,
                right: Box::new(right.substitute(bindings)),
            },
            Expr::Range { start, end } => Expr::Range {
                start: start.as_ref().map(|e| Box::new(e.substitute(bindings))),
                end: end.as_ref().map(|e| Box::new(e.substitute(bindings))),
            },
            Expr::FnCall { name, args } => Expr::FnCall {
                name: name.clone(),
                args: args.iter().map(|a| a.substitute(bindings)).collect(),
            },
            Expr::Vec { elements } => Expr::Vec {
                elements: elements.iter().map(|e| e.substitute(bindings)).collect(),
            },
            Expr::Tuple { elements } => Expr::Tuple {
                elements: elements.iter().map(|e| e.substitute(bindings)).collect(),
            },
            Expr::If {
                condition,
                then_expr,
                else_expr,
            } => Expr::If {
                condition: Box::new(condition.substitute(bindings)),
                then_expr: Box::new(then_expr.substitute(bindings)),
                else_expr: Box::new(else_expr.substitute(bindings)),
            },
            Expr::IfLet {
                pattern,
                expr,
                then_expr,
                else_expr,
            } => Expr::IfLet {
                pattern: pattern.clone(),
                expr: Box::new(expr.substitute(bindings)),
                then_expr: Box::new(then_expr.substitute(bindings)),
                else_expr: Box::new(else_expr.substitute(bindings)),
            },
            Expr::FormatMacro { template, args } => Expr::FormatMacro {
                template: template.clone(),
                args: args.iter().map(|a| a.substitute(bindings)).collect(),
            },
            Expr::MacroCall {
                name,
                args,
                use_debug,
            } => Expr::MacroCall {
                name: name.clone(),
                args: args.iter().map(|a| a.substitute(bindings)).collect(),
                use_debug: use_debug.clone(),
            },
            Expr::Await(e) => Expr::Await(Box::new(e.substitute(bindings))),
            Expr::Deref(e) => Expr::Deref(Box::new(e.substitute(bindings))),
            Expr::Ref(e) => Expr::Ref(Box::new(e.substitute(bindings))),
            Expr::Index { object, index } => Expr::Index {
                object: Box::new(object.substitute(bindings)),
                index: Box::new(index.substitute(bindings)),
            },
            Expr::RuntimeTypeof { operand } => Expr::RuntimeTypeof {
                operand: Box::new(operand.substitute(bindings)),
            },
            Expr::Matches { expr, pattern } => Expr::Matches {
                expr: Box::new(expr.substitute(bindings)),
                pattern: pattern.clone(),
            },
            Expr::Block(stmts) => {
                Expr::Block(stmts.iter().map(|s| s.substitute(bindings)).collect())
            }
            Expr::Match { expr, arms } => Expr::Match {
                expr: Box::new(expr.substitute(bindings)),
                arms: arms.iter().map(|a| a.substitute(bindings)).collect(),
            },
            // Leaf nodes without RustType or sub-expressions
            Expr::NumberLit(_)
            | Expr::IntLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::Unit
            | Expr::RawCode(_)
            | Expr::Regex { .. } => self.clone(),
        }
    }
}
