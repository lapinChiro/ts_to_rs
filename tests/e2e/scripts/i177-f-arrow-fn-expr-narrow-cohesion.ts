// I-177-F: resolve_arrow_expr block_end traversal cohesion
//
// Empirical lock-in for arrow form post-narrow scope EarlyReturnComplement
// event push. Pre-fix `current_block_end` was None for arrow bodies (because
// resolve_arrow_expr iterated `block.stmts` directly instead of calling
// visit_block_stmt), so `detect_early_return_narrowing` skipped its work in
// arrow context. Post-fix: visit_block_stmt sets current_block_end → narrow
// events populate → canonical helper resolves narrowed type → variant wrap
// correctly emitted.
//
// fn-expression form (`const f = function (x: ...) {...}`) is verified at
// unit test level (`test_collect_leaves_typeof_narrow_post_if_return_fn_expr`)
// because the Transformer does not currently lift fn-expressions assigned to
// `const` to standalone Rust functions (separate Transformer concern).

const fArrow = (x: string | number): number => {
    if (typeof x === "number") return x * 2;
    return x.length;
};

function main(): void {
    console.log(fArrow(10));
    console.log(fArrow("hello"));
}
