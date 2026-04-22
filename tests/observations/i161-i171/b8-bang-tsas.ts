// B8: `!(x as any)`.
// `as` is a type assertion only; has no runtime effect. Truthy of inner value applies.
function f(x: number | null): string {
    if (!(x as any)) return "falsy";
    return "truthy";
}
console.log(f(null));
console.log(f(0));
console.log(f(5));
