// T7-5: `&&=` on narrowed synthetic union with string RHS.

function f(init: number | string | null): string {
    let x: number | string | null = init;
    if (x !== null) {
        x &&= "result";  // Type-compatible RHS within union narrow
        return typeof x === "string" ? x : `n:${x}`;
    }
    return "null";
}

function main(): void {
    console.log(f(5));     // "result" (5 truthy → assign "result")
    console.log(f("hi")); // "result"
    console.log(f(0));     // "n:0"   (0 falsy → no assign)
    console.log(f(""));    // ""      (empty string falsy → no assign)
}
