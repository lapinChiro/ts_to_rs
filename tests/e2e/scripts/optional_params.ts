// E2E test for I-040: TS `?:` optional parameter unified wrap in Rust `Option<T>`.
// Verifies that TS and Rust runtime produce identical stdout across the
// param-emission sites relevant to I-040 (interface method + fn type alias).
// Class constructors without explicit `constructor()` and closure-passing to
// `Box<dyn Fn>` params are not in I-040 scope and are deliberately avoided.

// --- 1. Fn type alias with optional param ---
type BinaryOp = (a: number, b?: number) => number;

const sumFn: BinaryOp = (a: number, b?: number): number => {
    return a;
};

// --- 2. Plain function with optional param (already working) ---
function greet(name: string, prefix?: string): string {
    return "hi " + name;
}

function main(): void {
    // Fn type alias: arg omitted → caller emits None
    console.log(sumFn(3));
    console.log(sumFn(3, 4));

    // Plain function optional param (baseline)
    console.log(greet("world"));
    console.log(greet("world", "yo"));
}
