use super::*;
use crate::ir::{EnumVariant, Item, RustType, StructField, Visibility};

// ===== I-382: user 型 import 生成テスト =====

fn make_user_struct(name: &str) -> Item {
    Item::Struct {
        vis: Visibility::Public,
        name: name.to_string(),
        type_params: vec![],
        fields: vec![],
    }
}

fn make_synthetic_enum_with_user_ref(synth_name: &str, variants: &[(&str, &str)]) -> Item {
    Item::Enum {
        vis: Visibility::Public,
        name: synth_name.to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: variants
            .iter()
            .map(|(vname, ty_name)| EnumVariant {
                name: vname.to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: ty_name.to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            })
            .collect(),
    }
}

#[test]
fn test_shared_synthetic_with_user_type_generates_import() {
    // shared 配置された synthetic enum が user 定義型を variant に持つ場合、
    // shared_types.rs に user 型の import が生成される。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("My", "MyType"), ("S", "String")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // shared_module が存在し、user 型 import を含む
    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "shared_types.rs should import MyType: {content}"
    );
}

#[test]
fn test_inline_synthetic_same_file_no_import() {
    // inline 配置された synthetic が同一ファイルの user 型を参照 → import 不要
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![TestFile::new(
        "types.rs",
        "pub struct MyType {}",
        vec![make_user_struct("MyType"), fn_returning("foo", "MyUnion")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("My", "MyType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // inline 配置、import なし
    assert!(placement.inline.contains_key(Path::new("types.rs")));
    assert!(
        placement.shared_imports.is_empty(),
        "same-file user type should not generate import"
    );
}

#[test]
fn test_inline_synthetic_different_file_generates_import() {
    // inline 配置された synthetic が別ファイルの user 型を参照 → import 生成
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "handler.rs",
            "fn handle() -> MyUnion { todo!() }",
            vec![fn_returning("handle", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("My", "MyType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // inline in handler.rs, import for MyType
    assert!(placement.inline.contains_key(Path::new("handler.rs")));
    let imports = placement
        .shared_imports
        .get(Path::new("handler.rs"))
        .expect("handler.rs should have user type import");
    assert!(
        imports.iter().any(|i| i.contains("MyType")),
        "handler.rs should import MyType: {imports:?}"
    );
}

#[test]
fn test_shared_synthetic_multiple_user_types_grouped() {
    // shared synthetic が複数の user 型を同一モジュールから参照 → グループ化
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct TypeA {} pub struct TypeB {}",
            vec![make_user_struct("TypeA"), make_user_struct("TypeB")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("A", "TypeA"), ("B", "TypeB")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::{TypeA, TypeB};"),
        "should group imports from same module: {content}"
    );
}

#[test]
fn test_transitive_shared_user_type_import() {
    // shared A → shared B → user MyType の推移依存
    // shared_types.rs に MyType の import が含まれる
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let synth_b = make_synthetic_enum_with_user_ref("SynthB", &[("My", "MyType")]);
    let synth_a = Item::Struct {
        vis: Visibility::Public,
        name: "SynthA".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "b".to_string(),
            ty: RustType::Named {
                name: "SynthB".to_string(),
                type_args: vec![],
            },
        }],
    };

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> SynthA { todo!() }",
            vec![fn_returning("foo", "SynthA")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SynthA { todo!() }",
            vec![fn_returning("bar", "SynthA")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![synth_a, synth_b];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "transitive user type should be imported in shared module: {content}"
    );
}

#[test]
fn test_user_type_not_in_transpiled_files_no_import() {
    // user 型が transpile 対象外 (user_type_def_map にない) → import 生成されない
    // (compile error は Tier 2 であり、空 stub より上位の fidelity)
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    // ExternalOnly は files に定義されていない
    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("E", "ExternalOnly")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    // ExternalOnly は files にない → import 文は生成されない
    // (enum 本文に ExternalOnly は残るが use 文は生成しない)
    assert!(
        !content.contains("use") || !content.contains("ExternalOnly;"),
        "type not in transpiled files should not generate a use import: {content}"
    );
    // より正確: "use" で始まる行に ExternalOnly が含まれないこと
    for line in content.lines() {
        if line.trim_start().starts_with("use ") {
            assert!(
                !line.contains("ExternalOnly"),
                "use statement should not reference ExternalOnly: {line}"
            );
        }
    }
}

#[test]
fn test_inline_synthetic_different_modules_grouped_separately() {
    // inline synthetic が異なるモジュールの user 型を参照 → モジュールごとに別 import 文
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct TypeA {}",
            vec![make_user_struct("TypeA")],
        ),
        TestFile::new(
            "models.rs",
            "pub struct TypeB {}",
            vec![make_user_struct("TypeB")],
        ),
        TestFile::new(
            "handler.rs",
            "fn handle() -> MyUnion { todo!() }",
            vec![fn_returning("handle", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("A", "TypeA"), ("B", "TypeB")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // inline in handler.rs
    assert!(placement.inline.contains_key(Path::new("handler.rs")));
    let imports = placement
        .shared_imports
        .get(Path::new("handler.rs"))
        .expect("handler.rs should have user type imports");
    // 2 つの異なるモジュールからの import
    assert!(
        imports.iter().any(|i| i.contains("crate::types::TypeA")),
        "should import TypeA from types module: {imports:?}"
    );
    assert!(
        imports.iter().any(|i| i.contains("crate::models::TypeB")),
        "should import TypeB from models module: {imports:?}"
    );
}

#[test]
fn test_shared_and_inline_both_ref_same_user_type() {
    // shared synthetic と inline synthetic が同じ user 型を参照
    // shared_types.rs に import、inline 側は shared_types 経由でアクセス可能
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let synth_shared = make_synthetic_enum_with_user_ref("SharedUnion", &[("My", "MyType")]);
    let synth_inline = make_synthetic_enum_with_user_ref("InlineUnion", &[("My", "MyType")]);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> SharedUnion { todo!() }",
            vec![fn_returning("foo", "SharedUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SharedUnion { todo!() } fn baz() -> InlineUnion { todo!() }",
            vec![
                fn_returning("bar", "SharedUnion"),
                fn_returning("baz", "InlineUnion"),
            ],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![synth_shared, synth_inline];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // SharedUnion は shared (a.rs + b.rs から参照)
    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "shared module should import MyType: {content}"
    );

    // InlineUnion は b.rs に inline (b.rs のみから参照)
    assert!(placement.inline.contains_key(Path::new("b.rs")));

    // b.rs の imports に MyType が含まれるか確認
    // InlineUnion は b.rs に inline され、MyType を参照 → types.rs は別ファイル → import 必要
    let b_imports = placement
        .shared_imports
        .get(Path::new("b.rs"))
        .expect("b.rs should have imports");
    assert!(
        b_imports.iter().any(|i| i.contains("MyType")),
        "b.rs should import MyType for inline synthetic: {b_imports:?}"
    );
}

#[test]
fn test_inline_synthetic_transitive_to_shared_no_scope_leak() {
    // scope isolation 検証: inline A → shared B → user MyType
    // A のファイルには MyType の import は不要 (B は shared_types.rs にあり、
    // shared_types.rs 側で MyType を import する)
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let synth_b = make_synthetic_enum_with_user_ref("SynthB", &[("My", "MyType")]);
    let synth_a = Item::Struct {
        vis: Visibility::Public,
        name: "SynthA".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "b".to_string(),
            ty: RustType::Named {
                name: "SynthB".to_string(),
                type_args: vec![],
            },
        }],
    };

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        // a.rs references SynthA (inline), SynthB referenced only by SynthA (shared)
        TestFile::new(
            "a.rs",
            "fn foo() -> SynthA { todo!() }",
            vec![fn_returning("foo", "SynthA")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SynthB { todo!() }",
            vec![fn_returning("bar", "SynthB")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![synth_a, synth_b];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // SynthB は 2 ファイル (a.rs via SynthA + b.rs 直接) → shared
    // SynthA は a.rs + synthetic ref → shared
    // shared_types.rs に MyType の import がある
    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "shared module should import MyType: {content}"
    );

    // a.rs は MyType を直接参照しない → user type import は不要
    // (shared_types の use は別だが、user type import はなし)
    if let Some(a_imports) = placement.shared_imports.get(Path::new("a.rs")) {
        for import in a_imports {
            assert!(
                !import.contains("crate::types::MyType"),
                "a.rs should NOT import MyType directly (it's in shared_types): {import}"
            );
        }
    }
}

#[test]
fn test_user_type_import_from_index_module() {
    // index.rs → crate::<parent> モジュールパスの検証
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "services/index.rs",
            "pub struct ServiceType {}",
            vec![make_user_struct("ServiceType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("S", "ServiceType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    // index.rs → parent module "services" (not "services::index")
    assert!(
        content.contains("use crate::services::ServiceType;"),
        "index.rs should map to parent module: {content}"
    );
    assert!(
        !content.contains("crate::services::index"),
        "should NOT contain index in module path: {content}"
    );
}

#[test]
fn test_user_type_import_with_hyphenated_module() {
    // ハイフン付きパス → アンダースコア変換の検証
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "my-utils.rs",
            "pub struct UtilType {}",
            vec![make_user_struct("UtilType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("U", "UtilType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    // my-utils.rs → crate::my_utils (hyphen → underscore)
    assert!(
        content.contains("use crate::my_utils::UtilType;"),
        "hyphen should be replaced with underscore: {content}"
    );
    assert!(
        !content.contains("my-utils"),
        "should NOT contain hyphen in module path: {content}"
    );
}
