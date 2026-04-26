// I-177-D Matrix Cell #10: Primary(NullCheck NotEqEqNull) + closure-reassign + body read
//
// 案 C target: TypeResolver narrowed_type for `x !== null` narrow inside
// cons-span returns Some(T) even when closure-reassign exists. T7-3 と同型 pattern
// だが body read-only で silent change risk 回避。

function f(x: number | null): number {
    let last: number = -1;
    const reset = () => { x = null; };
    if (x !== null) {
        last = x;              // Primary narrow read: x: number (case-C target)
    }
    reset();                   // mutates outer x; runtime: x = null
    return last;
}

function main(): void {
    console.log(f(5));     // 5
    console.log(f(null));  // -1
}
