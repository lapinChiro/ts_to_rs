// Function call patterns

function square(n: number): number {
  return n * n;
}

function sumOfSquares(a: number, b: number): number {
  return square(a) + square(b);
}

export function main(): number {
  const result = sumOfSquares(3, 4);
  return result;
}

// Method chain on string
function processString(s: string): string {
  return s.trim().toLowerCase();
}

// Recursive call
function factorial(n: number): number {
  if (n <= 1) {
    return 1;
  }
  return n * factorial(n - 1);
}

// Nested function calls
function compose(x: number): number {
  return square(square(x));
}

// Call with computed arguments
function calculate(a: number, b: number): number {
  return sumOfSquares(a + 1, b * 2);
}
