//! TypeRegistry — モジュール内の型定義を事前収集し、変換時に参照するレジストリ。
//!
//! 2-pass 方式で構築する:
//! - **Pass 1**: 型名だけをプレースホルダーとして登録する
//! - **Pass 2**: Pass 1 の型名一覧を参照しながらフィールド型を完全に解決する
//!
//! これにより前方参照（`interface A { b: B }` が `interface B` より前に宣言される場合）
//! でも正しく型を解決できる。

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
        /// メソッドシグネチャ（メソッド名 → パラメータ一覧）
        methods: HashMap<String, Vec<(String, RustType)>>,
        /// 親 interface 名のリスト（`interface B extends A` の `A`）
        extends: Vec<String>,
        /// Whether this type comes from a TS interface declaration (true) or class/type alias (false)
        is_interface: bool,
    },
    /// enum
    Enum {
        /// バリアント名の一覧
        variants: Vec<String>,
        /// 文字列リテラル値 → バリアント名のマッピング（string literal union / discriminated union）
        string_values: HashMap<String, String>,
        /// discriminated union の tag フィールド名（例: "kind"）
        tag_field: Option<String>,
        /// バリアント名 → フィールド一覧のマッピング（discriminated union のみ）
        variant_fields: HashMap<String, Vec<(String, RustType)>>,
    },
    /// 関数
    Function {
        /// パラメータ名と型のペア
        params: Vec<(String, RustType)>,
        /// 戻り値型
        return_type: Option<RustType>,
        /// 最後のパラメータが rest パラメータかどうか
        has_rest: bool,
    },
}

impl TypeDef {
    /// Creates a new struct TypeDef (from class, type alias, or other non-interface source).
    pub fn new_struct(
        fields: Vec<(String, RustType)>,
        methods: HashMap<String, Vec<(String, RustType)>>,
        extends: Vec<String>,
    ) -> Self {
        TypeDef::Struct {
            fields,
            methods,
            extends,
            is_interface: false,
        }
    }

    /// Creates a new interface TypeDef (from TS interface declaration).
    pub fn new_interface(
        fields: Vec<(String, RustType)>,
        methods: HashMap<String, Vec<(String, RustType)>>,
        extends: Vec<String>,
    ) -> Self {
        TypeDef::Struct {
            fields,
            methods,
            extends,
            is_interface: true,
        }
    }
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

    /// 型名が trait（メソッドを持つ interface）を指すかどうかを判定する。
    ///
    /// interface 由来かつ methods が空でない場合に `true` を返す。
    /// class 由来の型は常に `false`。
    pub fn is_trait_type(&self, name: &str) -> bool {
        if let Some(TypeDef::Struct {
            methods,
            is_interface,
            ..
        }) = self.get(name)
        {
            *is_interface && !methods.is_empty()
        } else {
            false
        }
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
/// 2-pass 方式で構築する:
/// - **Pass 1**: 型名だけをプレースホルダーとして登録する
/// - **Pass 2**: Pass 1 で構築した型名一覧を参照しながら、フィールド型を完全に解決する
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

    // Pass 1: 型名だけをプレースホルダーとして登録する
    for item in &module.body {
        match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => {
                collect_type_name(&mut reg, decl);
            }
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                collect_type_name(&mut reg, &export.decl);
            }
            _ => {}
        }
    }

    // Pass 2: Pass 1 の型名一覧を参照しながらフィールド型を完全に解決する
    let lookup = reg.clone();
    for item in &module.body {
        match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => {
                collect_decl(&mut reg, decl, &lookup);
            }
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                collect_decl(&mut reg, &export.decl, &lookup);
            }
            _ => {}
        }
    }

    reg
}

