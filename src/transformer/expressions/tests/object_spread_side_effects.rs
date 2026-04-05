use super::*;
use crate::ir::Stmt as IrStmt;

// --- I-298: spread override side-effect preservation ---

#[test]
fn test_spread_override_pure_expr_no_block_registered() {
    // { x: 42, ...base } registered — pure literal overridden → no Block wrapper needed
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "Point", &["x", "y"]);
    let f = TctxFixture::from_source_with_reg("const p: Point = { x: 42, ...base };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Pure literal → no Block wrapping, just StructInit
    assert!(matches!(result, Expr::StructInit { .. }));
}

#[test]
fn test_spread_override_fn_call_emits_side_effect_registered() {
    // { x: getX(), ...base } registered — fn call overridden → Block preserving side effect
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "Point", &["x", "y"]);
    register_fn(&mut reg, "getX", RustType::F64);
    let f = TctxFixture::from_source_with_reg("const p: Point = { x: getX(), ...base };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Should be Block { let _ = getX(); TailExpr(StructInit) }
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2);
            // First: side-effect evaluation
            assert!(
                matches!(&stmts[0], IrStmt::Let { init: Some(Expr::FnCall { name, .. }), .. } if name == "getX"),
                "expected Let {{ init: FnCall getX }}, got {:?}",
                stmts[0]
            );
            // Last: tail expr returning the struct
            assert!(
                matches!(&stmts[1], IrStmt::TailExpr(Expr::StructInit { .. })),
                "expected TailExpr(StructInit), got {:?}",
                stmts[1]
            );
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}

#[test]
fn test_spread_override_multiple_side_effects_preserves_source_order_registered() {
    // { x: f(), y: g(), ...base } registered — both overridden by spread
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "S", &["x", "y"]);
    register_fn(&mut reg, "f", RustType::F64);
    register_fn(&mut reg, "g", RustType::F64);
    let f = TctxFixture::from_source_with_reg("const s: S = { x: f(), y: g(), ...base };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 3, "2 side effects + 1 tail expr");
            // Source order: f() first, g() second
            assert!(
                matches!(&stmts[0], IrStmt::Let { init: Some(Expr::FnCall { name, .. }), .. } if name == "f"),
                "first side effect should be f(), got {:?}",
                stmts[0]
            );
            assert!(
                matches!(&stmts[1], IrStmt::Let { init: Some(Expr::FnCall { name, .. }), .. } if name == "g"),
                "second side effect should be g(), got {:?}",
                stmts[1]
            );
            assert!(matches!(
                &stmts[2],
                IrStmt::TailExpr(Expr::StructInit { .. })
            ));
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}

#[test]
fn test_spread_override_fn_call_between_multiple_spreads_registered() {
    // { ...a, x: f(), ...b } registered — f() is overridden by b (rightmost spread)
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "Point", &["x", "y"]);
    register_fn(&mut reg, "f", RustType::F64);
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...a, x: f(), ...b };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2, "1 side effect + 1 tail expr");
            // f() is overridden by spread b, should be preserved as side effect
            assert!(
                matches!(&stmts[0], IrStmt::Let { init: Some(Expr::FnCall { name, .. }), .. } if name == "f"),
                "expected let _ = f(), got {:?}",
                stmts[0]
            );
            assert!(matches!(
                &stmts[1],
                IrStmt::TailExpr(Expr::StructInit { .. })
            ));
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}

#[test]
fn test_spread_middle_side_effect_before_only_registered() {
    // { x: f(), ...base, y: g() } registered — f() overridden, g() is used
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "S", &["x", "y"]);
    register_fn(&mut reg, "f", RustType::F64);
    register_fn(&mut reg, "g", RustType::F64);
    let f = TctxFixture::from_source_with_reg("const s: S = { x: f(), ...base, y: g() };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2, "1 side effect + 1 tail expr");
            assert!(
                matches!(&stmts[0], IrStmt::Let { init: Some(Expr::FnCall { name, .. }), .. } if name == "f"),
            );
            // g() should be in the struct init fields (it's used, not overridden)
            if let IrStmt::TailExpr(Expr::StructInit { fields, .. }) = &stmts[1] {
                let y_field = fields.iter().find(|(k, _)| k == "y").expect("y field");
                assert!(
                    matches!(&y_field.1, Expr::FnCall { name, .. } if name == "g"),
                    "y should use g() as value, got {:?}",
                    y_field.1
                );
            } else {
                panic!("expected TailExpr(StructInit), got {:?}", stmts[1]);
            }
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}

