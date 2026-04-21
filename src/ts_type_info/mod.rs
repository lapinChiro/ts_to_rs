//! TypeScript レベルの型表現。
//!
//! SWC AST から抽出した型情報を保持する中間表現。
//! RustType への変換前に TypeDef に格納される。
//!
//! ## 設計方針
//!
//! - SWC の `TsType` は `Clone` 未実装で保存不可 → 自前の所有型として定義
//! - 純粋に構文的な TS 型情報のみ保持（Rust 固有の変換は含まない）
//! - TypeRegistry 不要で TsType → TsTypeInfo への変換が可能
//!
//! ## Module layout
//!
//! - [`self`] — types (`TsTypeInfo` / structs) + main dispatcher
//!   [`convert_to_ts_type_info`]
//! - [`helpers`] — subtree converters invoked by the dispatcher
//!   (`convert_type_lit_members`, `extract_sig_params`,
//!   `extract_fn_params`, `extract_entity_name`). Kept `pub(super)` —
//!   not part of the crate's public API.
//! - [`resolve`] — `TsTypeInfo` → `RustType` resolution (public
//!   sub-module, unchanged)

mod helpers;
pub mod resolve;

use self::helpers::{extract_entity_name, extract_fn_params};

/// TypeScript レベルの型表現。
///
/// SWC AST から抽出した型情報を、所有型として保持する。
/// `convert_ts_type_info` で SWC AST から変換し、
/// `resolve_to_rust_type` で RustType に変換する。
#[derive(Debug, Clone, PartialEq)]
pub enum TsTypeInfo {
    // ── Keyword types ──
    /// TS `string`
    String,
    /// TS `number`
    Number,
    /// TS `boolean`
    Boolean,
    /// TS `void`
    Void,
    /// TS `null`
    Null,
    /// TS `undefined`
    Undefined,
    /// TS `never`
    Never,
    /// TS `any`
    Any,
    /// TS `unknown`
    Unknown,
    /// TS `object` keyword
    Object,
    /// TS `bigint`
    BigInt,
    /// TS `symbol`
    Symbol,

    // ── Composite types ──
    /// TS `T[]` or `Array<T>`
    Array(Box<TsTypeInfo>),
    /// TS `[T, U, ...]`
    Tuple(Vec<TsTypeInfo>),
    /// TS `T | U | ...`
    Union(Vec<TsTypeInfo>),
    /// TS `T & U & ...`
    Intersection(Vec<TsTypeInfo>),
    /// TS `(x: T, y?: U, ...) => V`
    ///
    /// `TsParamInfo` でパラメータを保持することで `optional` フラグを
    /// 伝播する (I-040)。下流の `resolve_ts_type` が `RustType::Fn` に
    /// 変換する際、`optional == true` のパラメータは `Option<T>` にラップされる。
    Function {
        /// パラメータ情報（型 + optional）
        params: Vec<TsParamInfo>,
        /// 戻り値型
        return_type: Box<TsTypeInfo>,
    },

    // ── Reference types ──
    /// TS named type reference（例: `Foo`, `Array<T>`, `Promise<T>`, `Partial<T>`）
    ///
    /// ユーティリティ型（Partial, Required, Pick, Omit 等）も未解決のまま保持する。
    /// 解決は TsTypeInfo → RustType 変換時に行う。
    TypeRef {
        /// 型名
        name: std::string::String,
        /// 型引数
        type_args: Vec<TsTypeInfo>,
    },

    // ── Literal types ──
    /// TS literal type（`"foo"`, `42`, `true`）
    Literal(TsLiteralKind),

    // ── Structural types ──
    /// TS type literal（`{ key: Type; method(): U; ... }`）
    ///
    /// プロパティ・メソッド・call/construct/index シグネチャを含む完全な型リテラル表現。
    TypeLiteral(TsTypeLiteralInfo),
    /// TS mapped type `{ [K in C]: V }` / `{ readonly [K in C]?: V }`
    Mapped {
        /// 型パラメータ名
        type_param: std::string::String,
        /// 制約型
        constraint: Box<TsTypeInfo>,
        /// 値型
        value: Option<Box<TsTypeInfo>>,
        /// readonly 修飾子あり（`+readonly` / `readonly`）
        has_readonly: bool,
        /// optional 修飾子あり（`+?` / `?`）
        has_optional: bool,
        /// name type（`as` clause）: `[K in C as N]: V` の `N`
        name_type: Option<Box<TsTypeInfo>>,
    },

