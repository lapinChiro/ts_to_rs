// I-161 Cell O-3 (String `||=`): `x ||= y` on pure String.
// Ideal: `if x.is_empty() { x = y.to_string(); }`.

function f(init: string): string {
    let x: string = init;
    x ||= "default";
    return x;
}

function main(): void {
    console.log(f("hello")); // hello
    console.log(f(""));      // default
}
