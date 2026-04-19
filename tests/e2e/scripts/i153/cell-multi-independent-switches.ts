// I-153 Review-insight G-R1: multiple independent switches in the same function.
// Each emits `'__ts_switch:` labeled block at sibling scope. Rust allows label
// redefinition in sibling scopes; verify no runtime divergence.

function f(x: number, y: number, cond: boolean): number {
    let count = 0;
    for (let i = 0; i < 2; i++) {
        switch (x) {
            case 1:
                if (cond) break;
                count = count + 100;
                break;
            default:
                count = count + 1;
                break;
        }
        // Second switch in the same fn, sibling scope to the first.
        switch (y) {
            case 10:
                if (cond) break;
                count = count + 1000;
                break;
            default:
                count = count + 10000;
                break;
        }
    }
    return count;
}

function main(): void {
    // f(1, 10, true): 2× (both break → 0) = 0
    console.log(f(1, 10, true));
    // f(1, 10, false): 2× (first +100, second +1000) = 2200
    console.log(f(1, 10, false));
    // f(9, 99, true): 2× (first default +1, second default +10000) = 20002
    console.log(f(9, 99, true));
}
