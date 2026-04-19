// T3b on Union: number | string | undefined
function f(x: number | string | undefined): string {
    if (x === undefined) return "undef";
    // narrow to number | string
    return typeof x + ":" + String(x);
}
console.log(f(5));
console.log(f("ab"));
console.log(f(undefined));
