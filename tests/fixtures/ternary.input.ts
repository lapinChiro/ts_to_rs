// Ternary expression patterns

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

// Ternary with different types (string vs number would be a union)
function formatValue(x: number): string {
  return x > 0 ? `+${x}` : `${x}`;
}

// Ternary in variable assignment
function getLabel(active: boolean): number {
  const label = active ? 1 : 0;
  return label;
}

// Ternary with function calls
function max(a: number, b: number): number {
  return a > b ? a : b;
}

// Ternary with null
function maybeValue(flag: boolean): string | null {
  return flag ? "value" : null;
}

// Ternary with different types in branches (union result)
function toDisplay(x: number): string | number {
  return x > 0 ? x : "negative";
}
