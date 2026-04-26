// I-177-E: TypeResolver synthetic fork inheritance gap fix
//
// Empirical lock-in for the synthetic fork bug where `fork_dedup_state` previously
// inherited dedup signatures but started with empty `types`, causing
// `compute_complement_type` to silently fail for synthetic union types whose
// signature was already in the inherited dedup state. This dropped post-narrow
// `EarlyReturnComplement` events, manifesting as "cannot determine return variant"
// hard errors and silent type widening at narrow-stale read sites.
//
// Pre-fix behavior: post-if `x.length` (in else-complement scope) treated `x` as
//   the union F64OrString instead of String → compile error or wrong method call.
// Post-fix behavior: narrow event correctly populated → `x: string` inside the
//   else-complement region → `x.length` resolves to String::len.

function f(x: string | number): number {
    if (typeof x === "number") {
        return x * 2;
    }
    return x.length;
}

function main(): void {
    console.log(f(10));
    console.log(f("hello"));
}
