// I-144 Cell T4c regression: `if (x)` on bare `string` → truthy predicate
// `!x.is_empty()`. Locks in the primitive-String truthy path added in T6-3
// (previously the bare-primitive `if (x)` fell through to `if x { ... }`
// which fails Rust type checking).
// TS runtime: report("hello") → "nonempty:hello"; report("") → "empty".

function report(x: string): string {
    if (x) {
        return "nonempty:" + x;
    }
    return "empty";
}

function main(): void {
    console.log(report("hello"));
    console.log(report(""));
    console.log(report("a"));
}
