// I-153 matrix cell #7: nested switch inside outer switch case, with bare break
// in the inner case body's `if.cons`. Tests the critical inner/outer emission
// interaction: inner switch wraps in its own `'__ts_switch`, outer sees the
// inner `Stmt::LabeledBlock` and correctly skips descent (inner-owned).

function f(x: number, y: number): number {
    let count = 0;
    for (let i = 0; i < 2; i++) {
        switch (x) {
            case 1:
                switch (y) {
                    case 10:
                        if (i === 0) break;  // inner switch break
                        count = count + 100;
                        break;
                    default:
                        count = count + 1;
                        break;
                }
                count = count + 50;
                break;
            default:
                count = count + 1000;
                break;
        }
        count = count + 10;
    }
    return count;
}

function main(): void {
    // f(1,10): i=0 inner case10 break → +50 +10 = 60
    //          i=1 inner case10 +100 +50 +10 = 160 → 220
    console.log(f(1, 10));
    // f(1,99): 2× (default +1 +50 +10) = 122
    console.log(f(1, 99));
    // f(9,*): 2× (outer default +1000 +10) = 2020
    console.log(f(9, 99));
}
