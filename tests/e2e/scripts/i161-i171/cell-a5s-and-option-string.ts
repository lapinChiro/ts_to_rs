// I-161 Cell A-5s (Option<String> without narrow): `x &&= y` on Option<String>.
// JS: Some("hello") &&= "world" → Some("world"); None &&= ... → None; Some("") &&= ... → Some("").
// Ideal: `if x.as_ref().is_some_and(|v| !v.is_empty()) { x = Some("world".to_string()); }`.

function f(init: string | null): string | null {
    let x: string | null = init;
    x &&= "world";
    return x;
}

function show(v: string | null): string {
    return v === null ? "null" : `"${v}"`;
}

function main(): void {
    console.log(show(f("hello"))); // "world"
    console.log(show(f(null)));    // null
    console.log(show(f("")));      // ""
}
