//! TypeRegistry — モジュール内の型定義を事前収集し、変換時に参照するレジストリ。
//!
//! 2-pass 方式で構築する:
//! - **Pass 1**: 型名だけをプレースホルダーとして登録する
//! - **Pass 2**: Pass 1 の型名一覧を参照しながらフィールド型を完全に解決する
//!
//! これにより前方参照（`interface A { b: B }` が `interface B` より前に宣言される場合）
//! でも正しく型を解決できる。

pub(crate) mod collection;
mod enums;
mod functions;
pub(crate) mod interfaces;
mod swc_method_kind;
mod unions;

pub(crate) use enums::register_extra_enums;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use swc_ecma_ast as ast;

use crate::ir::{RustType, TypeParam};
use crate::pipeline::SyntheticTypeRegistry;

pub(crate) use collection::collect_type_params;

/// TypeDef のフィールド定義。optional フラグを TS メタデータとして保持する。
///
/// TypeDef::Struct の fields や TypeDef::Enum の variant_fields で使用される。
/// `optional: true` のフィールドは変換フェーズで `Option<T>` にラップされる。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef<T = RustType> {
    /// フィールド名
    pub name: String,
    /// フィールド型
    pub ty: T,
    /// TS の optional property (`?:`) か
    pub optional: bool,
}

impl FieldDef {
    /// 非 optional なフィールド定義を生成する便利コンストラクタ。
    pub fn new(name: String, ty: RustType) -> Self {
        Self {
            name,
            ty,
            optional: false,
        }
    }

    /// 型パラメータを具体型で置換した新しい FieldDef を返す。
    pub fn substitute(&self, bindings: &std::collections::HashMap<String, RustType>) -> FieldDef {
        FieldDef {
            name: self.name.clone(),
            ty: self.ty.substitute(bindings),
            optional: self.optional,
        }
    }
}

impl From<(String, RustType)> for FieldDef {
    fn from((name, ty): (String, RustType)) -> Self {
        Self::new(name, ty)
    }
}

/// TypeDef のパラメータ定義。optional / has_default フラグを TS メタデータとして保持する。
///
/// TypeDef::Function の params や MethodSignature の params で使用される。
/// `has_default: true` のパラメータは変換フェーズで `Option<T>` にラップされる。
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef<T = RustType> {
    /// パラメータ名
    pub name: String,
    /// パラメータ型
    pub ty: T,
    /// TS の optional parameter (`?:`) か
    pub optional: bool,
    /// TS のデフォルトパラメータ (`= value`) か
    pub has_default: bool,
}

impl ParamDef {
    /// 非 optional・デフォルトなしのパラメータ定義を生成する便利コンストラクタ。
    pub fn new(name: String, ty: RustType) -> Self {
        Self {
            name,
            ty,
            optional: false,
            has_default: false,
        }
    }

    /// 型パラメータを具体型で置換した新しい ParamDef を返す。
    pub fn substitute(&self, bindings: &std::collections::HashMap<String, RustType>) -> ParamDef {
        ParamDef {
            name: self.name.clone(),
            ty: self.ty.substitute(bindings),
            optional: self.optional,
            has_default: self.has_default,
        }
    }
}

impl From<(String, RustType)> for ParamDef {
    fn from((name, ty): (String, RustType)) -> Self {
        Self::new(name, ty)
    }
}

/// `MethodKind` の re-export (foundational module は `crate::ir::MethodKind`)。
///
/// I-205 T1-T3 batch `/check_job` 4-layer review (2026-04-28) で原因 1 (foundational
/// placement 不在 → module circular dep registry ↔ ts_type_info) を発見、`MethodKind` を
/// `src/ir/method_kind.rs` に move。本 re-export は既存 51 site (`crate::registry::MethodKind`
/// 参照) の backward compat を維持する目的。新規参照は `crate::ir::MethodKind` を推奨。
pub use crate::ir::MethodKind;

