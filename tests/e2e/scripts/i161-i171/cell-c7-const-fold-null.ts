// I-171 Layer 2 Cell C-7 (const-fold `!null` in if-cond).
// Ideal: const-fold `if (!null)` → always-taken body (body inlined) / else skipped.

function f(): string {
    if (!null) {
        return "ok";
    }
    return "unreachable";
}

function main(): void {
    console.log(f()); // ok
}
