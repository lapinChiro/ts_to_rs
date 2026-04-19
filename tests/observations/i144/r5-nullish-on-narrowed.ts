// R5: ??= on already-narrowed (already non-null) variable
function f() {
    let x: number | null = 5;
    if (x !== null) {
        // x: number
        x ??= 10;  // no-op at runtime since x is non-null
        return x;
    }
    return -1;
}
// More tricky: assign narrow then ??=
function g() {
    let x: number | null = null;
    x = 7;  // assignment narrows x to number
    x ??= 99;  // TS: does narrow persist?
    return x;
}
console.log(f());
console.log(g());
