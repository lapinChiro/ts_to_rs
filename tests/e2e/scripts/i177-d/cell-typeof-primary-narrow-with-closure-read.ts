// I-177-D Matrix Cell #2: Primary(TypeofGuard) + closure-reassign + body read
//
// 案 C target: TypeResolver narrowed_type query inside cons-span returns Some(T)
// even when closure-reassign event exists in the same fn body. IR shadow form
// (`if let Some(x) = x { body }` style + typeof variant pattern) and TypeResolver
// narrow agree → cons-span body works without coerce_default.
//
// Body is read-only (no mutation) to avoid silent semantic change risk.
// Closure call happens AFTER cons-span, so cons-span runtime sees narrow-valid x.

function f(x: string | number | null): string {
    let last: string = "init";
    const reset = () => { x = null; };
    if (typeof x === "string") {
        last = x;            // Primary narrow read: x: string (case-C target)
    }
    reset();                 // mutates outer x; runtime: x = null
    return last;             // Returns "init" (init=5 path) or "str-value" (str path)
}

function main(): void {
    console.log(f("hello"));   // "hello" (typeof === "string" branch entered, last = "hello")
    console.log(f(5));         // "init"  (typeof !== "string", branch skipped)
    console.log(f(null));      // "init"  (typeof !== "string", branch skipped)
}
