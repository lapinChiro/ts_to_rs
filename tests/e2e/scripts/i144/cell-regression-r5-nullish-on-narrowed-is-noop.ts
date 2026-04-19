// I-144 regression lock-in R5: `??=` applied to an already-narrowed (non-null)
// variable is a runtime no-op. Observed in
// `tests/observations/i144/r5-nullish-on-narrowed.ts` f() — tsx runtime = 5.
// PRD Sub-matrix 5 RC3 narrow-alive maps to E9 "predicate elide" — emitter
// must not surface UnsupportedSyntaxError here (narrow alive, no reset ahead).

function f(): number {
    let x: number | null = 5;
    if (x !== null) {
        x ??= 10;
        return x;
    }
    return -1;
}

function main(): void {
    console.log(f());
}
