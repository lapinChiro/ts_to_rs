// Closures: arrow functions and function expressions with variable capture

// Basic arrow function (no capture)
export const double = (x: number): number => x * 2;

export const greet = (name: string): string => {
  return `Hello, ${name}`;
};

export const getFortyTwo = (): number => 42;

// Read capture: closure reads outer variable inside function
function useGreeting(greeting: string, name: string): string {
  const greetFn = (n: string): string => `${greeting}, ${n}`;
  return greetFn(name);
}

// Mutable capture: closure modifies outer variable
function makeCounter(): () => number {
  let count = 0;
  return (): number => {
    count += 1;
    return count;
  };
}

// Multiple variable capture inside function
function makeLabel(base: number, label: string, x: number): string {
  const format = (v: number): string => `${label}: ${base + v}`;
  return format(x);
}

// Higher-order function: takes function parameter
function applyTwice(f: (x: number) => number, value: number): number {
  return f(f(value));
}

// Interface with function type fields
export interface Processor {
  transform: (x: number) => number;
  callback: (a: string, b: number) => boolean;
}

// Closure returning closure (function factory)
function makeGreeter(greeting: string): (name: string) => string {
  return (name: string): string => `${greeting}, ${name}`;
}

// Closure in higher-order method context
function processValues(values: number[]): number[] {
  return values.map((x: number): number => x * 2);
}

// Filter with closure (captures threshold)
function filterAbove(values: number[], threshold: number): number[] {
  return values.filter((x: number): boolean => x > threshold);
}
