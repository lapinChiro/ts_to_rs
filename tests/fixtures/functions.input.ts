// Functions: various signatures and patterns

// Basic function with return value
function add(a: number, b: number): number {
  return a + b;
}

// Boolean return
function isPositive(n: number): boolean {
  return n > 0;
}

// Void return type
function logValue(value: string): void {
  console.log(value);
}

// Multiple return paths
function classify(n: number): string {
  if (n > 0) {
    return "positive";
  } else if (n < 0) {
    return "negative";
  }
  return "zero";
}

// Optional parameter
function greetUser(name: string, greeting?: string): string {
  if (greeting) {
    return `${greeting}, ${name}`;
  }
  return `Hello, ${name}`;
}

// Rest parameters
function sum(...nums: number[]): number {
  let total = 0;
  for (const n of nums) {
    total += n;
  }
  return total;
}

// Function returning function
function multiplier(factor: number): (x: number) => number {
  return (x: number): number => x * factor;
}

// Generic function
function identity<T>(value: T): T {
  return value;
}

// Function with array parameter and return
function first(items: string[]): string | undefined {
  return items[0];
}

// Early return pattern
function findIndex(arr: number[], target: number): number {
  for (let i = 0; i < arr.length; i++) {
    if (arr[i] === target) {
      return i;
    }
  }
  return -1;
}
