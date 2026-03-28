// T4: Ternary operator union generation
// Tests that ternary with different branch types produces a union
// instead of Unknown.

// S9: Different types should produce union
function getStringOrNumber(flag: boolean): string | number {
    return flag ? "hello" : 42;
}

// Same type should NOT produce union
function getEitherString(flag: boolean): string {
    return flag ? "yes" : "no";
}
