//! struct/impl/trait 生成ロジック。

use anyhow::Result;

use super::helpers::{make_impl, make_struct, make_trait_ref, strip_method_visibility};
use super::inheritance::rewrite_super_constructor;
use super::ClassInfo;
use crate::ir::{Item, Method, TraitRef, Visibility};

/// Generates IR items for a class, optionally with a parent class for inheritance.
///
/// When `parent` is `Some`, the class is treated as a child:
/// - Parent fields are copied to the child struct
/// - A trait is generated from the parent's methods
/// - Both parent and child get trait impls
pub fn generate_items_for_class(info: &ClassInfo, parent: Option<&ClassInfo>) -> Result<Vec<Item>> {
    match parent {
        None => generate_standalone_class(info),
        Some(parent_info) => generate_child_class(info, parent_info),
    }
}

/// Generates items for a parent class that is extended by another class.
///
/// Produces: struct + trait + impl (constructor) + impl trait for struct
pub fn generate_parent_class_items(info: &ClassInfo) -> Result<Vec<Item>> {
    let trait_name = format!("{}Trait", info.name);
    let mut items = vec![make_struct(
        &info.name,
        &info.vis,
        info.type_params.clone(),
        info.fields.clone(),
    )];

    // Trait with method signatures (no bodies)
    let trait_methods: Vec<Method> = info
        .methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            body: None,
            ..m.clone()
        })
        .collect();
    items.push(Item::Trait {
        vis: info.vis.clone(),
        name: trait_name.clone(),
        type_params: info.type_params.clone(),
        supertraits: vec![],
        methods: trait_methods,
        associated_types: vec![],
    });

    // impl (constructor + static consts)
    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        vec![],
    ));

    // impl Trait for Struct (method bodies)
    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        Some(make_trait_ref(&trait_name, &info.type_params)),
        vec![],
        None,
        strip_method_visibility(&info.methods),
    ));

    Ok(items)
}

/// Generates IR items for an abstract class as a trait.
///
/// Abstract methods become trait method signatures (no body).
/// Concrete methods become default implementations.
pub fn generate_abstract_class_items(info: &ClassInfo) -> Result<Vec<Item>> {
    let methods: Vec<Method> = info
        .methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            has_self: true,
            has_mut_self: m.has_mut_self,
            ..m.clone()
        })
        .collect();

    Ok(vec![Item::Trait {
        vis: info.vis.clone(),
        name: info.name.clone(),
        type_params: info.type_params.clone(),
        supertraits: vec![],
        methods,
        associated_types: vec![],
    }])
}

/// Generates IR items for a concrete class that extends an abstract class.
///
/// Produces: struct + impl (constructor) + impl Trait for Struct (methods)
pub fn generate_child_of_abstract(
    info: &ClassInfo,
    abstract_trait_name: &str,
) -> Result<Vec<Item>> {
    let mut items = vec![make_struct(
        &info.name,
        &info.vis,
        info.type_params.clone(),
        info.fields.clone(),
    )];

    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        vec![],
    ));
    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        Some(make_trait_ref(abstract_trait_name, &info.type_params)),
        vec![],
        None,
        strip_method_visibility(&info.methods),
    ));

    Ok(items)
}

/// Generates IR items for a standalone class (no inheritance).
fn generate_standalone_class(info: &ClassInfo) -> Result<Vec<Item>> {
    let mut items = vec![make_struct(
        &info.name,
        &info.vis,
        info.type_params.clone(),
        info.fields.clone(),
    )];

    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        info.methods.clone(),
    ));

    Ok(items)
}

