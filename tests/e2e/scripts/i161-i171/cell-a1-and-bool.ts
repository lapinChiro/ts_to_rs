// I-161 Cell A-1 (Bool): `x &&= y` on pure Bool.
// JS: true &&= true → true; true &&= false → false; false &&= true → false.
// Ideal: `if x { x = y; }` (Bool native truthy).

function f(init: boolean, rhs: boolean): boolean {
    let x: boolean = init;
    x &&= rhs;
    return x;
}

function main(): void {
    console.log(f(true, true));   // true
    console.log(f(true, false));  // false
    console.log(f(false, true));  // false
    console.log(f(false, false)); // false
}
