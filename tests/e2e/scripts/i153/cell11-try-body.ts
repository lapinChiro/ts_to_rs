// I-153 matrix cell #11: bare break inside try body within switch case.
// Interaction: TryBodyRewrite emits `if _try_break { break; }` sibling; our walker
// rewrites that bare break to `break '__ts_switch`.

function f(x: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 3; i++) {
        switch (x) {
            case 1:
                try {
                    if (cond) break;
                    count = count + 100;
                } catch (e) {
                    count = count + 999;
                }
                count = count + 10;  // post-try
                break;
            default:
                count = count + 1;
                break;
        }
        count = count + 1000;  // post-switch
    }
    return count;
}

function main(): void {
    // f(1, true): try break → skip 100 + 10 (post-try), +1000 × 3 = 3000
    console.log(f(1, true));
    // f(1, false): try body completes → +100 + 10 + 1000 = 1110 × 3 = 3330
    console.log(f(1, false));
    // f(2, false): default +1 + 1000 × 3 = 3003
    console.log(f(2, false));
}
