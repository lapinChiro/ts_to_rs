// I-161 Cell A-7 (Option<Named other>): `x &&= y` on Option<interface>.
// JS: Some(obj) is truthy → assign; None is falsy → no assign.
// Ideal: `if x.is_some() { x = Some(y); }`.
//
// NOTE (Spec stage 2026-04-22): TS object literal `{ a: 99 }` as RHS currently
// emits as synthetic `_TypeLit0` type instead of `P` (pre-existing
// object-literal-to-Named inference gap, orthogonal to this PRD). To isolate
// the A-7 structural fix (is_some predicate + Some wrap), the fixture uses an
// explicit constructor helper `mkP` so the RHS type is exactly `P`. After T3,
// the emission should be `if x.is_some() { x = Some(mkP(99.0)); }`.

interface P { a: number }

function mkP(a: number): P { return { a }; }

function f(init: P | null): P | null {
    let x: P | null = init;
    x &&= mkP(99);
    return x;
}

function main(): void {
    const r1 = f(mkP(1));
    console.log(r1 === null ? "null" : r1.a); // 99
    const r2 = f(null);
    console.log(r2 === null ? "null" : r2.a); // null
}