#[test]
fn test_spread_override_fn_call_emits_side_effect_unregistered() {
    // { x: f(), ...base } unregistered — fn call before spread → Block preserving side effect
    let mut reg = TypeRegistry::new();
    register_fn(&mut reg, "f", RustType::F64);
    let f = TctxFixture::from_source_with_reg("const p: Point = { x: f(), ...base };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2);
            assert!(
                matches!(&stmts[0], IrStmt::Let { init: Some(Expr::FnCall { name, .. }), .. } if name == "f"),
            );
            assert!(matches!(
                &stmts[1],
                IrStmt::TailExpr(Expr::StructInit { base: Some(_), .. })
            ));
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}

#[test]
fn test_spread_override_pure_expr_no_block_unregistered() {
    // { x: 42, ...base } unregistered — pure literal → no Block
    let f = TctxFixture::from_source("const p: Point = { x: 42, ...base };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(
        matches!(result, Expr::StructInit { .. }),
        "pure literals should not cause Block wrapping"
    );
}

// --- I-365: spread source single-evaluation ---

#[test]
fn test_spread_source_fn_call_bound_to_temp_var_registered() {
    // { ...getBase() } registered — fn call spread source must be evaluated once
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "Point", &["x", "y"]);
    register_fn(
        &mut reg,
        "getBase",
        RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        },
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...getBase() };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Should bind spread source to temp var, then use it for field access
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2, "1 temp binding + 1 tail expr");
            // Temp var binding: let __spread_obj_0 = getBase();
            match &stmts[0] {
                IrStmt::Let {
                    name,
                    init: Some(Expr::FnCall { name: fn_name, .. }),
                    ..
                } => {
                    assert_eq!(name, "__spread_obj_0");
                    assert_eq!(fn_name, "getBase");
                }
                other => panic!(
                    "expected Let {{ __spread_obj_0 = getBase() }}, got {:?}",
                    other
                ),
            }
            // Struct init uses temp var for field access
            if let IrStmt::TailExpr(Expr::StructInit { fields, .. }) = &stmts[1] {
                for (field_name, field_expr) in fields {
                    match field_expr {
                        Expr::FieldAccess { object, field } => {
                            assert_eq!(
                                **object,
                                Expr::Ident("__spread_obj_0".to_string()),
                                "field {field_name} should access __spread_obj_0, not the original fn call"
                            );
                            assert_eq!(field, field_name);
                        }
                        other => panic!("expected FieldAccess for {field_name}, got {:?}", other),
                    }
                }
            } else {
                panic!("expected TailExpr(StructInit), got {:?}", stmts[1]);
            }
        }
        _ => panic!(
            "expected Block for non-pure spread source, got {:?}",
            result
        ),
    }
}

#[test]
fn test_spread_source_ident_no_temp_var_registered() {
    // { ...base } registered — ident is pure, no temp var needed
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "Point", &["x", "y"]);
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...base };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // No Block wrapping needed — ident is trivially pure
    assert!(
        matches!(result, Expr::StructInit { .. }),
        "pure ident spread should not cause Block wrapping, got {:?}",
        result
    );
}

#[test]
fn test_spread_source_fn_call_with_overridden_explicit_source_order() {
    // { x: f(), ...getBase() } registered — both side effects in source order
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "S", &["x", "y"]);
    register_fn(&mut reg, "f", RustType::F64);
    register_fn(
        &mut reg,
        "getBase",
        RustType::Named {
            name: "S".to_string(),
            type_args: vec![],
        },
    );
    let f = TctxFixture::from_source_with_reg("const s: S = { x: f(), ...getBase() };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Source order: f() at idx 0 (overridden), getBase() at idx 1 (spread binding)
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(
                stmts.len(),
                3,
                "let _ = f(); let __spread_obj_0 = getBase(); TailExpr"
            );
            // idx 0: overridden explicit f()
            assert!(
                matches!(&stmts[0], IrStmt::Let { name, init: Some(Expr::FnCall { name: fn_name, .. }), .. }
                    if name == "_" && fn_name == "f"),
                "stmts[0] should be let _ = f(), got {:?}",
                stmts[0]
            );
            // idx 1: spread binding getBase()
            assert!(
                matches!(&stmts[1], IrStmt::Let { name, init: Some(Expr::FnCall { name: fn_name, .. }), .. }
                    if name == "__spread_obj_0" && fn_name == "getBase"),
                "stmts[1] should be let __spread_obj_0 = getBase(), got {:?}",
                stmts[1]
            );
            // idx 2: struct init
            assert!(matches!(
                &stmts[2],
                IrStmt::TailExpr(Expr::StructInit { .. })
            ));
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}

#[test]
fn test_spread_source_multiple_fn_calls_get_separate_temp_vars() {
    // { ...getA(), x: 1, ...getB() } registered — two non-pure spreads → two temp vars
    let mut reg = TypeRegistry::new();
    register_f64_struct(&mut reg, "S", &["x", "y"]);
    let s_type = RustType::Named {
        name: "S".to_string(),
        type_args: vec![],
    };
    for name in ["getA", "getB"] {
        register_fn(&mut reg, name, s_type.clone());
    }
    let f = TctxFixture::from_source_with_reg("const s: S = { ...getA(), x: 1, ...getB() };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // rightmost-wins: getB (idx 2) wins all fields; x:1 is pure → no side effect
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(
                stmts.len(),
                3,
                "__spread_obj_0 = getA(); __spread_obj_1 = getB(); TailExpr"
            );
            // Source order: getA at idx 0, getB at idx 2
            assert!(
                matches!(&stmts[0], IrStmt::Let { name, init: Some(Expr::FnCall { name: fn_name, .. }), .. }
                    if name == "__spread_obj_0" && fn_name == "getA"),
                "stmts[0] should be __spread_obj_0 = getA(), got {:?}",
                stmts[0]
            );
            assert!(
                matches!(&stmts[1], IrStmt::Let { name, init: Some(Expr::FnCall { name: fn_name, .. }), .. }
                    if name == "__spread_obj_1" && fn_name == "getB"),
                "stmts[1] should be __spread_obj_1 = getB(), got {:?}",
                stmts[1]
            );
            // Struct init: rightmost spread (__spread_obj_1) wins all fields
            if let IrStmt::TailExpr(Expr::StructInit { fields, .. }) = &stmts[2] {
                for (_, field_expr) in fields {
                    if let Expr::FieldAccess { object, .. } = field_expr {
                        assert_eq!(
                            **object,
                            Expr::Ident("__spread_obj_1".to_string()),
                            "rightmost spread (__spread_obj_1) should win"
                        );
                    }
                }
            } else {
                panic!("expected TailExpr(StructInit), got {:?}", stmts[2]);
            }
        }
        _ => panic!("expected Block, got {:?}", result),
    }
}
