// I-153 matrix cell #14: bare break inside IfLet body in case body.
// Note: `IfLet` in IR is generated from destructuring patterns like
// `if (obj) { ... }` when obj: T | null, which compile-narrows to `if let Some(obj) = ...`.
// This test exercises walker's descent into IfLet.then_body.

function f(x: number, maybe: number | null): number {
    let count = 0;
    for (let i = 0; i < 3; i++) {
        switch (x) {
            case 1:
                if (maybe !== null) {
                    if (i >= 1) break;  // nested inside if (maybe -> narrowed to number)
                    count = count + maybe;
                }
                count = count + 50;
                break;
            default:
                count = count + 1;
                break;
        }
    }
    return count;
}

function main(): void {
    // f(1, 7): i=0 +7 +50 = 57; i=1 break → skip +50; i=2 break → skip +50
    //          total: 57
    console.log(f(1, 7));
    // f(1, null): 3× (maybe=null → skip if-body, +50 each) = 150
    console.log(f(1, null));
    // f(2, null): 3× default +1 = 3
    console.log(f(2, null));
}
