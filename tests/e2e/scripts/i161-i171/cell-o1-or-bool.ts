// I-161 Cell O-1 (Bool `||=`): `x ||= y` on pure Bool.
// Ideal: `if !x { x = y; }`.

function f(init: boolean, rhs: boolean): boolean {
    let x: boolean = init;
    x ||= rhs;
    return x;
}

function main(): void {
    console.log(f(true, false));  // true
    console.log(f(false, true));  // true
    console.log(f(false, false)); // false
    console.log(f(true, true));   // true
}
