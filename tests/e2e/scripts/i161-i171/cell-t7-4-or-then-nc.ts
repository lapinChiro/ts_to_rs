// T7-4: `||=` followed by `??=` chain on narrowed var.

function f(): number {
    let x: number | null = null;
    x ||= 5;   // null ||= 5 → x = 5
    x ??= 99;  // x=5 narrowed; ??= is no-op
    return x;
}

function main(): void {
    console.log(f()); // 5
}
