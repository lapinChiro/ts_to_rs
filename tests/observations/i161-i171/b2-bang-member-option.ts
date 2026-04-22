// B2: `!u.v` (Member LHS) + narrow propagation after if.
function f(u: {v: string | null}): string {
    if (!u.v) return "none";
    return u.v;  // narrowed to string
}
console.log(f({ v: null }));
console.log(f({ v: "" }));
console.log(f({ v: "hi" }));
