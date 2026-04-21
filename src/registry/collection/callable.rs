//! Callable-interface classification.
//!
//! Distinguishes `interface F { (x): string }` (traitifiable callable)
//! from regular interfaces (fields, methods, constructor) and from
//! type-alias-derived callables (`type F = (x) => string`, handled
//! elsewhere as `Box<dyn Fn>`). The [`CallableInterfaceKind`] enum is
//! consumed by the transformer to select between struct + impl and
//! `Box<dyn Fn>` call dispatch.

use crate::registry::{MethodSignature, TypeDef};

/// Callable interface の分類結果。
///
/// `TypeDef::Struct` の `call_signatures` と他メンバー (fields, methods, constructor)
/// の有無に基づいて、callable interface かどうかを判定する。
#[derive(Debug, Clone, PartialEq)]
pub enum CallableInterfaceKind {
    /// call signature がない、または他メンバーが存在する (通常の struct/interface)
    NonCallable,
    /// call signature が 1 つのみで、他メンバーなし
    SingleOverload(MethodSignature),
    /// call signature が 2 つ以上で、他メンバーなし
    MultiOverload(Vec<MethodSignature>),
}

/// TypeDef を callable interface として分類する。
///
/// Callable interface = `interface` 宣言由来で、call signature のみを持つ型。
/// `type T = { (x): string }` (type alias 由来、`is_interface: false`) は NonCallable。
/// fields, methods, constructor がある場合も `NonCallable` を返す。
/// `TypeDef::Struct` 以外も `NonCallable`。
pub fn classify_callable_interface(def: &TypeDef) -> CallableInterfaceKind {
    let TypeDef::Struct {
        call_signatures,
        fields,
        methods,
        constructor,
        is_interface,
        ..
    } = def
    else {
        return CallableInterfaceKind::NonCallable;
    };

    // type alias 由来 (is_interface: false) は callable interface として扱わない。
    // type alias の callable type は type_aliases.rs で Box<dyn Fn> として処理される。
    // trait 化は interfaces.rs の interface 宣言のみが対象。
    if !is_interface {
        return CallableInterfaceKind::NonCallable;
    }

    // call signature がなければ非 callable
    if call_signatures.is_empty() {
        return CallableInterfaceKind::NonCallable;
    }

    // 他メンバーがあれば非 callable (call signature + fields/methods の混合 interface)
    if !fields.is_empty() || !methods.is_empty() || constructor.is_some() {
        return CallableInterfaceKind::NonCallable;
    }

    match call_signatures.len() {
        1 => CallableInterfaceKind::SingleOverload(call_signatures[0].clone()),
        _ => CallableInterfaceKind::MultiOverload(call_signatures.clone()),
    }
}

#[cfg(test)]
mod classify_callable_interface_tests {
    use std::collections::HashMap;

    use super::*;
    use crate::ir::RustType;
    use crate::registry::{FieldDef, ParamDef};

    fn make_call_sig(param_count: usize) -> MethodSignature {
        MethodSignature {
            params: (0..param_count)
                .map(|i| ParamDef {
                    name: format!("p{i}"),
                    ty: RustType::String,
                    optional: false,
                    has_default: false,
                })
                .collect(),
            return_type: Some(RustType::String),
            ..Default::default()
        }
    }

    #[test]
    fn non_callable_no_call_signatures() {
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![FieldDef {
                name: "name".to_string(),
                ty: RustType::String,
                optional: false,
            }],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }

    #[test]
    fn single_overload_one_call_signature() {
        let sig = make_call_sig(1);
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![sig.clone()],
            extends: vec![],
            is_interface: true,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::SingleOverload(sig)
        );
    }

    #[test]
    fn multi_overload_two_call_signatures() {
        let sig1 = make_call_sig(1);
        let sig2 = make_call_sig(2);
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![sig1.clone(), sig2.clone()],
            extends: vec![],
            is_interface: true,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::MultiOverload(vec![sig1, sig2])
        );
    }

    #[test]
    fn non_callable_call_sig_with_fields() {
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![FieldDef {
                name: "name".to_string(),
                ty: RustType::String,
                optional: false,
            }],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![make_call_sig(1)],
            extends: vec![],
            is_interface: true,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }

    #[test]
    fn non_callable_call_sig_with_methods() {
        let mut methods = HashMap::new();
        methods.insert("doSomething".to_string(), vec![make_call_sig(0)]);
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods,
            constructor: None,
            call_signatures: vec![make_call_sig(1)],
            extends: vec![],
            is_interface: true,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }

    #[test]
    fn non_callable_call_sig_with_constructor() {
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: Some(vec![make_call_sig(1)]),
            call_signatures: vec![make_call_sig(1)],
            extends: vec![],
            is_interface: true,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }

    #[test]
    fn non_callable_enum() {
        let def = TypeDef::Enum {
            type_params: vec![],
            variants: vec!["A".to_string()],
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }

    #[test]
    fn non_callable_function() {
        let def = TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: None,
            has_rest: false,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }

    #[test]
    fn non_callable_type_alias_with_call_sig() {
        // type T = { (x: string): string } — is_interface: false
        // type alias 由来の callable type は NonCallable (Box<dyn Fn> path を維持)
        let def = TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![make_call_sig(1)],
            extends: vec![],
            is_interface: false,
        };
        assert_eq!(
            classify_callable_interface(&def),
            CallableInterfaceKind::NonCallable
        );
    }
}
