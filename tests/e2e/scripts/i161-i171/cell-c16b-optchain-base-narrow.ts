// I-171 Layer 2 Cell C-16b (OptChain base narrow after `if (!x?.v) return`).
// Base narrow: post-if, x is narrowed to non-null so `x.v` field access is valid.
// (Field narrow for x.v → non-null is I-165 scope; this fixture tests base only.)
//
// Ideal (post-T6 P3b guards.rs extension):
//   guards.rs detect_early_return_narrowing Bang arm detects OptChain operand,
//   extracts base ident `x`, pushes NarrowEvent::Narrow with OptChainInvariant
//   trigger for post-if scope. Subsequent `x.v ?? ""` emits validly because
//   `x` is narrowed from `Option<Wrap>` to `Wrap`.
//
// NOTE: post-if `x.v` is Option<string>, so we use `?? ""` to coerce to string.
// This isolates base narrow (in-scope) from field narrow (I-165, out-of-scope).

function f(x: { v: string | null } | null): string {
    if (!x?.v) return "none";
    return (x.v ?? "") + ":ok";
}

function main(): void {
    console.log(f(null));          // none
    console.log(f({ v: null }));   // none
    console.log(f({ v: "" }));     // none
    console.log(f({ v: "hi" }));  // hi:ok
}
