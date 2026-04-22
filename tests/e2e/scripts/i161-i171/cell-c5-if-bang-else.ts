// I-171 Layer 2 Cell C-5 (else branch): `if (!x) A; else B` on Option<F64>.
// TS: else branch narrows x to number (non-falsy).
// Current emission: fall-through naive `if !x { ... } else { ... }` → E0600.
// Ideal: consolidated match with else_body as truthy arm + then_body as wildcard arm:
//   match x { Some(v) if <truthy> => { else_body }, _ => { then_body } }.

function f(x: number | null): string {
    if (!x) {
        return "falsy";
    } else {
        return `truthy:${x + 1}`;
    }
}

function main(): void {
    console.log(f(null));  // falsy
    console.log(f(0));     // falsy
    console.log(f(5));     // truthy:6
}