/// メソッドシグネチャ（パラメータ + 戻り値型 + rest パラメータ情報 + メソッド固有の generic 型パラメータ + method kind）。
#[derive(Debug, Clone, PartialEq)]
pub struct MethodSignature<T = RustType> {
    /// パラメータ定義
    pub params: Vec<ParamDef<T>>,
    /// 戻り値型（アノテーションなしの場合は None）
    pub return_type: Option<T>,
    /// 最後のパラメータが rest パラメータか（`...args: T[]` パターン）
    pub has_rest: bool,
    /// メソッド自身の generic 型パラメータ。
    ///
    /// I-383 T8': `class C<S> { foo<M extends string>(x: M | M[]): void }` のような
    /// generic メソッドで `<M>` を保持する。`resolve_method_sig` が scope に push し、
    /// メソッド本体の `M | M[]` 等の anonymous union が generic 化される。
    pub type_params: Vec<TypeParam<T>>,
    /// メソッド種別 (Method / Getter / Setter)。
    ///
    /// I-205: SWC `ClassMethod.kind` を propagate、`resolve_member_access` /
    /// `dispatch_member_write` での getter/setter dispatch 判別に利用。default は
    /// `MethodKind::Method` で既存 test fixture / constructor signature の backward
    /// compat を維持する。
    pub kind: MethodKind,
}

/// `MethodSignature<T>` の Default 実装。
///
/// I-383 T8': `type_params` フィールド追加に伴い、test fixture や builder pattern で
/// `..Default::default()` を使って type_params のみ補完できるようにする。手動実装の理由:
/// `T` に Default 制約を課さずに `Vec<T>` 系フィールドのみ default 化するため (`Option<T>::None`、
/// `Vec<X>::new()` は T が Default でなくても動く)。
impl<T> Default for MethodSignature<T> {
    fn default() -> Self {
        Self {
            params: Vec::new(),
            return_type: None,
            has_rest: false,
            type_params: Vec::new(),
            kind: MethodKind::Method,
        }
    }
}

impl MethodSignature {
    /// 型パラメータを具体型で置換した新しい MethodSignature を返す。
    pub fn substitute(
        &self,
        bindings: &std::collections::HashMap<String, RustType>,
    ) -> MethodSignature {
        MethodSignature {
            params: self.params.iter().map(|p| p.substitute(bindings)).collect(),
            return_type: self.return_type.as_ref().map(|ty| ty.substitute(bindings)),
            has_rest: self.has_rest,
            kind: self.kind,
            // メソッド自身の type_params は substitute 対象外 (上位 scope の binding と
            // 衝突する場合は scope shadowing で別変数として扱う設計)
            type_params: self.type_params.clone(),
        }
    }
}

/// Selects the best matching overload from a set of method signatures.
///
/// Returns the index and the full `MethodSignature` so callers can extract both
/// parameter types and return type from the **same** signature, avoiding inconsistency.
/// The index corresponds to the position in the input `sigs` slice (0-based).
///
/// Resolution strategy (4 stages):
/// 1. Single signature → use it
/// 2. Filter by argument count → if exactly one matches, use it
/// 3. Filter by argument type compatibility → if exactly one matches, use it
/// 4. Fallback: first signature
pub fn select_overload<'a>(
    sigs: &'a [MethodSignature],
    arg_count: usize,
    arg_types: &[Option<RustType>],
) -> (usize, &'a MethodSignature) {
    debug_assert!(
        !sigs.is_empty(),
        "select_overload called with empty signatures"
    );

    // Stage 1: single signature
    if sigs.len() == 1 {
        return (0, &sigs[0]);
    }

    // Stage 2: filter by argument count
    let by_count: Vec<(usize, &MethodSignature)> = sigs
        .iter()
        .enumerate()
        .filter(|(_, sig)| sig.params.len() == arg_count)
        .collect();
    if by_count.len() == 1 {
        return by_count[0];
    }

    // Stage 3: filter by argument type compatibility
    let candidates: Vec<(usize, &MethodSignature)> = if by_count.is_empty() {
        sigs.iter().enumerate().collect()
    } else {
        by_count
    };
    if arg_types.iter().any(|t| t.is_some()) {
        let compatible: Vec<&(usize, &MethodSignature)> = candidates
            .iter()
            .filter(|(_, sig)| {
                sig.params
                    .iter()
                    .zip(arg_types.iter())
                    .all(|(param, arg_ty)| match arg_ty {
                        Some(at) => at == &param.ty,
                        None => true,
                    })
            })
            .collect();
        if compatible.len() == 1 {
            return *compatible[0];
        }
    }

    // Stage 4: fallback to first candidate (respects arity filter from Stage 2)
    candidates[0]
}

