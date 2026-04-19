// R4: &&= on already-narrowed Option<number>
// Does &&= preserve narrow or reset? How does tsc behave?
function f() {
    let x: number | null = 5;
    if (x !== null) {
        // x: number
        x &&= 3;
        // x &&= y  is equivalent to  if (x) x = y
        // After: x: number (assigning number) — narrow preserved
        return x;
    }
    return -1;
}
function g() {
    let x: number | null = null;
    x ??= 10;
    // x: number (??= narrows because right side is non-null)
    x &&= 5;
    return x;
}
console.log(f());
console.log(g());
