// T7-2 regression: `||=` on narrowed F64. Narrow preserved (x truthy, no assign).

function f(): number {
    let x: number | null = 5;
    if (x !== null) {
        x ||= 99;  // x=5 truthy → no assign
        return x;
    }
    return -1;
}

function main(): void {
    console.log(f()); // 5
}
