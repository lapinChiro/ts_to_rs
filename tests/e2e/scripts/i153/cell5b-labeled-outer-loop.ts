// I-153 matrix cell #5: bare break in if.cons inside switch case body,
// with OUTER loop being user-labeled (O-3). Pre-fix: bare break targets
// the USER-labeled outer loop silently. Post-fix: `break '__ts_switch`
// targets the switch correctly.

function f(x: number, cond: boolean): number {
    let count = 0;
    outer: for (let i = 0; i < 3; i++) {
        switch (x) {
            case 1:
                if (cond) break;   // user intent: break switch (not outer loop)
                count = count + 100;
                break;
            default:
                count = count + 1;
                break;
        }
        count = count + 10;
    }
    return count;
}

function main(): void {
    // f(1,true): 3 iter × (switch break → +10) = 30
    console.log(f(1, true));
    // f(1,false): 3 × (+100+10) = 330
    console.log(f(1, false));
    // f(2,false): 3 × (+1+10) = 33
    console.log(f(2, false));
}