/// `ConstValue` のオブジェクトフィールド。
#[derive(Debug, Clone, PartialEq)]
pub struct ConstField<T = RustType> {
    /// フィールド名
    pub name: String,
    /// フィールド型
    pub ty: T,
    /// `as const` オブジェクトの文字列リテラル値（`{ key: 'value' } as const` の場合）
    pub string_literal_value: Option<String>,
}

impl ConstField {
    /// 型パラメータを具体型で置換した新しい ConstField を返す。
    pub fn substitute(&self, bindings: &std::collections::HashMap<String, RustType>) -> ConstField {
        ConstField {
            name: self.name.clone(),
            ty: self.ty.substitute(bindings),
            string_literal_value: self.string_literal_value.clone(),
        }
    }
}

/// `ConstValue` の配列要素。
#[derive(Debug, Clone, PartialEq)]
pub struct ConstElement<T = RustType> {
    /// 要素の型
    pub ty: T,
    /// `as const` 配列の文字列リテラル値（`['a', 'b'] as const` の場合に保持）
    pub string_literal_value: Option<String>,
}

impl ConstElement {
    /// 型パラメータを具体型で置換した新しい ConstElement を返す。
    pub fn substitute(
        &self,
        bindings: &std::collections::HashMap<String, RustType>,
    ) -> ConstElement {
        ConstElement {
            ty: self.ty.substitute(bindings),
            string_literal_value: self.string_literal_value.clone(),
        }
    }
}

/// 型定義の種類。
///
/// 型パラメータ `T` によって保持する型表現を切り替える:
/// - `TypeDef<RustType>` (= `TypeDef`): Rust 型表現。TypeRegistry に格納され、コンシューマが使用。
/// - `TypeDef<TsTypeInfo>`: TS 型表現。registry フェーズの内部で使用。
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef<T = RustType> {
    /// struct（interface / type alias から変換）
    Struct {
        /// ジェネリック型パラメータ
        type_params: Vec<TypeParam<T>>,
        /// フィールド定義
        fields: Vec<FieldDef<T>>,
        /// メソッドシグネチャ（メソッド名 → オーバーロードを含む全シグネチャ）
        methods: HashMap<String, Vec<MethodSignature<T>>>,
        /// コンストラクタシグネチャ（オーバーロード対応）
        constructor: Option<Vec<MethodSignature<T>>>,
        /// Call signatures for callable interfaces.
        /// e.g., `interface GetCookie { (c: Context): Cookie; (c: Context, key: string): string }`
        call_signatures: Vec<MethodSignature<T>>,
        /// 親 interface 名のリスト（`interface B extends A` の `A`）
        extends: Vec<String>,
        /// Whether this type comes from a TS interface declaration (true) or class/type alias (false)
        is_interface: bool,
    },
    /// enum
    Enum {
        /// ジェネリック型パラメータ
        type_params: Vec<TypeParam<T>>,
        /// バリアント名の一覧
        variants: Vec<String>,
        /// 文字列リテラル値 → バリアント名のマッピング（string literal union / discriminated union）
        string_values: HashMap<String, String>,
        /// discriminated union の tag フィールド名（例: "kind"）
        tag_field: Option<String>,
        /// バリアント名 → フィールド定義のマッピング（discriminated union のみ）
        variant_fields: HashMap<String, Vec<FieldDef<T>>>,
    },
    /// 関数
    Function {
        /// ジェネリック型パラメータ
        type_params: Vec<TypeParam<T>>,
        /// パラメータ定義
        params: Vec<ParamDef<T>>,
        /// 戻り値型
        return_type: Option<T>,
        /// 最後のパラメータが rest パラメータかどうか
        has_rest: bool,
    },
    /// const 変数の値型（`as const` 宣言または型注釈付き const 宣言）
    ConstValue {
        /// const オブジェクトのフィールド
        fields: Vec<ConstField<T>>,
        /// const 配列の要素（`as const` 配列リテラルから抽出）
        elements: Vec<ConstElement<T>>,
        /// 型注釈の参照先型名（`const x: Config = ...` → `Some("Config")`）
        /// TsTypeQuery ハンドラで typeof 解決時にこの型名へリダイレクトする
        type_ref_name: Option<String>,
    },
}