/// Generates IR items for a child class that extends a parent.
///
/// Produces: struct (with parent fields) + impl (constructor + own methods) + impl trait
fn generate_child_class(info: &ClassInfo, parent: &ClassInfo) -> Result<Vec<Item>> {
    let trait_name = format!("{}Trait", parent.name);

    // Struct with parent + child fields
    let mut fields = parent.fields.clone();
    fields.extend(info.fields.clone());
    let mut items = vec![make_struct(
        &info.name,
        &info.vis,
        info.type_params.clone(),
        fields,
    )];

    // impl (rewritten constructor + own methods + static consts)
    let ctor = info
        .constructor
        .as_ref()
        .map(|c| rewrite_super_constructor(c, parent))
        .transpose()?;
    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        None,
        info.static_consts.clone(),
        ctor.as_ref(),
        info.methods.clone(),
    ));

    // impl ParentTrait for Child (parent method bodies)
    // trait 型引数は child の extends 型引数（例: extends Parent<string> → <String>）
    let parent_trait_ref = TraitRef {
        name: trait_name,
        type_args: info.parent_type_args.clone(),
    };
    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        Some(parent_trait_ref),
        vec![],
        None,
        strip_method_visibility(&parent.methods),
    ));

    Ok(items)
}

/// Generates IR items for a child class that also implements interfaces.
///
/// Produces: struct (parent+child fields) + impl Child + impl ParentTrait for Child
/// + impl Interface for Child (per interface).
pub fn generate_child_class_with_implements(
    info: &ClassInfo,
    parent: Option<&ClassInfo>,
    iface_methods: &std::collections::HashMap<String, Vec<String>>,
) -> Result<Vec<Item>> {
    // Start with child class items (struct + impl + impl ParentTrait)
    let mut items = generate_items_for_class(info, parent)?;

    // Add interface trait impls by moving matching methods from impl Child to impl Interface
    let mut claimed_methods: std::collections::HashSet<String> = std::collections::HashSet::new();

    for iface_ref in &info.implements {
        if let Some(method_names) = iface_methods.get(&iface_ref.name) {
            let trait_methods = strip_method_visibility(
                &info
                    .methods
                    .iter()
                    .filter(|m| method_names.contains(&m.name))
                    .cloned()
                    .collect::<Vec<_>>(),
            );

            if !trait_methods.is_empty() {
                for m in &trait_methods {
                    claimed_methods.insert(m.name.clone());
                }
                items.extend(make_impl(
                    &info.name,
                    info.type_params.clone(),
                    Some(iface_ref.clone()),
                    vec![],
                    None,
                    trait_methods,
                ));
            }
        }
    }

    // Remove claimed methods from the own impl block
    if !claimed_methods.is_empty() {
        for item in &mut items {
            if let Item::Impl {
                struct_name,
                for_trait: None,
                methods,
                ..
            } = item
            {
                if struct_name == &info.name {
                    methods.retain(|m| !claimed_methods.contains(&m.name));
                }
            }
        }
    }

    Ok(items)
}

/// Generates IR items for a class that implements one or more interfaces.
///
/// Methods matching interface method names go into `impl Trait for Struct`.
/// Remaining methods (including constructor) go into `impl Struct`.
pub fn generate_class_with_implements(
    info: &ClassInfo,
    iface_methods: &std::collections::HashMap<String, Vec<String>>,
) -> Result<Vec<Item>> {
    let mut items = vec![make_struct(
        &info.name,
        &info.vis,
        info.type_params.clone(),
        info.fields.clone(),
    )];

    let mut claimed_methods: std::collections::HashSet<String> = std::collections::HashSet::new();

    for iface_ref in &info.implements {
        if let Some(method_names) = iface_methods.get(&iface_ref.name) {
            let trait_methods = strip_method_visibility(
                &info
                    .methods
                    .iter()
                    .filter(|m| method_names.contains(&m.name))
                    .cloned()
                    .collect::<Vec<_>>(),
            );

            if !trait_methods.is_empty() {
                for m in &trait_methods {
                    claimed_methods.insert(m.name.clone());
                }
                items.extend(make_impl(
                    &info.name,
                    info.type_params.clone(),
                    Some(iface_ref.clone()),
                    vec![],
                    None,
                    trait_methods,
                ));
            }
        }
    }

    let unclaimed: Vec<_> = info
        .methods
        .iter()
        .filter(|m| !claimed_methods.contains(&m.name))
        .cloned()
        .collect();
    items.extend(make_impl(
        &info.name,
        info.type_params.clone(),
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        unclaimed,
    ));

    Ok(items)
}
