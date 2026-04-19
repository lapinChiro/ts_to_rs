// I-153 matrix cell #6 (P-4): bare break inside `if.alt` (else body).

function f(x: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 3; i++) {
        switch (x) {
            case 1:
                if (cond) {
                    count = count + 1000;
                } else {
                    break;
                }
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
    console.log(f(1, true));    // 3 × (1000 + 100 + 10) = 3330
    console.log(f(1, false));   // 3 × 10 = 30 (else break → no 100, no 1000)
    console.log(f(2, false));   // 3 × (1 + 10) = 33
}
