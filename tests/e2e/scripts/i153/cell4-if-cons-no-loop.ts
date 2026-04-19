// I-153 matrix cell #4: nested `break` in switch inside function body (no outer loop).

function f(x: number, cond: boolean): number {
    let result = 0;
    switch (x) {
        case 1:
            if (cond) break;
            result = 100;
            break;
        default:
            result = 2;
            break;
    }
    result = result + 1;  // executed after switch break
    return result;
}

function main(): void {
    console.log(f(1, true));   // break → 0+1 = 1
    console.log(f(1, false));  // 100+1 = 101
    console.log(f(2, false));  // 2+1 = 3
}
