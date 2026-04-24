// I-171 T5 deep-deep-fix lock-in (Spec gap discovered at /check_job deep
// deep review, 2026-04-24): symmetric extension of cell-c5b for the
// `=== null` early-return pattern.
//
// `if (x === null) return; else <non-exit>; return x;` (Option<T> return
// type) requires post-if narrow to be materialised AND TypeResolver to
// agree so that `return x;` re-wraps in `Some(x)` to match the function
// return type.
//
// Pre-fix path: TypeResolver had no narrow event for this case
// (`alt.is_some` blocked the push); emission used bare `if let Some(x) =
// x { ... } else { return; }` which scopes the narrow inside the if-let
// block, leaving post-if `x: Option<T>` so `return x` matched the return
// type by accident.
//
// After Deep-Deep-Fix-1 (visitors.rs broad push) the TypeResolver
// records narrow `T` post-if, triggering Some-wrap on `return x;` — but
// the if-let emission did not materialise the IR-level narrow, leaving
// `x: Option<T>` and breaking the Some-wrap (Option<Option<T>>).
//
// Deep-Deep-Deep-Fix-1 adds a 4th branch to
// `try_generate_narrowing_match` that emits a Let-wrap match for this
// case (analog of `OptionTruthyShape::EarlyReturnFromExitWithElse` on
// the if-let path), so both TypeResolver narrow and IR shadow agree
// and the Some-wrap coerce produces correct `Some(x): Option<T>`.

function f(x: number | null): number {
    if (x === null) {
        return -1;
    } else {
        console.log("ne");
    }
    return x + 1;  // x: number narrow → arithmetic
}

function h(x: number | null): number | null {
    if (x === null) {
        return -1;
    } else {
        console.log("ne");
    }
    return x;  // x: number narrow + return type number|null → Some-wrap
}

function main(): void {
    console.log(f(null), f(5));
    console.log(h(null), h(5));
}
