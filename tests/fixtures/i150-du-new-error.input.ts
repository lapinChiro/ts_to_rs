// I-150: `new UnknownClass(arg)` must visit `arg` so that expr_types is
// populated. In no-builtin mode (`transpile_collecting` via compile_test),
// `Error` is unregistered → `resolve_new_expr` takes the else branch. Without
// I-150 fix, `s.tag` inside `new Error(s.tag)` would not be resolved as a DU
// field binding, and member_access.rs would emit raw `s.tag` → rustc error
// E0609 "no field on type Shape".
//
// This fixture uses string-typed DU field to isolate I-150 from unrelated
// string+numeric concat issues (I-035).

type Shape =
    | { kind: "alpha"; tag: string }
    | { kind: "beta"; label: string };

export function describe(s: Shape): string {
    switch (s.kind) {
        case "alpha":
            if (s.tag === "danger") {
                throw new Error(s.tag);
            }
            return s.tag;
        case "beta":
            return s.label;
    }
}
