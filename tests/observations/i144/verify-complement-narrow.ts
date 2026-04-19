// F3 verification: does TS narrow complement in else branch?
function f(x: string | number): string {
    if (typeof x === "string") {
        return "str:" + x.toUpperCase();
    } else {
        // x: number (complement of string)
        return "num:" + x.toFixed(2);
    }
}
console.log(f("hello"));
console.log(f(3.14));

// Multi-condition narrow with &&
function g(x: number | null, y: string | null): string {
    if (x !== null && y !== null) {
        return x + ":" + y;
    }
    return "incomplete";
}
console.log(g(5, "x"));
console.log(g(null, "x"));
console.log(g(5, null));

// Negation narrow
function h(x: string | null): string {
    if (!(x === null)) {
        return "defined:" + x;
    }
    return "null";
}
console.log(h("a"));
console.log(h(null));

// Early throw narrow
function i(x: number | null): number {
    if (x === null) throw new Error("null");
    return x + 1;  // x: number (narrowed via throw)
}
console.log(i(5));
