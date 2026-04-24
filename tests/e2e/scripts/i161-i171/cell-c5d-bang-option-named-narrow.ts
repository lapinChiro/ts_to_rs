// I-171 T5 deep-deep-deep-deep-fix lock-in (post-/check_job 4th-iteration
// deep deep review, 2026-04-24): Bang `!x` × Option<Named other> +
// Option<Vec> must materialise post-if narrow.
//
// PRD Matrix C-3 enumerates `Option<Named other>` early-return as
// "✓ T6-3" but the implementation in `build_option_truthy_match_arms`
// returned `None` for non-synthetic-union `Named` (interface / class /
// non-synthetic enum), falling back to Layer 1's predicate form which
// did NOT shadow `x` to the inner type post-if. Subsequent access to
// `x.field` / `x.method()` failed with E0609.
//
// The fix recognises always-truthy types (Named non-synthetic / Vec /
// Fn / Tuple / StdCollection / DynTrait / Ref) and emits a single
// `Some(x) => <body>` arm WITHOUT a truthy guard, materialising the
// narrow via shadow rebinding.

interface Tag { label: string; }

function build(s: string): Tag {
    return { label: s };
}

function f_named(x: Tag | null): string {
    if (!x) return "no";
    return x.label;  // post-narrow access; pre-fix: E0609 on Option<Tag>
}

function f_vec(x: number[] | null): number {
    if (!x) return -1;
    return x.length;  // post-narrow access; pre-fix: E0624 (private len on Option)
}

function f_named_else(x: Tag | null): string {
    if (!x) {
        return "no";
    } else {
        // non-exit else (EarlyReturnFromExitWithElse sub-case)
    }
    return x.label;  // post-narrow access in (T, F) sub-case
}

function main(): void {
    const t = build("hi");
    console.log(f_named(null));
    console.log(f_named(t));
    console.log(f_vec(null), f_vec([1, 2, 3]));
    console.log(f_named_else(build("yo")));
}