/// Pass 1: 宣言から型名だけをプレースホルダーとして登録する。
///
/// フィールド型の解決は行わず、型名の存在だけを記録する。
/// これにより Pass 2 で前方参照を解決できる。
fn collect_type_name(reg: &mut TypeRegistry, decl: &ast::Decl) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            reg.register(
                iface.id.sym.to_string(),
                TypeDef::new_interface(vec![], HashMap::new(), vec![]),
            );
        }
        ast::Decl::TsTypeAlias(alias) => {
            reg.register(
                alias.id.sym.to_string(),
                TypeDef::new_struct(vec![], HashMap::new(), vec![]),
            );
        }
        ast::Decl::TsEnum(ts_enum) => {
            reg.register(
                ts_enum.id.sym.to_string(),
                TypeDef::Enum {
                    variants: vec![],
                    string_values: HashMap::new(),
                    tag_field: None,
                    variant_fields: HashMap::new(),
                },
            );
        }
        ast::Decl::Fn(fn_decl) => {
            reg.register(
                fn_decl.ident.sym.to_string(),
                TypeDef::Function {
                    params: vec![],
                    return_type: None,
                    has_rest: false,
                },
            );
        }
        ast::Decl::Var(var_decl) => {
            for d in &var_decl.decls {
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(_) = init.as_ref() {
                        let name = match &d.name {
                            ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                            _ => continue,
                        };
                        reg.register(
                            name,
                            TypeDef::Function {
                                params: vec![],
                                return_type: None,
                                has_rest: false,
                            },
                        );
                    }
                }
            }
        }
        ast::Decl::Class(class) => {
            reg.register(
                class.ident.sym.to_string(),
                TypeDef::new_struct(vec![], HashMap::new(), vec![]),
            );
        }
        _ => {}
    }
}

/// Pass 2: 個々の宣言から型情報を完全に収集する。
///
/// `lookup` には Pass 1 で登録された全型名が含まれており、
/// `convert_ts_type` での型解決に使用される。
fn collect_decl(reg: &mut TypeRegistry, decl: &ast::Decl, lookup: &TypeRegistry) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            if let Ok(fields) = collect_interface_fields(iface, lookup) {
                let methods = collect_interface_methods(iface, lookup);
                let extends: Vec<String> = iface
                    .extends
                    .iter()
                    .filter_map(|e| {
                        if let ast::Expr::Ident(ident) = e.expr.as_ref() {
                            Some(ident.sym.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                let name = iface.id.sym.to_string();
                reg.register(name, TypeDef::new_interface(fields, methods, extends));
            }
        }
        ast::Decl::TsTypeAlias(alias) => {
            if let Some(enum_def) = try_collect_string_literal_union(alias) {
                reg.register(alias.id.sym.to_string(), enum_def);
            } else if let Some(enum_def) = try_collect_discriminated_union(alias, lookup) {
                reg.register(alias.id.sym.to_string(), enum_def);
            } else if let Some(func_def) = try_collect_fn_type_alias(alias, lookup) {
                reg.register(alias.id.sym.to_string(), func_def);
            } else {
                // Intersection types need pass-2 resolved types (e.g., `type Person = Named & Aged`
                // requires Named and Aged to have their fields already resolved).
                // Use `reg` which accumulates resolved types during pass 2.
                let fields = collect_type_alias_fields(alias, reg);
                if let Some(fields) = fields {
                    reg.register(
                        alias.id.sym.to_string(),
                        TypeDef::new_struct(fields, HashMap::new(), vec![]),
                    );
                }
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
            reg.register(
                ts_enum.id.sym.to_string(),
                TypeDef::Enum {
                    variants,
                    string_values: HashMap::new(),
                    tag_field: None,
                    variant_fields: HashMap::new(),
                },
            );
        }
        ast::Decl::Fn(fn_decl) => {
            if let Ok(func_def) = collect_fn_def(&fn_decl.function, lookup) {
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
                        if let Ok(func_def) = collect_arrow_def(arrow, lookup) {
                            reg.register(name, func_def);
                        }
                    }
                }
            }
        }
        ast::Decl::Class(class) => {
            let def = collect_class_info(class, lookup);
            if let TypeDef::Struct {
                ref fields,
                ref methods,
                ..
            } = def
            {
                if !fields.is_empty() || !methods.is_empty() {
                    reg.register(class.ident.sym.to_string(), def);
                }
            }
        }
        _ => {}
    }
}

/// クラス宣言からフィールドとメソッドシグネチャを収集し、`TypeDef::Struct` を返す。
fn collect_class_info(class: &ast::ClassDecl, lookup: &TypeRegistry) -> TypeDef {
    let mut fields = Vec::new();
    let mut methods = HashMap::new();

    for member in &class.class.body {
        match member {
            ast::ClassMember::ClassProp(prop) => {
                let name = match &prop.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                        fields.push((name, ty));
                    }
                }
            }
            ast::ClassMember::Method(method) => {
                let name = match &method.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(func) = &method.function.body {
                    let _ = func; // body exists, collect params
                }
                let params: Vec<(String, RustType)> = method
                    .function
                    .params
                    .iter()
                    .filter_map(|param| {
                        let ident = match &param.pat {
                            ast::Pat::Ident(ident) => ident,
                            _ => return None,
                        };
                        let ty = ident.type_ann.as_ref().and_then(|ann| {
                            convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup).ok()
                        })?;
                        Some((ident.id.sym.to_string(), ty))
                    })
                    .collect();
                methods.insert(name, params);
            }
            _ => {}
        }
    }

    TypeDef::new_struct(fields, methods, vec![])
}

