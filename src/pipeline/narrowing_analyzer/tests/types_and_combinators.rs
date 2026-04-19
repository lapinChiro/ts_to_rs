//! Pure data-type & combinator tests for [`super::super::NarrowingAnalyzer`].
//!
//! Covers `ResetCause::invalidates_narrow`, `NarrowEvent` variant
//! accessors, `NarrowingAnalyzer` construction, and the
//! `classifier::merge_branches` / `merge_sequential` combinators in
//! isolation from AST fixtures.

use super::*;

mod reset_cause {
    use super::*;

    #[test]
    fn invalidates_narrow_direct_assign() {
        assert!(ResetCause::DirectAssign.invalidates_narrow());
        assert!(ResetCause::NullAssign.invalidates_narrow());
        assert!(ResetCause::CompoundLogical.invalidates_narrow());
        assert!(ResetCause::ClosureReassign.invalidates_narrow());
        assert!(ResetCause::LoopBoundary.invalidates_narrow());
    }

    #[test]
    fn preserves_narrow_for_numeric_compound() {
        assert!(!ResetCause::CompoundArith.invalidates_narrow());
        assert!(!ResetCause::UpdateExpr.invalidates_narrow());
        assert!(!ResetCause::NullishAssignOnNarrow.invalidates_narrow());
    }
}

mod narrow_event_accessors {
    use super::*;
    use crate::pipeline::type_resolution::Span;

    #[test]
    fn var_name_for_narrow_variant() {
        let e = NarrowEvent::Narrow {
            var_name: "x".into(),
            scope_start: 0,
            scope_end: 10,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        };
        assert_eq!(e.var_name(), "x");
    }

    #[test]
    fn var_name_for_reset_variant() {
        let e = NarrowEvent::Reset {
            var_name: "y".into(),
            position: 42,
            cause: ResetCause::NullAssign,
        };
        assert_eq!(e.var_name(), "y");
    }

    #[test]
    fn var_name_for_closure_capture_variant() {
        let e = NarrowEvent::ClosureCapture {
            var_name: "z".into(),
            closure_span: Span { lo: 10, hi: 20 },
            outer_narrow: RustType::String,
        };
        assert_eq!(e.var_name(), "z");
    }

    #[test]
    fn as_narrow_returns_some_for_narrow_variant() {
        let e = NarrowEvent::Narrow {
            var_name: "x".into(),
            scope_start: 1,
            scope_end: 5,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        };
        let view = e.as_narrow().expect("Narrow → Some");
        assert_eq!(view.var_name, "x");
        assert_eq!(view.scope_start, 1);
        assert_eq!(view.scope_end, 5);
        assert!(matches!(view.narrowed_type, RustType::F64));
        assert!(matches!(
            view.trigger,
            NarrowTrigger::Primary(PrimaryTrigger::Truthy)
        ));
    }

    #[test]
    fn as_narrow_returns_none_for_reset_variant() {
        let e = NarrowEvent::Reset {
            var_name: "x".into(),
            position: 0,
            cause: ResetCause::DirectAssign,
        };
        assert!(e.as_narrow().is_none());
    }

    #[test]
    fn as_narrow_returns_none_for_closure_capture_variant() {
        let e = NarrowEvent::ClosureCapture {
            var_name: "x".into(),
            closure_span: Span { lo: 0, hi: 0 },
            outer_narrow: RustType::Bool,
        };
        assert!(e.as_narrow().is_none());
    }

    // --- NarrowTrigger accessor methods ---

    #[test]
    fn narrow_trigger_primary_of_primary_variant_returns_inner() {
        let t = NarrowTrigger::Primary(PrimaryTrigger::Truthy);
        assert!(matches!(t.primary(), PrimaryTrigger::Truthy));
    }

    #[test]
    fn narrow_trigger_primary_of_early_return_complement_returns_inner_trigger() {
        let t = NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::TypeofGuard("string".into()));
        assert!(matches!(t.primary(), PrimaryTrigger::TypeofGuard(s) if s == "string"));
    }

    #[test]
    fn narrow_trigger_is_early_return_complement_true_for_complement_variant() {
        let t = NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy);
        assert!(t.is_early_return_complement());
    }

    #[test]
    fn narrow_trigger_is_early_return_complement_false_for_primary_variant() {
        let t = NarrowTrigger::Primary(PrimaryTrigger::Truthy);
        assert!(!t.is_early_return_complement());
    }
}

mod analyzer_construction {
    use super::*;

    #[test]
    fn new_has_no_var_types() {
        let a = NarrowingAnalyzer::new();
        assert!(a.var_type("anything").is_none());
    }

