// Basic ternary
function abs(a: number): number {
    const result = a > 0 ? a : 0;
    return result;
}

// Ternary with string literals
function sign(x: number): string {
    return x > 0 ? "positive" : "negative";
}

// Nested ternary
function signOrZero(x: number): string {
    return x > 0 ? "positive" : x < 0 ? "negative" : "zero";
}

// Ternary in function argument
function pick(flag: boolean, a: number, b: number): number {
    return flag ? a : b;
}
