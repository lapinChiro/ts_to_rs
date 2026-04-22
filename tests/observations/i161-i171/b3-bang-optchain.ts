// B3: `!x?.v` (OptChain LHS).
// `x?.v` evaluates to undefined when x is null/undefined.
function f(x: { v: string | null } | null): string {
    if (!x?.v) return "none";
    return x.v;  // narrow: x is non-null, x.v is non-null after !x?.v falsy branch
}
console.log(f(null));
console.log(f({ v: null }));
console.log(f({ v: "" }));
console.log(f({ v: "hi" }));
