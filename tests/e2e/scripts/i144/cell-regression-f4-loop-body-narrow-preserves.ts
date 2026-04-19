// I-144 regression lock-in F4: narrow established outside a loop must persist
// across iterations when the body does not reassign the variable. Observed in
// `tests/observations/i144/f4-loop-body-narrow.ts` g() — tsx runtime = 15.
// Guards against CFG analyzer (T3-T5) introducing spurious per-iteration reset.

function g(): number {
    let x: number | null = 5;
    if (x !== null) {
        let out = 0;
        for (let i = 0; i < 3; i++) {
            out += x;
        }
        return out;
    }
    return -1;
}

function main(): void {
    console.log(g());
}