impl TypeDef {
    /// Creates a new struct TypeDef (from class, type alias, or other non-interface source).
    pub fn new_struct(
        fields: Vec<FieldDef>,
        methods: HashMap<String, Vec<MethodSignature>>,
        extends: Vec<String>,
    ) -> Self {
        TypeDef::Struct {
            type_params: vec![],
            fields,
            methods,
            constructor: None,
            call_signatures: vec![],
            extends,
            is_interface: false,
        }
    }

    /// Creates a new interface TypeDef (from TS interface declaration).
    pub fn new_interface(
        type_params: Vec<TypeParam>,
        fields: Vec<FieldDef>,
        methods: HashMap<String, Vec<MethodSignature>>,
        extends: Vec<String>,
    ) -> Self {
        TypeDef::Struct {
            type_params,
            fields,
            methods,
            constructor: None,
            call_signatures: vec![],
            extends,
            is_interface: true,
        }
    }

    /// Returns the type parameters of this TypeDef, if any.
    pub fn type_params(&self) -> &[TypeParam] {
        match self {
            TypeDef::Struct { type_params, .. }
            | TypeDef::Enum { type_params, .. }
            | TypeDef::Function { type_params, .. } => type_params,
            TypeDef::ConstValue { .. } => &[],
        }
    }

    /// `Struct` または `ConstValue` のフィールド名一覧を返す。
    ///
    /// フィールドが空の場合は `None`。`Enum`/`Function` に対しても `None`。
    /// `keyof typeof X` の解決で、const オブジェクトのキー名を取得する際に使用する。
    pub fn field_names(&self) -> Option<Vec<String>> {
        match self {
            TypeDef::Struct { fields, .. } if !fields.is_empty() => {
                Some(fields.iter().map(|f| f.name.clone()).collect())
            }
            TypeDef::ConstValue { fields, .. } if !fields.is_empty() => {
                Some(fields.iter().map(|f| f.name.clone()).collect())
            }
            TypeDef::Struct { .. }
            | TypeDef::ConstValue { .. }
            | TypeDef::Enum { .. }
            | TypeDef::Function { .. } => None,
        }
    }

    /// `Struct` または `ConstValue` のフィールド値型を重複なしで返す。
    ///
    /// フィールドが空の場合は `None`。
    /// `(typeof X)[keyof typeof X]` の解決で、全値型の union を構築する際に使用する。
    pub fn unique_field_types(&self) -> Option<Vec<RustType>> {
        let types_iter: Box<dyn Iterator<Item = &RustType>> = match self {
            TypeDef::Struct { fields, .. } if !fields.is_empty() => {
                Box::new(fields.iter().map(|f| &f.ty))
            }
            TypeDef::ConstValue { fields, .. } if !fields.is_empty() => {
                Box::new(fields.iter().map(|f| &f.ty))
            }
            TypeDef::Struct { .. }
            | TypeDef::ConstValue { .. }
            | TypeDef::Enum { .. }
            | TypeDef::Function { .. } => return None,
        };
        let mut unique = Vec::new();
        for ty in types_iter {
            if !unique.contains(ty) {
                unique.push(ty.clone());
            }
        }
        Some(unique)
    }

