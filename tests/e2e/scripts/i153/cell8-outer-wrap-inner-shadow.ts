// I-153 matrix cell #8: BOTH outer and inner switch have nested bare breaks.
// Both emit `'__ts_switch:` labeled block. Rust lexical shadowing ensures
// each `break '__ts_switch` targets its own enclosing block (innermost wins).

function f(x: number, y: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 2; i++) {
        switch (x) {
            case 1:
                // outer case body has a nested bare break (if.cons)
                if (cond) break;
                switch (y) {
                    case 10:
                        // inner case body has its own nested bare break (if.cons)
                        if (i === 0) break;
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
    // f(1,10,true): 2× (outer switch break → +10) = 20
    console.log(f(1, 10, true));
    // f(1,10,false): i=0: inner case10 break → outer +50 +10 = 60; i=1: inner +100 +50 +10 = 160 → 220
    console.log(f(1, 10, false));
    // f(9,99,false): 2× (default +1000 +10) = 2020
    console.log(f(9, 99, false));
}
