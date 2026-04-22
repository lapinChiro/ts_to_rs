// I-161 Cell A-3 (String non-null): `x &&= y` on pure String.
// JS: "hello" &&= "world" → "world"; "" &&= "world" → "" (empty string is falsy).
// Ideal: `if !x.is_empty() { x = "world".to_string(); }`.

function f(init: string): string {
    let x: string = init;
    x &&= "world";
    return x;
}

function main(): void {
    console.log(f("hello")); // "world"
    console.log(f(""));      // ""
}
