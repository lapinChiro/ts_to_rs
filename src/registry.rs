//! TypeRegistry — モジュール内の型定義を事前収集し、変換時に参照するレジストリ。
//!
//! 変換パイプラインの第 1 パスで SWC AST を走査して型情報を収集し、
//! 第 2 パスの変換時にネストしたオブジェクトリテラルや enum メンバーアクセスの
//! 解決に使用する。

use std::collections::HashMap;

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::transformer::types::convert_ts_type;

/// 型定義の種類。
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// struct（interface / type alias から変換）
    Struct {
        /// フィールド名と型のペア
        fields: Vec<(String, RustType)>,
    },
    /// enum
    Enum {
        /// バリアント名の一覧
        variants: Vec<String>,
    },
    /// 関数
    Function {
        /// パラメータ名と型のペア
        params: Vec<(String, RustType)>,
        /// 戻り値型
        return_type: Option<RustType>,
    },
}

/// モジュール内の型定義を保持するレジストリ。
///
/// 型名をキーにして `TypeDef` を引くことで、変換時にフィールド型や
/// enum バリアントを解決できる。
#[derive(Debug, Clone)]
pub struct TypeRegistry {
    types: HashMap<String, TypeDef>,
}

impl TypeRegistry {
    /// 空の TypeRegistry を作成する。
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    /// 型定義を登録する。
    pub fn register(&mut self, name: String, def: TypeDef) {
        self.types.insert(name, def);
    }

    /// 型名から TypeDef を取得する。
    pub fn get(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    /// 別の TypeRegistry の内容をマージする。
    ///
    /// 同名の型が既に存在する場合は上書きする。
    pub fn merge(&mut self, other: &TypeRegistry) {
        for (name, def) in &other.types {
            self.types.insert(name.clone(), def.clone());
        }
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// SWC [`ast::Module`] を走査し、型定義を収集して [`TypeRegistry`] を構築する。
///
/// 以下の宣言を収集する:
/// - `interface` → `TypeDef::Struct`
/// - `type` (オブジェクト型) → `TypeDef::Struct`
/// - `enum` → `TypeDef::Enum`
/// - 関数宣言 → `TypeDef::Function`
/// - `const` + アロー関数 → `TypeDef::Function`
///
/// 型変換に失敗した宣言はスキップする（レジストリ構築は best-effort）。
pub fn build_registry(module: &ast::Module) -> TypeRegistry {
    let mut reg = TypeRegistry::new();

    for item in &module.body {
        match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => {
                collect_decl(&mut reg, decl);
            }
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                collect_decl(&mut reg, &export.decl);
            }
            _ => {}
        }
    }

    reg
}

/// 個々の宣言から型情報を収集する。
fn collect_decl(reg: &mut TypeRegistry, decl: &ast::Decl) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            if let Ok(fields) = collect_interface_fields(iface) {
                reg.register(iface.id.sym.to_string(), TypeDef::Struct { fields });
            }
        }
        ast::Decl::TsTypeAlias(alias) => {
            if let Some(fields) = collect_type_alias_fields(alias) {
                reg.register(alias.id.sym.to_string(), TypeDef::Struct { fields });
            }
        }
        ast::Decl::TsEnum(ts_enum) => {
            let variants = ts_enum
                .members
                .iter()
                .map(|m| match &m.id {
                    ast::TsEnumMemberId::Ident(ident) => ident.sym.to_string(),
                    ast::TsEnumMemberId::Str(s) => s.value.to_string_lossy().into_owned(),
                })
                .collect();
            reg.register(ts_enum.id.sym.to_string(), TypeDef::Enum { variants });
        }
        ast::Decl::Fn(fn_decl) => {
            if let Ok(func_def) = collect_fn_def(&fn_decl.function) {
                reg.register(fn_decl.ident.sym.to_string(), func_def);
            }
        }
        ast::Decl::Var(var_decl) => {
            // const f = (x: number): string => ...
            for d in &var_decl.decls {
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(arrow) = init.as_ref() {
                        let name = match &d.name {
                            ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                            _ => continue,
                        };
                        if let Ok(func_def) = collect_arrow_def(arrow) {
                            reg.register(name, func_def);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// interface のフィールド名・型を収集する。
fn collect_interface_fields(iface: &ast::TsInterfaceDecl) -> Result<Vec<(String, RustType)>> {
    let mut fields = Vec::new();
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            if let Some((name, ty)) = collect_property_signature(prop) {
                fields.push((name, ty));
            }
        }
    }
    Ok(fields)
}

/// TsPropertySignature からフィールド名と型を取得する。
fn collect_property_signature(prop: &ast::TsPropertySignature) -> Option<(String, RustType)> {
    let name = match prop.key.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let ty = prop
        .type_ann
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann).ok())?;

    // Optional fields are wrapped in Option
    let ty = if prop.optional {
        RustType::Option(Box::new(ty))
    } else {
        ty
    };

    Some((name, ty))
}

