// T4b: if truthy on `any` — what narrow does TS apply?
// Expected: tsc assigns `any` inside then-branch (no narrow in type system);
// runtime: truthy check passes for non-falsy values.
function f(x: any): any {
    if (x) {
        // inside: static type `any`
        return x.v ?? "no-v";
    }
    return "falsy";
}
console.log(f({ v: "ok" }));
console.log(f(0));
console.log(f(""));
console.log(f(null));
console.log(f(undefined));
