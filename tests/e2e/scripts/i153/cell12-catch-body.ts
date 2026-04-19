// I-153 matrix cell #12: bare break inside catch body within switch case.
// IR: catch body is emitted as `Stmt::IfLet.then_body`; walker descends and
// rewrites the bare break to `break '__ts_switch`.

function f(x: number, fail: boolean): number {
    let count = 0;
    for (let i = 0; i < 3; i++) {
        switch (x) {
            case 1:
                try {
                    if (fail) throw new Error("e");
                    count = count + 10;
                } catch (e) {
                    if (i >= 1) break;   // catch body bare break → switch break
                    count = count + 100;
                }
                count = count + 1000;  // post-try
                break;
            default:
                count = count + 1;
                break;
        }
    }
    return count;
}

function main(): void {
    // f(1,true): i=0 catch +100, post-try +1000 = 1100
    //           i=1 catch break → skip post-try
    //           i=2 catch break → skip post-try
    console.log(f(1, true));
    // f(1,false): 3× (try +10, post-try +1000) = 3030
    console.log(f(1, false));
    // f(2,false): 3× default +1 = 3
    console.log(f(2, false));
}