/// type alias (オブジェクト型) のフィールドを収集する。
fn collect_type_alias_fields(alias: &ast::TsTypeAliasDecl) -> Option<Vec<(String, RustType)>> {
    let type_lit = match alias.type_ann.as_ref() {
        ast::TsType::TsTypeLit(lit) => lit,
        _ => return None,
    };

    let mut fields = Vec::new();
    for member in &type_lit.members {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            if let Some((name, ty)) = collect_property_signature(prop) {
                fields.push((name, ty));
            }
        }
    }
    Some(fields)
}

/// 関数宣言からパラメータ型と戻り値型を収集する。
fn collect_fn_def(func: &ast::Function) -> Result<TypeDef> {
    let mut params = Vec::new();
    for param in &func.params {
        if let ast::Pat::Ident(ident) = &param.pat {
            let name = ident.id.sym.to_string();
            if let Some(ann) = &ident.type_ann {
                if let Ok(ty) = convert_ts_type(&ann.type_ann) {
                    params.push((name, ty));
                }
            }
        }
    }

    let return_type = func
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann).ok());

    Ok(TypeDef::Function {
        params,
        return_type,
    })
}

/// アロー関数からパラメータ型と戻り値型を収集する。
fn collect_arrow_def(arrow: &ast::ArrowExpr) -> Result<TypeDef> {
    let mut params = Vec::new();
    for param in &arrow.params {
        if let ast::Pat::Ident(ident) = param {
            let name = ident.id.sym.to_string();
            if let Some(ann) = &ident.type_ann {
                if let Ok(ty) = convert_ts_type(&ann.type_ann) {
                    params.push((name, ty));
                }
            }
        }
    }

    let return_type = arrow
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann).ok());

    Ok(TypeDef::Function {
        params,
        return_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::RustType;
    use crate::parser::parse_typescript;

    #[test]
    fn test_registry_new_is_empty() {
        let reg = TypeRegistry::new();
        assert!(reg.get("Foo").is_none());
    }

    #[test]
    fn test_registry_register_and_get_struct() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Point".to_string(),
            TypeDef::Struct {
                fields: vec![
                    ("x".to_string(), RustType::F64),
                    ("y".to_string(), RustType::F64),
                ],
            },
        );
        let def = reg.get("Point").unwrap();
        assert_eq!(
            *def,
            TypeDef::Struct {
                fields: vec![
                    ("x".to_string(), RustType::F64),
                    ("y".to_string(), RustType::F64),
                ],
            }
        );
    }

    #[test]
    fn test_registry_register_and_get_enum() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Color".to_string(),
            TypeDef::Enum {
                variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            },
        );
        let def = reg.get("Color").unwrap();
        assert_eq!(
            *def,
            TypeDef::Enum {
                variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(),],
            }
        );
    }

    #[test]
    fn test_registry_register_and_get_function() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "draw".to_string(),
            TypeDef::Function {
                params: vec![(
                    "p".to_string(),
                    RustType::Named {
                        name: "Point".to_string(),
                        type_args: vec![],
                    },
                )],
                return_type: None,
            },
        );
        let def = reg.get("draw").unwrap();
        match def {
            TypeDef::Function {
                params,
                return_type,
            } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].0, "p");
                assert!(return_type.is_none());
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_registry_get_nonexistent_returns_none() {
        let reg = TypeRegistry::new();
        assert!(reg.get("NonExistent").is_none());
    }

    #[test]
    fn test_registry_merge() {
        let mut reg1 = TypeRegistry::new();
        reg1.register(
            "Point".to_string(),
            TypeDef::Struct {
                fields: vec![("x".to_string(), RustType::F64)],
            },
        );

        let mut reg2 = TypeRegistry::new();
        reg2.register(
            "Color".to_string(),
            TypeDef::Enum {
                variants: vec!["Red".to_string()],
            },
        );

        reg1.merge(&reg2);
        assert!(reg1.get("Point").is_some());
        assert!(reg1.get("Color").is_some());
    }

    // -- build_registry tests --

    #[test]
    fn test_build_registry_interface() {
        let module = parse_typescript("interface Point { x: number; y: number; }").unwrap();
        let reg = build_registry(&module);
        assert_eq!(
            reg.get("Point").unwrap(),
            &TypeDef::Struct {
                fields: vec![
                    ("x".to_string(), RustType::F64),
                    ("y".to_string(), RustType::F64),
                ],
            }
        );
    }

    #[test]
    fn test_build_registry_type_alias_object() {
        let module = parse_typescript("type Config = { name: string; count: number; };").unwrap();
        let reg = build_registry(&module);
        assert_eq!(
            reg.get("Config").unwrap(),
            &TypeDef::Struct {
                fields: vec![
                    ("name".to_string(), RustType::String),
                    ("count".to_string(), RustType::F64),
                ],
            }
        );
    }

    #[test]
    fn test_build_registry_enum() {
        let module = parse_typescript("enum Color { Red, Green, Blue }").unwrap();
        let reg = build_registry(&module);
        assert_eq!(
            reg.get("Color").unwrap(),
            &TypeDef::Enum {
                variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(),],
            }
        );
    }

    #[test]
    fn test_build_registry_function() {
        let module =
            parse_typescript("function draw(p: Point, color: string): boolean { return true; }")
                .unwrap();
        let reg = build_registry(&module);
        match reg.get("draw").unwrap() {
            TypeDef::Function {
                params,
                return_type,
            } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "p");
                assert_eq!(
                    params[0].1,
                    RustType::Named {
                        name: "Point".to_string(),
                        type_args: vec![],
                    }
                );
                assert_eq!(params[1].0, "color");
                assert_eq!(params[1].1, RustType::String);
                assert_eq!(*return_type, Some(RustType::Bool));
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_build_registry_arrow_function() {
        let module = parse_typescript("const greet = (name: string): string => name;").unwrap();
        let reg = build_registry(&module);
        match reg.get("greet").unwrap() {
            TypeDef::Function {
                params,
                return_type,
            } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].0, "name");
                assert_eq!(params[0].1, RustType::String);
                assert_eq!(*return_type, Some(RustType::String));
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_build_registry_export_declarations() {
        let module =
            parse_typescript("export interface Foo { x: number; }\nexport enum Bar { A, B }")
                .unwrap();
        let reg = build_registry(&module);
        assert!(reg.get("Foo").is_some());
        assert!(reg.get("Bar").is_some());
    }

    #[test]
    fn test_build_registry_optional_field() {
        let module = parse_typescript("interface Config { name?: string; }").unwrap();
        let reg = build_registry(&module);
        assert_eq!(
            reg.get("Config").unwrap(),
            &TypeDef::Struct {
                fields: vec![(
                    "name".to_string(),
                    RustType::Option(Box::new(RustType::String)),
                )],
            }
        );
    }

    #[test]
    fn test_build_registry_empty_module() {
        let module = parse_typescript("").unwrap();
        let reg = build_registry(&module);
        assert!(reg.get("anything").is_none());
    }
}
