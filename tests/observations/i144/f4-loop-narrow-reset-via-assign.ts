// F4 corner: narrow established before loop, then loop body reassigns — narrow reset?
function f(): string {
    let x: number | null = 5;
    if (x === null) return "null";
    // x: number
    let out = "";
    for (let i = 0; i < 2; i++) {
        out += ":" + x;
        // Reassign after use (but TS should detect reassignment as narrow reset at loop head)
        if (i === 0) x = null;
    }
    // x: number | null after loop
    return out + "|end";
}
console.log(f());
