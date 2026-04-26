// I-177-D Matrix Cell #14: Primary(Truthy) + closure-reassign + body read
//
// 案 C target: TypeResolver narrowed_type for bare truthy `if (x)` narrow inside
// cons-span returns Some(T) even when closure-reassign exists.

function f(x: string | null): string {
    let last: string = "init";
    const reset = () => { x = null; };
    if (x) {
        last = x;              // Primary narrow read: x: string (case-C target)
    }
    reset();                   // mutates outer x; runtime: x = null
    return last;
}

function main(): void {
    console.log(f("hello"));  // "hello"
    console.log(f(null));     // "init"
    console.log(f(""));       // "init" (empty string falsy)
}
