// T7-1 regression lock-in: `&&=` on narrowed F64 (R4 from I-144, re-hosted here).
// Previously removed when deferred to this PRD. Narrow preserved after &&=.

function f(): number {
    let x: number | null = 5;
    if (x !== null) {
        x &&= 3;
        return x;
    }
    return -1;
}

function main(): void {
    console.log(f()); // 3 (narrow preserved, truthy x &&= 3 → x = 3)
}
