// I-144 Cell T4e regression: `if (x)` on bare `boolean` → truthy predicate
// is identity (`x`). Locks in the primitive-Bool truthy path added in T6-3
// (previously the bare-primitive `if (x)` fell through to `if x { ... }`
// which works for Bool but not F64 / String — this test guards against a
// regression where the Bool special case is accidentally removed).
// TS runtime: check(true) → "yes"; check(false) → "no".

function check(x: boolean): string {
    if (x) {
        return "yes";
    }
    return "no";
}

function main(): void {
    console.log(check(true));
    console.log(check(false));
}
