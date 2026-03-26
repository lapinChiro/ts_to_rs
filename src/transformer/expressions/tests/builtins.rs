use super::*;

#[test]
fn test_process_env_access_converts_to_env_var() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // process.env.HOME → std::env::var("HOME").unwrap()
    let expr = parse_expr("process.env.HOME;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::env::var".to_string(),
                args: vec![Expr::StringLit("HOME".to_string())],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_read_file_sync_converts_to_read_to_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.readFileSync("a.txt", "utf8") → std::fs::read_to_string(&"a.txt").unwrap()
    let expr = parse_expr(r#"fs.readFileSync("a.txt", "utf8");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::fs::read_to_string".to_string(),
                args: vec![Expr::Ref(Box::new(Expr::StringLit("a.txt".to_string())))],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_write_file_sync_converts_to_fs_write() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.writeFileSync("a.txt", data) → std::fs::write(&"a.txt", &data).unwrap()
    let expr = parse_expr(r#"fs.writeFileSync("a.txt", data);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::fs::write".to_string(),
                args: vec![
                    Expr::Ref(Box::new(Expr::StringLit("a.txt".to_string()))),
                    Expr::Ref(Box::new(Expr::Ident("data".to_string()))),
                ],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_exists_sync_converts_to_path_exists() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.existsSync("a.txt") → std::path::Path::new("a.txt").exists()
    let expr = parse_expr(r#"fs.existsSync("a.txt");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::path::Path::new".to_string(),
                args: vec![Expr::Ref(Box::new(Expr::StringLit("a.txt".to_string())))],
            }),
            method: "exists".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_read_file_sync_stdin_converts_to_stdin_read() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.readFileSync("/dev/stdin", "utf8") → std::io::read_to_string(std::io::stdin()).unwrap()
    let expr = parse_expr(r#"fs.readFileSync("/dev/stdin", "utf8");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::io::read_to_string".to_string(),
                args: vec![Expr::FnCall {
                    name: "std::io::stdin".to_string(),
                    args: vec![],
                }],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_read_file_sync_fd0_converts_to_stdin_read() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.readFileSync(0, "utf8") → same as /dev/stdin
    let expr = parse_expr(r#"fs.readFileSync(0, "utf8");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::io::read_to_string".to_string(),
                args: vec![Expr::FnCall {
                    name: "std::io::stdin".to_string(),
                    args: vec![],
                }],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}
