//! TypeRegistry — モジュール内の型定義を事前収集し、変換時に参照するレジストリ。
//!
//! 2-pass 方式で構築する:
//! - **Pass 1**: 型名だけをプレースホルダーとして登録する
//! - **Pass 2**: Pass 1 の型名一覧を参照しながらフィールド型を完全に解決する
//!
//! これにより前方参照（`interface A { b: B }` が `interface B` より前に宣言される場合）
//! でも正しく型を解決できる。

mod collection;
mod enums;
mod functions;
mod interfaces;
mod unions;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use swc_ecma_ast as ast;

use crate::ir::{RustType, TypeParam};
use crate::pipeline::SyntheticTypeRegistry;

pub use collection::collect_type_params;

/// メソッドシグネチャ（パラメータ + 戻り値型）。
#[derive(Debug, Clone, PartialEq)]
pub struct MethodSignature {
    /// パラメータ名と型のペア
    pub params: Vec<(String, RustType)>,
    /// 戻り値型（アノテーションなしの場合は None）
    pub return_type: Option<RustType>,
}

/// 型定義の種類。
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// struct（interface / type alias から変換）
    Struct {
        /// ジェネリック型パラメータ
        type_params: Vec<TypeParam>,
        /// フィールド名と型のペア
        fields: Vec<(String, RustType)>,
        /// メソッドシグネチャ（メソッド名 → オーバーロードを含む全シグネチャ）
        methods: HashMap<String, Vec<MethodSignature>>,
        /// 親 interface 名のリスト（`interface B extends A` の `A`）
        extends: Vec<String>,
        /// Whether this type comes from a TS interface declaration (true) or class/type alias (false)
        is_interface: bool,
    },
    /// enum
    Enum {
        /// ジェネリック型パラメータ
        type_params: Vec<TypeParam>,
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
        methods: HashMap<String, Vec<MethodSignature>>,
        extends: Vec<String>,
    ) -> Self {
        TypeDef::Struct {
            type_params: vec![],
            fields,
            methods,
            extends,
            is_interface: false,
        }
    }

    /// Creates a new interface TypeDef (from TS interface declaration).
    pub fn new_interface(
        type_params: Vec<TypeParam>,
        fields: Vec<(String, RustType)>,
        methods: HashMap<String, Vec<MethodSignature>>,
        extends: Vec<String>,
    ) -> Self {
        TypeDef::Struct {
            type_params,
            fields,
            methods,
            extends,
            is_interface: true,
        }
    }

    /// Returns the type parameters of this TypeDef, if any.
    pub fn type_params(&self) -> &[TypeParam] {
        match self {
            TypeDef::Struct { type_params, .. } | TypeDef::Enum { type_params, .. } => type_params,
            _ => &[],
        }
    }

    /// 型パラメータを具体型で置換した新しい TypeDef を返す。
    pub fn substitute_types(
        &self,
        bindings: &std::collections::HashMap<String, RustType>,
    ) -> TypeDef {
        match self {
            TypeDef::Struct {
                type_params,
                fields,
                methods,
                extends,
                is_interface,
            } => TypeDef::Struct {
                type_params: type_params.clone(),
                fields: fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), ty.substitute(bindings)))
                    .collect(),
                methods: methods
                    .iter()
                    .map(|(name, sigs)| {
                        (
                            name.clone(),
                            sigs.iter()
                                .map(|sig| MethodSignature {
                                    params: sig
                                        .params
                                        .iter()
                                        .map(|(n, ty)| (n.clone(), ty.substitute(bindings)))
                                        .collect(),
                                    return_type: sig
                                        .return_type
                                        .as_ref()
                                        .map(|ty| ty.substitute(bindings)),
                                })
                                .collect(),
                        )
                    })
                    .collect(),
                extends: extends.clone(),
                is_interface: *is_interface,
            },
            TypeDef::Enum {
                type_params,
                variants,
                string_values,
                tag_field,
                variant_fields,
            } => TypeDef::Enum {
                type_params: type_params.clone(),
                variants: variants.clone(),
                string_values: string_values.clone(),
                tag_field: tag_field.clone(),
                variant_fields: variant_fields
                    .iter()
                    .map(|(variant, fields)| {
                        (
                            variant.clone(),
                            fields
                                .iter()
                                .map(|(name, ty)| (name.clone(), ty.substitute(bindings)))
                                .collect(),
                        )
                    })
                    .collect(),
            },
            other => other.clone(),
        }
    }
}