/// interface のフィールド名・型を収集する。
fn collect_interface_fields(
    iface: &ast::TsInterfaceDecl,
    lookup: &TypeRegistry,
) -> Result<Vec<(String, RustType)>> {
    let mut fields = Vec::new();
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            if let Some((name, ty)) = collect_property_signature(prop, lookup) {
                fields.push((name, ty));
            }
        }
    }
    Ok(fields)
}

/// interface のメソッドシグネチャを収集する。
fn collect_interface_methods(
    iface: &ast::TsInterfaceDecl,
    lookup: &TypeRegistry,
) -> HashMap<String, Vec<(String, RustType)>> {
    let mut methods = HashMap::new();
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsMethodSignature(method) = member {
            let name = match method.key.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };
            let params: Vec<(String, RustType)> = method
                .params
                .iter()
                .filter_map(|param| {
                    let param_name = match param {
                        ast::TsFnParam::Ident(ident) => ident.id.sym.to_string(),
                        _ => return None,
                    };
                    let ty = match param {
                        ast::TsFnParam::Ident(ident) => {
                            ident.type_ann.as_ref().and_then(|ann| {
                                convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup).ok()
                            })?
                        }
                        _ => return None,
                    };
                    Some((param_name, ty))
                })
                .collect();
            methods.insert(name, params);
        }
    }
    methods
}

/// TsPropertySignature からフィールド名と型を取得する。
fn collect_property_signature(
    prop: &ast::TsPropertySignature,
    lookup: &TypeRegistry,
) -> Option<(String, RustType)> {
    let name = match prop.key.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let ty = prop
        .type_ann
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup).ok())?;

    // Optional fields are wrapped in Option
    let ty = if prop.optional {
        RustType::Option(Box::new(ty))
    } else {
        ty
    };

    Some((name, ty))
}

/// string literal union type alias を検出し、`TypeDef::Enum` を返す。
///
/// `type Direction = "up" | "down"` のように、全メンバーが文字列リテラルの union type を検出する。
fn try_collect_string_literal_union(alias: &ast::TsTypeAliasDecl) -> Option<TypeDef> {
    use crate::transformer::types::string_to_pascal_case;

    let union = match alias.type_ann.as_ref() {
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return None,
    };

    let mut variants = Vec::new();
    let mut string_values = HashMap::new();
    for ty in &union.types {
        match ty.as_ref() {
            ast::TsType::TsLitType(lit) => match &lit.lit {
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    let variant_name = string_to_pascal_case(&value);
                    string_values.insert(value, variant_name.clone());
                    variants.push(variant_name);
                }
                _ => return None,
            },
            _ => return None,
        }
    }

    Some(TypeDef::Enum {
        variants,
        string_values,
        tag_field: None,
        variant_fields: HashMap::new(),
    })
}

/// discriminated union type alias を検出し、`TypeDef::Enum` を返す。
///
/// `type Shape = { kind: "circle", r: number } | { kind: "square", s: number }` を検出する。
/// 全メンバーがオブジェクト型リテラルで、共通の文字列リテラル discriminant フィールドを持つ場合に該当。
fn try_collect_discriminated_union(
    alias: &ast::TsTypeAliasDecl,
    lookup: &TypeRegistry,
) -> Option<TypeDef> {
    use crate::transformer::types::string_to_pascal_case;

    let union = match alias.type_ann.as_ref() {
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return None,
    };

    // All members must be object type literals
    let type_lits: Vec<&swc_ecma_ast::TsTypeLit> = union
        .types
        .iter()
        .filter_map(|ty| match ty.as_ref() {
            ast::TsType::TsTypeLit(lit) => Some(lit),
            _ => None,
        })
        .collect();

    if type_lits.len() != union.types.len() || type_lits.len() < 2 {
        return None;
    }

    // Find a common discriminant field with string literal types in all members
    let tag = find_registry_discriminant_field(&type_lits)?;

    let mut variants = Vec::new();
    let mut string_values = HashMap::new();
    let mut variant_fields_map = HashMap::new();

    for type_lit in &type_lits {
        let (disc_value, fields) = extract_registry_variant_info(type_lit, &tag, lookup)?;
        let variant_name = string_to_pascal_case(&disc_value);
        string_values.insert(disc_value, variant_name.clone());
        variant_fields_map.insert(variant_name.clone(), fields);
        variants.push(variant_name);
    }

    Some(TypeDef::Enum {
        variants,
        string_values,
        tag_field: Some(tag),
        variant_fields: variant_fields_map,
    })
}

