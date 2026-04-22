// I-171 Layer 2 Cell C-15 (Member LHS, always-exit, narrow 材料化なし):
// `if (!u.v) return <val>;` on Option<String>.
// Current emission: `if !u.v { return ... }` → E0600 on Option<String>.
// Ideal (本 PRD scope): `if u.v.is_none() { return ... }`.
// Out-of-scope (I-165): post-if `u.v` references narrow materialization.
//
// To make the E2E runnable without depending on I-165, the post-if use is
// wrapped with `?? ""` (explicit defaulting) so we don't rely on narrow
// materialization. This validates the Layer 1 fix at the `!u.v` site.

function f(u: { v: string | null }): string {
    if (!u.v) return "none";
    return u.v ?? "";
}

function main(): void {
    console.log(f({ v: null })); // none
    console.log(f({ v: "" }));   // none
    console.log(f({ v: "hi" })); // hi
}
