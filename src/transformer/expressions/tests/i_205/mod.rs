//! I-205 (Class member access dispatch with getter/setter methodology framework) lock-in tests.
//!
//! ## File split rationale (Iteration v10 third-review、`design-integrity.md` cohesion)
//!
//! Pre-split: 単一 `tests/i_205.rs` (~1078 lines、CLAUDE.md "0 errors / 0 warnings" の file-line
//! threshold 1000 行 violation)。post-split: Read context tests (T5 cells + B7 traversal helper +
//! Read defensive arms) と Write context tests (T6 cells + Write context regression + INV-2 +
//! Fallback equivalence + Write defensive arms) を architectural concern (Read vs Write) で
//! 分離、`design-integrity.md` "Higher-level design consistency" + `pipeline-integrity.md`
//! "single responsibility per module" 観点で cohesion 改善。
//!
//! ## Submodules
//!
//! - [`read`]: T5 Read context dispatch tests (cells 1-10) + B7 traversal helper tests (cycle /
//!   direct hit / single-step / multi-step inheritance) + Read defensive dispatch arm tests
//!   (matrix cell 化なし、Iteration v10 second-review C1 coverage 補完)
//! - [`write`]: T6 Write context dispatch tests (cells 11-19) + Write context regression test
//!   (T5 fix lock-in、Read dispatch leak 防止) + INV-2 E1 Read/Write symmetry + T6 Fallback
//!   equivalence (T5 `for_write=true` skip path との token-level identical lock-in) + Static
//!   field lookup miss + Write defensive dispatch arm tests (matrix cell 化なし、Iteration v10
//!   second-review C1 coverage 補完)
//!
//! 全 test は `Transformer::for_module(...).convert_expr(&swc_expr)` で direct invoke、IR Expr
//! を `assert_eq!` で token-level verify。Tier 2 path は `convert_expr` の Err を
//! `downcast::<UnsupportedSyntaxError>` で kind verify。

mod read;
mod write;