/// discriminated union の discriminant フィールドを見つける。
///
/// 全メンバーに共通し、すべて文字列リテラル型であるフィールド名を返す。
fn find_registry_discriminant_field(type_lits: &[&swc_ecma_ast::TsTypeLit]) -> Option<String> {
    let first = type_lits[0];
    for member in &first.members {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            let name = match prop.key.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };
            // Check if this field has a string literal type in all members
            let is_discriminant = type_lits.iter().all(|lit| {
                lit.members.iter().any(|m| {
                    if let ast::TsTypeElement::TsPropertySignature(p) = m {
                        let field_name = match p.key.as_ref() {
                            ast::Expr::Ident(id) => id.sym.to_string(),
                            _ => return false,
                        };
                        if field_name != name {
                            return false;
                        }
                        // Check if type annotation is a string literal
                        if let Some(ann) = &p.type_ann {
                            matches!(
                                ann.type_ann.as_ref(),
                                ast::TsType::TsLitType(lit) if matches!(&lit.lit, swc_ecma_ast::TsLit::Str(_))
                            )
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
            });
            if is_discriminant {
                return Some(name);
            }
        }
    }
    None
}

/// discriminated union の 1 つのバリアントから discriminant 値と非 discriminant フィールドを抽出する。
fn extract_registry_variant_info(
    type_lit: &swc_ecma_ast::TsTypeLit,
    tag_field: &str,
    lookup: &TypeRegistry,
) -> Option<(String, Vec<(String, RustType)>)> {
    let mut disc_value = None;
    let mut fields = Vec::new();

    for member in &type_lit.members {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            let name = match prop.key.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };
            if name == tag_field {
                // Extract string literal value
                if let Some(ann) = &prop.type_ann {
                    if let ast::TsType::TsLitType(lit) = ann.type_ann.as_ref() {
                        if let swc_ecma_ast::TsLit::Str(s) = &lit.lit {
                            disc_value = Some(s.value.to_string_lossy().into_owned());
                        }
                    }
                }
            } else {
                // Non-discriminant field: convert type
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                        let ty = if prop.optional {
                            RustType::Option(Box::new(ty))
                        } else {
                            ty
                        };
                        fields.push((name, ty));
                    }
                }
            }
        }
    }

    Some((disc_value?, fields))
}

/// type alias (オブジェクト型) のフィールドを収集する。
/// 関数型エイリアス (`type F = (x: T) => U`) を `TypeDef::Function` として収集する。
fn try_collect_fn_type_alias(
    alias: &ast::TsTypeAliasDecl,
    lookup: &TypeRegistry,
) -> Option<TypeDef> {
    match alias.type_ann.as_ref() {
        ast::TsType::TsFnOrConstructorType(ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            let mut params = Vec::new();
            for param in &fn_type.params {
                if let ast::TsFnParam::Ident(ident) = param {
                    let name = ident.id.sym.to_string();
                    if let Some(ann) = &ident.type_ann {
                        if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                            params.push((name, ty));
                        }
                    }
                }
            }
            let return_type =
                convert_ts_type(&fn_type.type_ann.type_ann, &mut Vec::new(), lookup).ok();
            Some(TypeDef::Function {
                params,
                return_type,
                has_rest: false,
            })
        }
        _ => None,
    }
}

