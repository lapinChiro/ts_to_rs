// I-171 Layer 2 Cell C-24 (always-truthy in if-cond): `if (![1, 2, 3]) { ... }`.
// Ideal: const-fold `!<always-truthy>` = false → else branch taken (or body skipped).

function f(): string {
    const arr = [1, 2, 3];
    if (!arr) {
        return "unreachable";
    }
    return "truthy";
}

function main(): void {
    console.log(f()); // truthy
}
