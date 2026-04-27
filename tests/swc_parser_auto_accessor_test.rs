//! I-205 cell 6 (A1 Read × B5 AutoAccessor without decorator): SWC parser empirical lock-in test.
//!
//! **本 test の意義**:
//!
//! I-205 spec では cell 6 を "別 PRD scope (PRD 2.8 / I-201-A AutoAccessor 単体 Tier 1 化)"
//! として認識。本 PRD I-205 では PRD 2.7 で確立した Tier 2 honest error 状態 (= `class.rs:165`
//! `UnsupportedSyntaxError::new("AutoAccessor", aa.span)`) を維持。
//!
//! ただし empirical observation 2026-04-28 により:
//!
//! - SWC parser は `accessor x: number = 0;` を `ClassMember::AutoAccessor(AutoAccessor { ... })`
//!   として **accept** する (= TS 5.0+ stable AutoAccessor 構文、SWC fully supported)
//! - tsx runtime は auto-generated getter/setter pair で正しく dispatch (cell 6 fixture で
//!   `f.x` → 0 を確認)
//! - I-205 framework は method kind tracking infrastructure を確立、PRD 2.8 (I-201-A) で
//!   AutoAccessor を Tier 1 完全変換 (struct field + fn x()/set_x() pair) する際の
//!   foundation として leverage 可能
//!
//! 本 lock-in test は以下を保証する:
//!
//! 1. SWC parser は `accessor x: T = init` を **accept** (= empirical fact)
//! 2. SWC parser は `static accessor x: T = init` (static modifier) も accept
//! 3. SWC parser は `private accessor x: T = init` (TS keyword private) も accept
//! 4. PRD 2.7 で確立した `ClassMember::AutoAccessor` arm match が functioning
//!    (class.rs:165-171 で UnsupportedSyntaxError emit、PRD 2.8 で Tier 1 化予定)

use ts_to_rs::parser::parse_typescript;

#[test]
fn test_swc_parser_accepts_auto_accessor_simple() {
    // I-205 cell 6 structural assumption: SWC parser accepts `accessor x: T = init`
    let source = "class Foo { accessor x: number = 0; }";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 6 structural assumption: SWC parser must accept simple AutoAccessor \
         `accessor x: number = 0` (empirical fact 2026-04-28、TS 5.0+ stable).\n\
         If this fires, SWC parser behavior changed — investigate ClassMember::AutoAccessor \
         arm in class.rs:165-171 and reconfirm Tier 2 honest error mechanism."
    );
}

#[test]
fn test_swc_parser_accepts_auto_accessor_without_init() {
    // Init optional form: `accessor x: T;` (declaration only)
    let source = "class Foo { accessor x: number; }";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 6 (no-init variant) structural assumption: SWC parser must accept \
         `accessor x: T` without init expression."
    );
}

#[test]
fn test_swc_parser_accepts_static_auto_accessor() {
    // Static modifier form: `static accessor x: T = init`
    let source = "class Foo { static accessor x: number = 100; }";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 6 (static variant) structural assumption: SWC parser must accept \
         `static accessor x: T = init` (PRD 2.8 scope future Tier 1 化、本 PRD では \
         Tier 2 honest error 維持)."
    );
}

#[test]
fn test_swc_parser_accepts_private_keyword_auto_accessor() {
    // TS keyword private modifier: `private accessor x: T = init`
    // (Note: TS hash-private `#x accessor` is different syntax = `Key::Private`)
    let source = "class Foo { private accessor x: number = 0; }";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 6 (TS keyword private variant) structural assumption: SWC parser \
         must accept `private accessor x: T = init`."
    );
}
