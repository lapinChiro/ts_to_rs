function withDefault(value: number | undefined, fallback: number): number {
    return value ?? fallback;
}

function stringFallback(s: string | undefined): string {
    return s ?? "default";
}

// I-022: Vec index LHS — must return default when array is empty (not panic).
function arrayIndexWithDefault(arr: string[], i: number, def: string): string {
    return arr[i] ?? def;
}

// I-022: Chain `a ?? b ?? c` — inner ?? preserves Option via .or_else, outer terminates.
function chainNullish(a: string | null, b: string | null, c: string): string {
    return a ?? b ?? c;
}

// I-022: Vec<Option<T>> + NC — `.flatten()` must fire so empty array does not panic.
// Pre-I-022 emitted `.unwrap()` here (panic on empty array, dropping "default").
function vecOfOptionWithDefault(items: (string | null)[], i: number, def: string): string {
    return items[i] ?? def;
}

// I-022: Chain + Vec<Option<T>> — Inner ?? RHS must also use `.flatten()`
// (not `.unwrap()`) so chain short-circuits to terminal default on empty array.
// Pre-review bug: inner RHS was emitted as `.unwrap()` under chain expected,
// panicking before reaching "default".
function chainVecOfOptionWithDefault(
    items: (string | null)[],
    i: number,
    j: number,
    def: string,
): string {
    return items[i] ?? items[j] ?? def;
}

function main(): void {
    console.log("has value:", withDefault(42, 0));
    console.log("direct:", 10 ?? 20);
    console.log("string fallback:", stringFallback(undefined));
    console.log("string present:", stringFallback("hello"));

    // Vec index + default (I-022: silent drop regression test)
    console.log("empty array default:", arrayIndexWithDefault([], 0, "missing"));
    console.log("in-bound index:", arrayIndexWithDefault(["a", "b"], 0, "missing"));
    console.log("out-of-bound index:", arrayIndexWithDefault(["a", "b"], 5, "missing"));

    // Chain (I-022: compile error regression test)
    console.log("chain all nullish:", chainNullish(null, null, "fallback"));
    console.log("chain first present:", chainNullish("a", null, "fallback"));
    console.log("chain middle present:", chainNullish(null, "b", "fallback"));

    // Vec<Option<T>> + NC (I-022: empty + null-element regression test)
    console.log("vec<opt> empty:", vecOfOptionWithDefault([], 0, "miss"));
    console.log("vec<opt> null at index:", vecOfOptionWithDefault([null, "x"], 0, "miss"));
    console.log("vec<opt> present:", vecOfOptionWithDefault(["a", null], 0, "miss"));
    console.log("vec<opt> oob:", vecOfOptionWithDefault([null], 5, "miss"));

    // Chain + Vec<Option<T>> (I-022 /check_job deep finding: inner RHS panic)
    console.log("chain vec empty:", chainVecOfOptionWithDefault([], 0, 0, "miss"));
    console.log("chain vec first:", chainVecOfOptionWithDefault(["a", "b"], 0, 1, "miss"));
    console.log("chain vec i-null:", chainVecOfOptionWithDefault([null, "b"], 0, 1, "miss"));
    console.log("chain vec both-null:", chainVecOfOptionWithDefault([null, null], 0, 1, "miss"));
    console.log("chain vec oob:", chainVecOfOptionWithDefault([null], 5, 6, "miss"));
}