    #[test]
    fn default_matches_new() {
        let a: NarrowingAnalyzer = NarrowingAnalyzer::default();
        assert!(a.var_type("x").is_none());
    }

    #[test]
    fn with_var_types_returns_seeded_type() {
        let mut map = std::collections::HashMap::new();
        map.insert("x".to_string(), RustType::F64);
        let a = NarrowingAnalyzer::with_var_types(map);
        assert!(matches!(a.var_type("x"), Some(RustType::F64)));
        assert!(a.var_type("y").is_none());
    }
}

mod combinators {
    use super::*;
    use crate::pipeline::narrowing_analyzer::classifier::{merge_branches, merge_sequential};

    // --- merge_branches ---

    #[test]
    fn merge_branches_none_none() {
        assert_eq!(merge_branches(None, None), None);
    }

    #[test]
    fn merge_branches_some_invalidating_left_none_right() {
        assert_eq!(
            merge_branches(Some(ResetCause::NullAssign), None),
            Some(ResetCause::NullAssign)
        );
    }

    #[test]
    fn merge_branches_none_left_some_invalidating_right() {
        assert_eq!(
            merge_branches(None, Some(ResetCause::DirectAssign)),
            Some(ResetCause::DirectAssign)
        );
    }

    #[test]
    fn merge_branches_some_preserving_left_none_right() {
        assert_eq!(
            merge_branches(Some(ResetCause::CompoundArith), None),
            Some(ResetCause::CompoundArith)
        );
    }

    #[test]
    fn merge_branches_invalidating_preferred_over_preserving() {
        // Either position → invalidating wins.
        assert_eq!(
            merge_branches(
                Some(ResetCause::CompoundArith),
                Some(ResetCause::NullAssign)
            ),
            Some(ResetCause::NullAssign)
        );
        assert_eq!(
            merge_branches(
                Some(ResetCause::NullAssign),
                Some(ResetCause::CompoundArith)
            ),
            Some(ResetCause::NullAssign)
        );
    }

    #[test]
    fn merge_branches_both_invalidating_picks_source_order() {
        // Both invalidating — left wins (documented source-order determinism).
        assert_eq!(
            merge_branches(Some(ResetCause::NullAssign), Some(ResetCause::DirectAssign),),
            Some(ResetCause::NullAssign)
        );
        assert_eq!(
            merge_branches(Some(ResetCause::DirectAssign), Some(ResetCause::NullAssign),),
            Some(ResetCause::DirectAssign)
        );
    }

    #[test]
    fn merge_branches_both_preserving_picks_source_order() {
        assert_eq!(
            merge_branches(
                Some(ResetCause::CompoundArith),
                Some(ResetCause::UpdateExpr),
            ),
            Some(ResetCause::CompoundArith)
        );
        assert_eq!(
            merge_branches(
                Some(ResetCause::UpdateExpr),
                Some(ResetCause::CompoundArith),
            ),
            Some(ResetCause::UpdateExpr)
        );
    }

    // --- merge_sequential ---

    #[test]
    fn merge_sequential_none_none() {
        assert_eq!(merge_sequential(None, None), None);
    }

    #[test]
    fn merge_sequential_some_left_none_right_keeps_left() {
        assert_eq!(
            merge_sequential(Some(ResetCause::CompoundArith), None),
            Some(ResetCause::CompoundArith)
        );
    }

    #[test]
    fn merge_sequential_none_left_some_right_keeps_right() {
        assert_eq!(
            merge_sequential(None, Some(ResetCause::UpdateExpr)),
            Some(ResetCause::UpdateExpr)
        );
    }

    #[test]
    fn merge_sequential_invalidating_left_short_circuits() {
        // Left invalidating — returns left regardless of right.
        assert_eq!(
            merge_sequential(
                Some(ResetCause::NullAssign),
                Some(ResetCause::CompoundArith)
            ),
            Some(ResetCause::NullAssign)
        );
    }

    #[test]
    fn merge_sequential_preserving_left_invalidating_right_returns_right() {
        // Left preserving, right invalidating — right wins (overrides preservation).
        assert_eq!(
            merge_sequential(
                Some(ResetCause::CompoundArith),
                Some(ResetCause::NullAssign)
            ),
            Some(ResetCause::NullAssign)
        );
    }

    #[test]
    fn merge_sequential_both_preserving_keeps_left() {
        // Neither invalidating — left wins (first cause in sequence).
        assert_eq!(
            merge_sequential(
                Some(ResetCause::CompoundArith),
                Some(ResetCause::UpdateExpr),
            ),
            Some(ResetCause::CompoundArith)
        );
    }
}