    // ── Advanced types ──
    /// TS conditional type `C extends E ? T : F`
    Conditional {
        /// チェック型
        check: Box<TsTypeInfo>,
        /// extends 制約
        extends: Box<TsTypeInfo>,
        /// true ブランチ
        true_type: Box<TsTypeInfo>,
        /// false ブランチ
        false_type: Box<TsTypeInfo>,
    },
    /// TS indexed access `T[K]`
    IndexedAccess {
        /// オブジェクト型
        object: Box<TsTypeInfo>,
        /// インデックス型
        index: Box<TsTypeInfo>,
    },
    /// TS `keyof T`
    KeyOf(Box<TsTypeInfo>),
    /// TS `typeof X`
    TypeQuery(std::string::String),
    /// TS `readonly T` (type operator)
    Readonly(Box<TsTypeInfo>),
    /// TS `infer T` (conditional type の extends 内で使用)
    Infer(std::string::String),
    /// TS type predicate `x is Type` → boolean at runtime
    TypePredicate,
}

/// TS リテラル型の種類。
#[derive(Debug, Clone, PartialEq)]
pub enum TsLiteralKind {
    /// 文字列リテラル（例: `"foo"`）
    String(std::string::String),
    /// 数値リテラル（例: `42`）
    Number(f64),
    /// 真偽値リテラル（例: `true`）
    Boolean(bool),
    /// BigInt リテラル（例: `100n`）
    BigInt(std::string::String),
    /// テンプレートリテラル型
    Template,
}

/// TS object type literal のフィールド情報。
///
/// `{ key: Type; key?: OptType }` の各メンバーを表す。
#[derive(Debug, Clone, PartialEq)]
pub struct TsFieldInfo {
    /// フィールド名
    pub name: std::string::String,
    /// フィールド型
    pub ty: TsTypeInfo,
    /// optional property か（`?:` 付き）
    pub optional: bool,
}

/// TS type literal の全メンバー情報。
///
/// `{ key: T; method(): U; (x: T): U; new (x: T): U; [k: string]: V }` を表現する。
/// SWC の `TsTypeLit` から抽出した情報を所有型として保持する。
#[derive(Debug, Clone, PartialEq)]
pub struct TsTypeLiteralInfo {
    /// プロパティシグネチャ（`key: T`, `key?: T`）
    pub fields: Vec<TsFieldInfo>,
    /// メソッドシグネチャ（`method(x: T): U`）
    pub methods: Vec<TsMethodInfo>,
    /// コールシグネチャ（`(x: T): U`）
    pub call_signatures: Vec<TsFnSigInfo>,
    /// コンストラクトシグネチャ（`new (x: T): U`）
    pub construct_signatures: Vec<TsFnSigInfo>,
    /// インデックスシグネチャ（`[key: string]: T`）
    pub index_signatures: Vec<TsIndexSigInfo>,
}

/// TS メソッドシグネチャ情報。
///
/// `method(x: T, y?: U): V` を表現する。
#[derive(Debug, Clone, PartialEq)]
pub struct TsMethodInfo {
    /// メソッド名
    pub name: std::string::String,
    /// パラメータ
    pub params: Vec<TsParamInfo>,
    /// 戻り値型
    pub return_type: Option<TsTypeInfo>,
    /// 型パラメータ名（`method<T, U>()` の `T`, `U`）
    pub type_params: Vec<std::string::String>,
    /// optional メソッドか（`method?(): T`）
    pub optional: bool,
    /// rest パラメータを含むか（`...args: T[]` パターン）
    pub has_rest: bool,
}

/// TS 関数シグネチャ情報（call/construct シグネチャ共通）。
///
/// `(x: T, ...rest: U[]): V` を表現する。
#[derive(Debug, Clone, PartialEq)]
pub struct TsFnSigInfo {
    /// パラメータ
    pub params: Vec<TsParamInfo>,
    /// 戻り値型
    pub return_type: Option<TsTypeInfo>,
    /// rest パラメータを含むか
    pub has_rest: bool,
}

/// TS パラメータ情報。
///
/// メソッド/コール/コンストラクトシグネチャのパラメータを表す。
#[derive(Debug, Clone, PartialEq)]
pub struct TsParamInfo {
    /// パラメータ名
    pub name: std::string::String,
    /// パラメータ型
    pub ty: TsTypeInfo,
    /// optional パラメータか（`x?: T`）
    pub optional: bool,
}

