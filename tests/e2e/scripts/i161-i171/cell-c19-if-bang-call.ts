// I-171 Layer 2 Cell C-19 (Call/Cond/Await/New in if-cond): `if (!g(...)) { ... }`.
// Ideal: Layer 1 feed-through with tmp bind on Call result.

function g(n: number): number | null {
    return n > 0 ? n : null;
}

function f(x: number): string {
    if (!g(x)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(0));   // falsy (g(0)=null)
    console.log(f(-1));  // falsy (g(-1)=null)
    console.log(f(5));   // truthy (g(5)=5)
}
