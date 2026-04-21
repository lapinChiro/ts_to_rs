//! `Item::Use` rendering: single name, multiple names, both visibilities.

use super::*;
use crate::ir::{Item, Visibility};

#[test]
fn test_generate_use_single() {
    let item = Item::Use {
        vis: Visibility::Private,
        path: "crate::bar".to_string(),
        names: vec!["Foo".to_string()],
    };
    assert_eq!(generate(&[item]), "use crate::bar::Foo;");
}

#[test]
fn test_generate_use_multiple() {
    let item = Item::Use {
        vis: Visibility::Private,
        path: "crate::bar".to_string(),
        names: vec!["A".to_string(), "B".to_string()],
    };
    assert_eq!(generate(&[item]), "use crate::bar::{A, B};");
}

#[test]
fn test_generate_pub_use_single() {
    let item = Item::Use {
        vis: Visibility::Public,
        path: "crate::bar".to_string(),
        names: vec!["Foo".to_string()],
    };
    assert_eq!(generate(&[item]), "pub use crate::bar::Foo;");
}

#[test]
fn test_generate_pub_use_multiple() {
    let item = Item::Use {
        vis: Visibility::Public,
        path: "crate::baz".to_string(),
        names: vec!["A".to_string(), "B".to_string()],
    };
    assert_eq!(generate(&[item]), "pub use crate::baz::{A, B};");
}