/// TS インデックスシグネチャ情報。
///
/// `[key: string]: T` や `readonly [key: number]: T` を表現する。
#[derive(Debug, Clone, PartialEq)]
pub struct TsIndexSigInfo {
    /// インデックスパラメータ名（例: `key`）
    pub param_name: std::string::String,
    /// インデックスパラメータ型（通常 `string` or `number`）
    pub param_type: TsTypeInfo,
    /// 値型
    pub value_type: TsTypeInfo,
    /// readonly か
    pub readonly: bool,
}

/// SWC の `TsType` AST ノードから `TsTypeInfo` に変換する。
///
/// 純粋に構文的なマッピングのみ行い、TypeRegistry は不要。
/// 型参照は `TsTypeInfo::TypeRef` として未解決のまま保持する。
pub fn convert_to_ts_type_info(ts_type: &swc_ecma_ast::TsType) -> anyhow::Result<TsTypeInfo> {
    use swc_ecma_ast::{self as ast, TsKeywordTypeKind};

    match ts_type {
        ast::TsType::TsKeywordType(kw) => match kw.kind {
            TsKeywordTypeKind::TsStringKeyword => Ok(TsTypeInfo::String),
            TsKeywordTypeKind::TsNumberKeyword => Ok(TsTypeInfo::Number),
            TsKeywordTypeKind::TsBooleanKeyword => Ok(TsTypeInfo::Boolean),
            TsKeywordTypeKind::TsVoidKeyword => Ok(TsTypeInfo::Void),
            TsKeywordTypeKind::TsAnyKeyword => Ok(TsTypeInfo::Any),
            TsKeywordTypeKind::TsUnknownKeyword => Ok(TsTypeInfo::Unknown),
            TsKeywordTypeKind::TsNeverKeyword => Ok(TsTypeInfo::Never),
            TsKeywordTypeKind::TsObjectKeyword => Ok(TsTypeInfo::Object),
            TsKeywordTypeKind::TsNullKeyword => Ok(TsTypeInfo::Null),
            TsKeywordTypeKind::TsUndefinedKeyword => Ok(TsTypeInfo::Undefined),
            TsKeywordTypeKind::TsBigIntKeyword => Ok(TsTypeInfo::BigInt),
            TsKeywordTypeKind::TsSymbolKeyword => Ok(TsTypeInfo::Symbol),
            other => Err(anyhow::anyhow!("unsupported keyword type: {:?}", other)),
        },

        ast::TsType::TsArrayType(arr) => {
            let inner = convert_to_ts_type_info(&arr.elem_type)?;
            Ok(TsTypeInfo::Array(Box::new(inner)))
        }

        ast::TsType::TsTypeRef(type_ref) => {
            let name = extract_entity_name(&type_ref.type_name);
            let type_args = type_ref
                .type_params
                .as_ref()
                .map(|params| {
                    params
                        .params
                        .iter()
                        .map(|p| convert_to_ts_type_info(p))
                        .collect::<anyhow::Result<Vec<_>>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(TsTypeInfo::TypeRef { name, type_args })
        }

        ast::TsType::TsUnionOrIntersectionType(ast::TsUnionOrIntersectionType::TsUnionType(
            union,
        )) => {
            let members = union
                .types
                .iter()
                .map(|t| convert_to_ts_type_info(t))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(TsTypeInfo::Union(members))
        }

        ast::TsType::TsUnionOrIntersectionType(
            ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
        ) => {
            let members = intersection
                .types
                .iter()
                .map(|t| convert_to_ts_type_info(t))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(TsTypeInfo::Intersection(members))
        }

        ast::TsType::TsParenthesizedType(paren) => convert_to_ts_type_info(&paren.type_ann),

        ast::TsType::TsFnOrConstructorType(ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            let params = extract_fn_params(&fn_type.params);
            let return_type = Box::new(convert_to_ts_type_info(&fn_type.type_ann.type_ann)?);
            Ok(TsTypeInfo::Function {
                params,
                return_type,
            })
        }
        ast::TsType::TsFnOrConstructorType(ast::TsFnOrConstructorType::TsConstructorType(
            ctor_type,
        )) => {
            // `new (x: T) => U` → Function type（コンストラクタとアロー関数は同じシグネチャ構造）
            let params = extract_fn_params(&ctor_type.params);
            let return_type = Box::new(convert_to_ts_type_info(&ctor_type.type_ann.type_ann)?);
            Ok(TsTypeInfo::Function {
                params,
                return_type,
            })
        }

        ast::TsType::TsTupleType(tuple) => {
            let elems = tuple
                .elem_types
                .iter()
                .map(|elem| convert_to_ts_type_info(&elem.ty))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(TsTypeInfo::Tuple(elems))
        }

        ast::TsType::TsIndexedAccessType(indexed) => {
            let object = Box::new(convert_to_ts_type_info(&indexed.obj_type)?);
            let index = Box::new(convert_to_ts_type_info(&indexed.index_type)?);
            Ok(TsTypeInfo::IndexedAccess { object, index })
        }

        ast::TsType::TsTypeLit(type_lit) => {
            let info = helpers::convert_type_lit_members(&type_lit.members)?;
            Ok(TsTypeInfo::TypeLiteral(info))
        }

        ast::TsType::TsLitType(lit) => {
            let kind = match &lit.lit {
                ast::TsLit::Str(s) => TsLiteralKind::String(s.value.to_string_lossy().into_owned()),
                ast::TsLit::Number(n) => TsLiteralKind::Number(n.value),
                ast::TsLit::Bool(b) => TsLiteralKind::Boolean(b.value),
                ast::TsLit::BigInt(bi) => TsLiteralKind::BigInt(format!("{}", bi.value)),
                ast::TsLit::Tpl(_) => TsLiteralKind::Template,
            };
            Ok(TsTypeInfo::Literal(kind))
        }

        ast::TsType::TsConditionalType(cond) => Ok(TsTypeInfo::Conditional {
            check: Box::new(convert_to_ts_type_info(&cond.check_type)?),
            extends: Box::new(convert_to_ts_type_info(&cond.extends_type)?),
            true_type: Box::new(convert_to_ts_type_info(&cond.true_type)?),
            false_type: Box::new(convert_to_ts_type_info(&cond.false_type)?),
        }),

        ast::TsType::TsMappedType(mapped) => {
            let type_param = mapped.type_param.name.sym.to_string();
            let constraint = Box::new(
                mapped
                    .type_param
                    .constraint
                    .as_ref()
                    .map(|c| convert_to_ts_type_info(c))
                    .transpose()?
                    .unwrap_or(TsTypeInfo::Any),
            );
            let value = mapped
                .type_ann
                .as_ref()
                .map(|ann| convert_to_ts_type_info(ann))
                .transpose()?
                .map(Box::new);
            let has_readonly = mapped.readonly.is_some();
            let has_optional = mapped.optional.is_some();
            let name_type = mapped
                .name_type
                .as_ref()
                .map(|nt| convert_to_ts_type_info(nt))
                .transpose()?
                .map(Box::new);
            Ok(TsTypeInfo::Mapped {
                type_param,
                constraint,
                value,
                has_readonly,
                has_optional,
                name_type,
            })
        }

        ast::TsType::TsTypePredicate(_) => Ok(TsTypeInfo::TypePredicate),

        ast::TsType::TsInferType(infer) => {
            Ok(TsTypeInfo::Infer(infer.type_param.name.sym.to_string()))
        }

        ast::TsType::TsTypeOperator(op) => {
            use ast::TsTypeOperatorOp;
            match op.op {
                TsTypeOperatorOp::ReadOnly => {
                    let inner = convert_to_ts_type_info(&op.type_ann)?;
                    Ok(TsTypeInfo::Readonly(Box::new(inner)))
                }
                TsTypeOperatorOp::KeyOf => {
                    let inner = convert_to_ts_type_info(&op.type_ann)?;
                    Ok(TsTypeInfo::KeyOf(Box::new(inner)))
                }
                _ => Err(anyhow::anyhow!("unsupported type operator: {:?}", op.op)),
            }
        }

        ast::TsType::TsTypeQuery(query) => {
            let name = match &query.expr_name {
                ast::TsTypeQueryExpr::TsEntityName(entity) => extract_entity_name(entity),
                _ => {
                    return Err(anyhow::anyhow!("unsupported typeof expression"));
                }
            };
            Ok(TsTypeInfo::TypeQuery(name))
        }

        _ => Err(anyhow::anyhow!("unsupported type: {:?}", ts_type)),
    }
}

#[cfg(test)]
mod tests;
