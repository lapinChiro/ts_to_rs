// I-161 Cell O-5s (Option<String> `||=`): `x ||= y` on nullable string.
// Ideal: `if x.as_ref().map_or(true, |v| v.is_empty()) { x = Some(y.to_string()); }`.

function f(init: string | null): string | null {
    let x: string | null = init;
    x ||= "default";
    return x;
}

function show(v: string | null): string {
    return v === null ? "null" : `"${v}"`;
}

function main(): void {
    console.log(show(f("hello"))); // "hello"
    console.log(show(f(null)));    // "default"
    console.log(show(f("")));      // "default"
}