fn collect_type_alias_fields(
    alias: &ast::TsTypeAliasDecl,
    reg: &TypeRegistry,
) -> Option<Vec<(String, RustType)>> {
    match alias.type_ann.as_ref() {
        ast::TsType::TsTypeLit(lit) => {
            let mut fields = Vec::new();
            for member in &lit.members {
                if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                    if let Some((name, ty)) = collect_property_signature(prop, reg) {
                        fields.push((name, ty));
                    }
                }
            }
            Some(fields)
        }
        // Intersection type: `type Person = Named & Aged` → merge fields from all members
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
        ) => {
            let mut fields = Vec::new();
            for ty in &intersection.types {
                match ty.as_ref() {
                    ast::TsType::TsTypeLit(lit) => {
                        for member in &lit.members {
                            if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                                if let Some(field) = collect_property_signature(prop, reg) {
                                    fields.push(field);
                                }
                            }
                        }
                    }
                    ast::TsType::TsTypeRef(type_ref) => {
                        if let ast::TsEntityName::Ident(ident) = &type_ref.type_name {
                            if let Some(TypeDef::Struct {
                                fields: ref_fields, ..
                            }) = reg.get(ident.sym.as_ref())
                            {
                                fields.extend(ref_fields.iter().cloned());
                            }
                        }
                    }
                    _ => {}
                }
            }
            if fields.is_empty() {
                None
            } else {
                Some(fields)
            }
        }
        _ => None,
    }
}

/// 関数宣言からパラメータ型と戻り値型を収集する。
fn collect_fn_def(func: &ast::Function, lookup: &TypeRegistry) -> Result<TypeDef> {
    let mut params = Vec::new();
    let mut has_rest = false;
    for param in &func.params {
        match &param.pat {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                if let Some(ann) = &ident.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                        params.push((name, ty));
                    }
                }
            }
            ast::Pat::Assign(assign) => {
                // Default parameter: `name: Type = value` → Option<Type>
                if let ast::Pat::Ident(ident) = assign.left.as_ref() {
                    let name = ident.id.sym.to_string();
                    if let Some(ann) = &ident.type_ann {
                        if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                            params.push((name, RustType::Option(Box::new(ty))));
                        }
                    }
                }
            }
            ast::Pat::Rest(rest) => {
                has_rest = true;
                if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                    let name = ident.id.sym.to_string();
                    let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                    if let Some(ann) = type_ann {
                        if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                            params.push((name, ty));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let return_type = func
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup).ok());

    Ok(TypeDef::Function {
        params,
        return_type,
        has_rest,
    })
}

