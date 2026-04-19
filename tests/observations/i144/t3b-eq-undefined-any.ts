// T3b on any: x === undefined
function f(x: any): string {
    if (x === undefined) return "undef";
    return "defined:" + String(x);
}
console.log(f(5));
console.log(f(undefined));
console.log(f(null));
console.log(f({a: 1}));
