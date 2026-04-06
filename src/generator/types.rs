//! Type generation: converts IR types into Rust type syntax.

use crate::ir::RustType;

/// Generates the Rust type syntax for a [`RustType`].
pub fn generate_type(ty: &RustType) -> String {
    match ty {
        RustType::Unit => "()".to_string(),
        RustType::String => "String".to_string(),
        RustType::F64 => "f64".to_string(),
        RustType::Bool => "bool".to_string(),
        RustType::Option(inner) => format!("Option<{}>", generate_type(inner)),
        RustType::Vec(inner) => format!("Vec<{}>", generate_type(inner)),
        RustType::Result { ok, err } => {
            format!("Result<{}, {}>", generate_type(ok), generate_type(err))
        }
        RustType::Fn {
            params,
            return_type,
        } => {
            let params_str = params
                .iter()
                .map(generate_type)
                .collect::<Vec<_>>()
                .join(", ");
            if matches!(return_type.as_ref(), RustType::Unit) {
                format!("Box<dyn Fn({params_str})>")
            } else {
                format!(
                    "Box<dyn Fn({params_str}) -> {}>",
                    generate_type(return_type)
                )
            }
        }
        RustType::Tuple(elems) => {
            let inner = elems
                .iter()
                .map(generate_type)
                .collect::<Vec<_>>()
                .join(", ");
            if elems.len() == 1 {
                format!("({inner},)")
            } else {
                format!("({inner})")
            }
        }
        RustType::Any => "serde_json::Value".to_string(),
        RustType::Never => "std::convert::Infallible".to_string(),
        RustType::Named { name, type_args } => {
            if type_args.is_empty() {
                name.clone()
            } else {
                let args = type_args
                    .iter()
                    .map(generate_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}<{args}>")
            }
        }
        RustType::Ref(inner) => format!("&{}", generate_type(inner)),
        RustType::DynTrait(name) => format!("dyn {name}"),
        RustType::QSelf {
            qself,
            trait_ref,
            item,
        } => {
            let trait_str = if trait_ref.type_args.is_empty() {
                trait_ref.name.clone()
            } else {
                let args = trait_ref
                    .type_args
                    .iter()
                    .map(generate_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{args}>", trait_ref.name)
            };
            format!("<{} as {trait_str}>::{item}", generate_type(qself))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::RustType;

    #[test]
    fn test_generate_type_string() {
        assert_eq!(generate_type(&RustType::String), "String");
    }

    #[test]
    fn test_generate_type_f64() {
        assert_eq!(generate_type(&RustType::F64), "f64");
    }

    #[test]
    fn test_generate_type_bool() {
        assert_eq!(generate_type(&RustType::Bool), "bool");
    }

    #[test]
    fn test_generate_type_option() {
        let ty = RustType::Option(Box::new(RustType::String));
        assert_eq!(generate_type(&ty), "Option<String>");
    }

    #[test]
    fn test_generate_type_vec() {
        let ty = RustType::Vec(Box::new(RustType::F64));
        assert_eq!(generate_type(&ty), "Vec<f64>");
    }

    #[test]
    fn test_generate_type_nested() {
        let ty = RustType::Option(Box::new(RustType::Vec(Box::new(RustType::Bool))));
        assert_eq!(generate_type(&ty), "Option<Vec<bool>>");
    }

    #[test]
    fn test_generate_type_fn_single_param() {
        let ty = RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::F64),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn(f64) -> f64>");
    }

    #[test]
    fn test_generate_type_fn_multiple_params() {
        let ty = RustType::Fn {
            params: vec![RustType::String, RustType::F64],
            return_type: Box::new(RustType::Bool),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn(String, f64) -> bool>");
    }

    #[test]
    fn test_generate_type_fn_no_params() {
        let ty = RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::F64),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn() -> f64>");
    }

    #[test]
    fn test_generate_type_result() {
        let ty = RustType::Result {
            ok: Box::new(RustType::String),
            err: Box::new(RustType::String),
        };
        assert_eq!(generate_type(&ty), "Result<String, String>");
    }

    #[test]
    fn test_generate_type_result_unit_ok() {
        let ty = RustType::Result {
            ok: Box::new(RustType::Named {
                name: "()".to_string(),
                type_args: vec![],
            }),
            err: Box::new(RustType::String),
        };
        assert_eq!(generate_type(&ty), "Result<(), String>");
    }

    #[test]
    fn test_generate_type_result_never_ok_uses_infallible() {
        let ty = RustType::Result {
            ok: Box::new(RustType::Never),
            err: Box::new(RustType::String),
        };
        assert_eq!(
            generate_type(&ty),
            "Result<std::convert::Infallible, String>"
        );
    }

    #[test]
    fn test_generate_type_named_no_type_args() {
        let ty = RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        };
        assert_eq!(generate_type(&ty), "Point");
    }

    #[test]
    fn test_generate_type_named_with_single_type_arg() {
        let ty = RustType::Named {
            name: "Box".to_string(),
            type_args: vec![RustType::String],
        };
        assert_eq!(generate_type(&ty), "Box<String>");
    }

    #[test]
    fn test_generate_type_named_with_multiple_type_args() {
        let ty = RustType::Named {
            name: "HashMap".to_string(),
            type_args: vec![RustType::String, RustType::F64],
        };
        assert_eq!(generate_type(&ty), "HashMap<String, f64>");
    }

    #[test]
    fn test_generate_type_unit() {
        assert_eq!(generate_type(&RustType::Unit), "()");
    }

    #[test]
    fn test_generate_type_any() {
        assert_eq!(generate_type(&RustType::Any), "serde_json::Value");
    }

    #[test]
    fn test_generate_type_never() {
        assert_eq!(generate_type(&RustType::Never), "std::convert::Infallible");
    }

    #[test]
    fn test_generate_type_fn_void_return_single_param() {
        let ty = RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::Unit),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn(f64)>");
    }

    #[test]
    fn test_generate_type_fn_void_return_no_params() {
        let ty = RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Unit),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn()>");
    }

    #[test]
    fn test_generate_type_qself_simple() {
        let ty = RustType::QSelf {
            qself: Box::new(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
            trait_ref: crate::ir::TraitRef {
                name: "Promise".to_string(),
                type_args: vec![],
            },
            item: "Output".to_string(),
        };
        assert_eq!(generate_type(&ty), "<T as Promise>::Output");
    }

    #[test]
    fn test_generate_type_qself_with_trait_args() {
        let ty = RustType::QSelf {
            qself: Box::new(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
            trait_ref: crate::ir::TraitRef {
                name: "Container".to_string(),
                type_args: vec![RustType::String],
            },
            item: "Item".to_string(),
        };
        assert_eq!(generate_type(&ty), "<T as Container<String>>::Item");
    }

    #[test]
    fn test_generate_type_qself_with_complex_qself() {
        // <Vec<Foo> as Promise>::Output
        let ty = RustType::QSelf {
            qself: Box::new(RustType::Vec(Box::new(RustType::Named {
                name: "Foo".to_string(),
                type_args: vec![],
            }))),
            trait_ref: crate::ir::TraitRef {
                name: "Promise".to_string(),
                type_args: vec![],
            },
            item: "Output".to_string(),
        };
        assert_eq!(generate_type(&ty), "<Vec<Foo> as Promise>::Output");
    }
}