/// アロー関数からパラメータ型と戻り値型を収集する。
fn collect_arrow_def(arrow: &ast::ArrowExpr, lookup: &TypeRegistry) -> Result<TypeDef> {
    let mut params = Vec::new();
    for param in &arrow.params {
        if let ast::Pat::Ident(ident) = param {
            let name = ident.id.sym.to_string();
            if let Some(ann) = &ident.type_ann {
                if let Ok(ty) = convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup) {
                    params.push((name, ty));
                }
            }
        }
    }

    let return_type = arrow
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), lookup).ok());

    Ok(TypeDef::Function {
        params,
        return_type,
        has_rest: false,
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
        let point = TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            HashMap::new(),
            vec![],
        );
        reg.register("Point".to_string(), point.clone());
        let def = reg.get("Point").unwrap();
        assert_eq!(*def, point);
    }

    #[test]
    fn test_registry_register_and_get_enum() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Color".to_string(),
            TypeDef::Enum {
                variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
            },
        );
        let def = reg.get("Color").unwrap();
        assert_eq!(
            *def,
            TypeDef::Enum {
                variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(),],
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
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
                has_rest: false,
            },
        );
        let def = reg.get("draw").unwrap();
        match def {
            TypeDef::Function {
                params,
                return_type,
                ..
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
            TypeDef::new_struct(
                vec![("x".to_string(), RustType::F64)],
                HashMap::new(),
                vec![],
            ),
        );

        let mut reg2 = TypeRegistry::new();
        reg2.register(
            "Color".to_string(),
            TypeDef::Enum {
                variants: vec!["Red".to_string()],
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
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
            &TypeDef::new_interface(
                vec![
                    ("x".to_string(), RustType::F64),
                    ("y".to_string(), RustType::F64),
                ],
                HashMap::new(),
                vec![],
            )
        );
    }

    #[test]
    fn test_build_registry_type_alias_object() {
        let module = parse_typescript("type Config = { name: string; count: number; };").unwrap();
        let reg = build_registry(&module);
        assert_eq!(
            reg.get("Config").unwrap(),
            &TypeDef::new_struct(
                vec![
                    ("name".to_string(), RustType::String),
                    ("count".to_string(), RustType::F64),
                ],
                HashMap::new(),
                vec![],
            )
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
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
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
                ..
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
                ..
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
    fn test_build_registry_fn_rest_param_sets_has_rest_true() {
        let module =
            parse_typescript("function sum(...nums: number[]): number { return 0; }").unwrap();
        let reg = build_registry(&module);
        match reg.get("sum").unwrap() {
            TypeDef::Function {
                params, has_rest, ..
            } => {
                assert!(has_rest, "has_rest should be true for rest param");
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].0, "nums");
                assert_eq!(params[0].1, RustType::Vec(Box::new(RustType::F64)));
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_build_registry_fn_mixed_and_rest_param() {
        let module =
            parse_typescript("function log(prefix: string, ...msgs: string[]): void {}").unwrap();
        let reg = build_registry(&module);
        match reg.get("log").unwrap() {
            TypeDef::Function {
                params, has_rest, ..
            } => {
                assert!(has_rest);
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "prefix");
                assert_eq!(params[0].1, RustType::String);
                assert_eq!(params[1].0, "msgs");
                assert_eq!(params[1].1, RustType::Vec(Box::new(RustType::String)));
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_build_registry_fn_no_rest_param_has_rest_false() {
        let module = parse_typescript("function greet(name: string): void {}").unwrap();
        let reg = build_registry(&module);
        match reg.get("greet").unwrap() {
            TypeDef::Function { has_rest, .. } => {
                assert!(!has_rest, "has_rest should be false without rest param");
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
            &TypeDef::new_interface(
                vec![(
                    "name".to_string(),
                    RustType::Option(Box::new(RustType::String)),
                )],
                HashMap::new(),
                vec![],
            )
        );
    }

    #[test]
    fn test_build_registry_empty_module() {
        let module = parse_typescript("").unwrap();
        let reg = build_registry(&module);
        assert!(reg.get("anything").is_none());
    }

    // --- intersection type registration ---

    #[test]
    fn test_build_registry_intersection_type_alias_merges_fields() {
        let module = parse_typescript(
            "interface Named { name: string; } interface Aged { age: number; } type Person = Named & Aged;",
        )
        .unwrap();
        let reg = build_registry(&module);
        let person = reg.get("Person").expect("Person should be registered");
        match person {
            TypeDef::Struct { fields, .. } => {
                assert_eq!(fields.len(), 2, "expected 2 merged fields");
                assert!(
                    fields
                        .iter()
                        .any(|(n, t)| n == "name" && *t == RustType::String),
                    "expected name: String"
                );
                assert!(
                    fields
                        .iter()
                        .any(|(n, t)| n == "age" && *t == RustType::F64),
                    "expected age: f64"
                );
            }
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    // --- string literal union enum registration ---

    #[test]
    fn test_build_registry_string_literal_union_registers_enum() {
        let module =
            parse_typescript(r#"type Direction = "up" | "down" | "left" | "right";"#).unwrap();
        let reg = build_registry(&module);
        let def = reg
            .get("Direction")
            .expect("Direction should be registered");
        match def {
            TypeDef::Enum {
                variants,
                string_values,
                ..
            } => {
                assert_eq!(variants, &["Up", "Down", "Left", "Right"]);
                assert_eq!(string_values.get("up").unwrap(), "Up");
                assert_eq!(string_values.get("down").unwrap(), "Down");
                assert_eq!(string_values.get("left").unwrap(), "Left");
                assert_eq!(string_values.get("right").unwrap(), "Right");
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    #[test]
    fn test_build_registry_ts_enum_has_empty_string_values() {
        let module = parse_typescript("enum Color { Red, Green, Blue }").unwrap();
        let reg = build_registry(&module);
        match reg.get("Color").unwrap() {
            TypeDef::Enum { string_values, .. } => {
                assert!(
                    string_values.is_empty(),
                    "TS enum should have empty string_values"
                );
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    // --- discriminated union registration ---

    #[test]
    fn test_build_registry_discriminated_union_registers_enum() {
        let module = parse_typescript(
            r#"type Shape = { kind: "circle", radius: number } | { kind: "square", side: number };"#,
        )
        .unwrap();
        let reg = build_registry(&module);
        let def = reg.get("Shape").expect("Shape should be registered");
        match def {
            TypeDef::Enum {
                variants,
                string_values,
                tag_field,
                variant_fields,
            } => {
                assert_eq!(variants, &["Circle", "Square"]);
                assert_eq!(tag_field.as_deref(), Some("kind"));
                assert_eq!(string_values.get("circle").unwrap(), "Circle");
                assert_eq!(string_values.get("square").unwrap(), "Square");
                // Circle variant has radius: f64
                let circle_fields = variant_fields.get("Circle").expect("Circle variant");
                assert_eq!(circle_fields, &[("radius".to_string(), RustType::F64)]);
                // Square variant has side: f64
                let square_fields = variant_fields.get("Square").expect("Square variant");
                assert_eq!(square_fields, &[("side".to_string(), RustType::F64)]);
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    #[test]
    fn test_build_registry_discriminated_union_unit_variant() {
        let module =
            parse_typescript(r#"type Status = { type: "active" } | { type: "inactive" };"#)
                .unwrap();
        let reg = build_registry(&module);
        let def = reg.get("Status").expect("Status should be registered");
        match def {
            TypeDef::Enum {
                variants,
                tag_field,
                variant_fields,
                ..
            } => {
                assert_eq!(variants, &["Active", "Inactive"]);
                assert_eq!(tag_field.as_deref(), Some("type"));
                assert!(
                    variant_fields.get("Active").unwrap().is_empty(),
                    "unit variant should have no fields"
                );
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    // --- Function type alias registration ---

    #[test]
    fn test_build_registry_fn_type_alias_with_params() {
        // type Handler = (c: string) => number;
        let module = parse_typescript("type Handler = (c: string) => number;").unwrap();
        let reg = build_registry(&module);
        let def = reg.get("Handler").expect("Handler should be registered");
        match def {
            TypeDef::Function {
                params,
                return_type,
                ..
            } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].0, "c");
                assert_eq!(params[0].1, RustType::String);
                assert_eq!(*return_type, Some(RustType::F64));
            }
            other => panic!("expected Function, got {other:?}"),
        }
    }

    #[test]
    fn test_build_registry_fn_type_alias_no_params() {
        // type Factory = () => string;
        let module = parse_typescript("type Factory = () => string;").unwrap();
        let reg = build_registry(&module);
        let def = reg.get("Factory").expect("Factory should be registered");
        match def {
            TypeDef::Function {
                params,
                return_type,
                ..
            } => {
                assert!(params.is_empty(), "expected no params, got {:?}", params);
                assert_eq!(*return_type, Some(RustType::String));
            }
            other => panic!("expected Function, got {other:?}"),
        }
    }

    #[test]
    fn test_is_trait_type_methods_only_returns_true() {
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert(
            "greet".to_string(),
            vec![("msg".to_string(), RustType::String)],
        );
        reg.register(
            "Greeter".to_string(),
            TypeDef::new_interface(vec![], methods, vec![]),
        );
        assert!(reg.is_trait_type("Greeter"));
    }

    #[test]
    fn test_is_trait_type_fields_only_returns_false() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Point".to_string(),
            TypeDef::new_interface(
                vec![("x".to_string(), RustType::F64)],
                HashMap::new(),
                vec![],
            ),
        );
        assert!(!reg.is_trait_type("Point"));
    }

    #[test]
    fn test_is_trait_type_mixed_returns_true() {
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert("greet".to_string(), vec![]);
        reg.register(
            "Ctx".to_string(),
            TypeDef::new_interface(
                vec![("name".to_string(), RustType::String)],
                methods,
                vec![],
            ),
        );
        assert!(reg.is_trait_type("Ctx"));
    }

    #[test]
    fn test_is_trait_type_unknown_returns_false() {
        let reg = TypeRegistry::new();
        assert!(!reg.is_trait_type("Unknown"));
    }

    #[test]
    fn test_build_registry_forward_reference_resolves_type() {
        // Interface A references interface B, but A is declared first.
        // With 2-pass construction, B should be registered before A's fields are resolved.
        let module = parse_typescript("interface A { b: B } interface B { x: number; }").unwrap();
        let reg = build_registry(&module);

        // A should have field b with type Named { name: "B" }
        match reg.get("A").unwrap() {
            TypeDef::Struct { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].0, "b");
                assert!(matches!(&fields[0].1, RustType::Named { name, .. } if name == "B"));
            }
            other => panic!("expected Struct, got: {:?}", other),
        }
        // B should also be registered
        assert!(reg.get("B").is_some());
    }
}
