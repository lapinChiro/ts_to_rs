// I-153 matrix cell #5: block stmt `{ ... }` in case body + nested bare break.
// Requires T0 (A-fix: ast::Stmt::Block support) + I-153 walker.

function f(x: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 5; i++) {
        switch (x) {
            case 1: {
                if (cond) break;
                count = count + 100;
                break;
            }
            default: {
                count = count + 1;
                break;
            }
        }
        count = count + 10;
    }
    return count;
}

function main(): void {
    console.log(f(1, true));   // 50
    console.log(f(1, false));  // 550
    console.log(f(2, false));  // 55
}
