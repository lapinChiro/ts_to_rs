// T3b: x === undefined on Option<T> (i.e., `number | undefined`)
// Does TS narrow to T on the else branch? What about `x !== undefined`?
function f(x: number | undefined): number {
    if (x === undefined) return -1;
    // inside: x narrowed to `number`
    return x + 1;
}
function g(x: number | undefined): number {
    if (x !== undefined) {
        // inside: x narrowed to `number`
        return x * 2;
    }
    return 0;
}
console.log(f(5));
console.log(f(undefined));
console.log(g(5));
console.log(g(undefined));