/// モジュール内の型定義を保持するレジストリ。
///
/// 型名をキーにして `TypeDef` を引くことで、変換時にフィールド型や
/// enum バリアントを解決できる。
///
/// 外部型（JSON から読み込まれたビルトイン型）とユーザー定義型（TS ソースから登録された型）を区別するため、
/// 外部型の名前セットを保持する。
#[derive(Debug, Clone)]
pub struct TypeRegistry {
    types: HashMap<String, TypeDef>,
    /// 外部型（JSON ビルトイン定義）として登録された型名のセット。
    /// `register_external` で登録された型のみ含まれる。
    external_types: HashSet<String>,
}

impl TypeRegistry {
    /// 空の TypeRegistry を作成する。
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            external_types: HashSet::new(),
        }
    }

    /// 型定義を登録する。
    pub fn register(&mut self, name: String, def: TypeDef) {
        self.types.insert(name, def);
    }

    /// 外部型（JSON ビルトイン定義）として型定義を登録する。
    ///
    /// 通常の `register` と同じく TypeDef を登録するが、追加で外部型として記録する。
    /// `is_external` で判定可能になる。
    pub fn register_external(&mut self, name: String, def: TypeDef) {
        self.external_types.insert(name.clone());
        self.types.insert(name, def);
    }

    /// 指定された型名が外部型（JSON ビルトイン定義）かどうかを判定する。
    pub fn is_external(&self, name: &str) -> bool {
        self.external_types.contains(name)
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

    /// ジェネリック型を具体型引数でインスタンス化する。
    ///
    /// 型パラメータがない、または引数の数が不一致の場合は元の TypeDef をそのまま返す。
    pub fn instantiate(&self, name: &str, args: &[RustType]) -> Option<TypeDef> {
        let type_def = self.get(name)?;
        let params = type_def.type_params();
        if params.is_empty() || args.len() != params.len() {
            return Some(type_def.clone());
        }
        let bindings: HashMap<String, RustType> = params
            .iter()
            .zip(args.iter())
            .map(|(p, a)| (p.name.clone(), a.clone()))
            .collect();
        Some(type_def.substitute_types(&bindings))
    }

    /// 別の TypeRegistry の内容をマージする。
    ///
    /// 同名の型が既に存在する場合は上書きする。
    pub fn merge(&mut self, other: &TypeRegistry) {
        for (name, def) in &other.types {
            self.types.insert(name.clone(), def.clone());
        }
        for name in &other.external_types {
            self.external_types.insert(name.clone());
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
    let mut synthetic = SyntheticTypeRegistry::new();
    build_registry_with_synthetic(module, &mut synthetic)
}

/// Builds a [`TypeRegistry`] from a module, accumulating synthetic types in the provided registry.
///
/// This is the primary API for the new pipeline (Pass 2). Synthetic types (union enums,
/// inline structs) generated during type conversion are registered in `synthetic` for
/// centralized deduplication.
pub fn build_registry_with_synthetic(
    module: &ast::Module,
    synthetic: &mut SyntheticTypeRegistry,
) -> TypeRegistry {
    let mut reg = TypeRegistry::new();

    // Pass 1: 型名だけをプレースホルダーとして登録する
    for item in &module.body {
        match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => {
                collection::collect_type_name(&mut reg, decl);
            }
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                collection::collect_type_name(&mut reg, &export.decl);
            }
            _ => {}
        }
    }

    // Pass 2: Pass 1 の型名一覧を参照しながらフィールド型を完全に解決する
    let lookup = reg.clone();
    for item in &module.body {
        match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => {
                collection::collect_decl(&mut reg, decl, &lookup, synthetic);
            }
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                collection::collect_decl(&mut reg, &export.decl, &lookup, synthetic);
            }
            _ => {}
        }
    }

    // Register synthetic enum types (generated during type conversion) into the TypeRegistry
    enums::register_extra_enums(&mut reg, synthetic);

    reg
}