    /// `ConstValue` の全フィールドが文字列リテラル値を持つ場合、その値一覧を返す。
    ///
    /// 一つでも `string_literal_value` が `None` のフィールドがあれば `None` を返す。
    /// `ConstValue` 以外の TypeDef に対しても `None`。
    /// `(typeof X)[keyof typeof X]` で全値が文字列リテラルの場合に string enum を生成する際に使用する。
    pub fn all_string_literal_field_values(&self) -> Option<Vec<String>> {
        if let TypeDef::ConstValue { fields, .. } = self {
            if fields.is_empty() {
                return None;
            }
            let values: Vec<String> = fields
                .iter()
                .filter_map(|f| f.string_literal_value.clone())
                .collect();
            if values.len() == fields.len() {
                return Some(values);
            }
        }
        None
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
                constructor,
                call_signatures,
                extends,
                is_interface,
            } => {
                let substitute_sigs = |sigs: &[MethodSignature]| -> Vec<MethodSignature> {
                    sigs.iter().map(|s| s.substitute(bindings)).collect()
                };
                TypeDef::Struct {
                    type_params: type_params
                        .iter()
                        .map(|tp| tp.substitute(bindings))
                        .collect(),
                    fields: fields.iter().map(|f| f.substitute(bindings)).collect(),
                    methods: methods
                        .iter()
                        .map(|(name, sigs)| (name.clone(), substitute_sigs(sigs)))
                        .collect(),
                    constructor: constructor.as_ref().map(|sigs| substitute_sigs(sigs)),
                    call_signatures: substitute_sigs(call_signatures),
                    extends: extends.clone(),
                    is_interface: *is_interface,
                }
            }
            TypeDef::Enum {
                type_params,
                variants,
                string_values,
                tag_field,
                variant_fields,
            } => TypeDef::Enum {
                type_params: type_params
                    .iter()
                    .map(|tp| tp.substitute(bindings))
                    .collect(),
                variants: variants.clone(),
                string_values: string_values.clone(),
                tag_field: tag_field.clone(),
                variant_fields: variant_fields
                    .iter()
                    .map(|(v, fields)| {
                        (
                            v.clone(),
                            fields.iter().map(|f| f.substitute(bindings)).collect(),
                        )
                    })
                    .collect(),
            },
            TypeDef::Function {
                type_params,
                params,
                return_type,
                has_rest,
            } => TypeDef::Function {
                type_params: type_params
                    .iter()
                    .map(|tp| tp.substitute(bindings))
                    .collect(),
                params: params.iter().map(|p| p.substitute(bindings)).collect(),
                return_type: return_type.as_ref().map(|ty| ty.substitute(bindings)),
                has_rest: *has_rest,
            },
            TypeDef::ConstValue {
                fields,
                elements,
                type_ref_name,
            } => TypeDef::ConstValue {
                fields: fields.iter().map(|f| f.substitute(bindings)).collect(),
                elements: elements.iter().map(|e| e.substitute(bindings)).collect(),
                type_ref_name: type_ref_name.clone(),
            },
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
    /// ビルトイン型（`external_types` に含まれる名前）がソース定義型で上書きされる場合、
    /// ビルトインの `constructor` と `methods` 情報を保護する。ソース定義型が独自の
    /// constructor/methods を持つ場合はソース定義を優先する。
    pub fn merge(&mut self, other: &TypeRegistry) {
        for (name, def) in &other.types {
            if self.external_types.contains(name) {
                if let Some(existing) = self.types.get(name) {
                    let merged = merge_with_builtin_preservation(existing, def);
                    self.types.insert(name.clone(), merged);
                    continue;
                }
            }
            self.types.insert(name.clone(), def.clone());
        }
        for name in &other.external_types {
            self.external_types.insert(name.clone());
        }
    }

    /// Looks up method signatures from the object type's definition.
    ///
    /// Handles `Vec<T>` → `Array<T>` mapping so that TypeScript Array methods
    /// (push, map, filter, etc.) are available on Rust Vec types.
    /// Also handles `String` → `String` interface and generic instantiation.
    pub fn lookup_method_sigs(
        &self,
        obj_type: &RustType,
        method_name: &str,
    ) -> Option<Vec<MethodSignature>> {
        // Vec<T> → Array<T>
        if let RustType::Vec(inner) = obj_type {
            let type_def = self.instantiate("Array", &[inner.as_ref().clone()]);
            return match &type_def {
                Some(TypeDef::Struct { methods, .. }) => methods.get(method_name).cloned(),
                _ => None,
            };
        }

        self.resolve_type_def(obj_type).and_then(|def| match &def {
            TypeDef::Struct { methods, .. } => methods.get(method_name).cloned(),
            _ => None,
        })
    }

    /// Looks up a field type from the object type's definition.
    ///
    /// Handles `Vec<T>` → `Array<T>` mapping for field access (e.g., `arr.length`).
    pub fn lookup_field_type(&self, obj_type: &RustType, field_name: &str) -> Option<RustType> {
        self.resolve_type_def(obj_type).and_then(|def| match &def {
            TypeDef::Struct { fields, .. } => fields
                .iter()
                .find(|f| f.name == field_name)
                .map(|f| f.ty.clone()),
            _ => None,
        })
    }

