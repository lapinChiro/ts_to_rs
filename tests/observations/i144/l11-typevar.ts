// L11: TypeVar narrow — narrow on generic parameter
function f<T>(x: T | null): T {
    if (x === null) throw new Error("null");
    // x: T (narrowed, generic)
    return x;
}
function g<T extends { v: number }>(x: T | null): number {
    if (x === null) return -1;
    // x: T with constraint
    return x.v;
}
// Typeof narrow on generic?
function h<T>(x: T | string): string {
    if (typeof x === "string") {
        // x: string (narrowed, despite generic union)
        return "s:" + x.toUpperCase();
    }
    return "other";
}
console.log(f("hello"));
console.log(g({ v: 42 }));
console.log(h<number>(5));
console.log(h<number>("hi"));
