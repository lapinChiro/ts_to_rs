// I-153 matrix cell #3: bare `break` inside `if.cons` within switch case body,
// inside an enclosing loop. TSX: switch-break → next iteration (count += 10).
// Rust (pre-fix): bare `break` targets outer `for` loop → silent divergence.
// Post-fix: bare `break` rewritten to `break '__ts_switch` → identical semantics.

function f(x: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 5; i++) {
        switch (x) {
            case 1:
                if (cond) break;
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
    console.log(f(1, true));   // expect 50
    console.log(f(1, false));  // expect 550
    console.log(f(2, false));  // expect 55
}