    /// Resolves a `RustType` to its `TypeDef`, handling Vec→Array and generic instantiation.
    fn resolve_type_def(&self, obj_type: &RustType) -> Option<TypeDef> {
        match obj_type {
            RustType::Vec(inner) => self.instantiate("Array", &[inner.as_ref().clone()]),
            RustType::String => self.get("String").cloned(),
            RustType::Named { name, type_args }
                if name == "Box"
                    && type_args.len() == 1
                    && matches!(&type_args[0], RustType::DynTrait(_)) =>
            {
                if let RustType::DynTrait(trait_name) = &type_args[0] {
                    self.get(trait_name).cloned()
                } else {
                    None
                }
            }
            RustType::Named { name, type_args } => {
                if type_args.is_empty() {
                    self.get(name).cloned()
                } else {
                    self.instantiate(name, type_args)
                }
            }
            RustType::Ref(inner) => match inner.as_ref() {
                RustType::DynTrait(name) => self.get(name).cloned(),
                _ => None,
            },
            RustType::DynTrait(name) => self.get(name).cloned(),
            _ => None,
        }
    }
}

/// ビルトイン型とソース定義型をマージする。
///
/// ソース定義型のフィールド・extends を採用しつつ、ビルトインの constructor/methods/
/// call_signatures を保護する。ソース定義型が独自のものを持つ場合はソース定義を優先する。
fn merge_with_builtin_preservation(builtin: &TypeDef, source: &TypeDef) -> TypeDef {
    match (builtin, source) {
        (
            TypeDef::Struct {
                constructor: builtin_ctor,
                methods: builtin_methods,
                call_signatures: builtin_call_sigs,
                ..
            },
            TypeDef::Struct {
                type_params,
                fields,
                methods: source_methods,
                constructor: source_ctor,
                call_signatures: source_call_sigs,
                extends,
                is_interface,
            },
        ) => {
            // Constructor: use source if it has one, otherwise preserve builtin
            let constructor = if source_ctor.as_ref().is_some_and(|c| !c.is_empty()) {
                source_ctor.clone()
            } else {
                builtin_ctor.clone()
            };

            // Methods: merge builtin methods with source methods (source takes priority)
            let mut methods = builtin_methods.clone();
            for (name, sigs) in source_methods {
                methods.insert(name.clone(), sigs.clone());
            }

            // Call signatures: use source if it has any, otherwise preserve builtin
            let call_signatures = if source_call_sigs.is_empty() {
                builtin_call_sigs.clone()
            } else {
                source_call_sigs.clone()
            };

            TypeDef::Struct {
                type_params: type_params.clone(),
                fields: fields.clone(),
                methods,
                constructor,
                call_signatures,
                extends: extends.clone(),
                is_interface: *is_interface,
            }
        }
        // For non-Struct types, source takes priority
        (_, source_def) => source_def.clone(),
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

    // Pass 2a: non-Var 宣言を先に resolve する。
    // interface/type alias/class/enum を先に完全解決することで、
    // Pass 2b の Var 宣言が型注釈で callable interface を参照できる。
    let lookup_1 = reg.clone();
    for item in &module.body {
        let decl = match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => decl,
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => &export.decl,
            _ => continue,
        };
        if !matches!(decl, ast::Decl::Var(_)) {
            collection::collect_decl(&mut reg, decl, &lookup_1, synthetic);
        }
    }

    // Pass 2b: Var 宣言を resolve する。
    // Pass 2a 完了後の snapshot を lookup に使用する (Pass 1 snapshot は使わない)。
    let lookup_2 = reg.clone();
    for item in &module.body {
        let decl = match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) => decl,
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => &export.decl,
            _ => continue,
        };
        if matches!(decl, ast::Decl::Var(_)) {
            collection::collect_decl(&mut reg, decl, &lookup_2, synthetic);
        }
    }

    // Register synthetic enum types (generated during type conversion) into the TypeRegistry
    enums::register_extra_enums(&mut reg, synthetic);

    reg
}
