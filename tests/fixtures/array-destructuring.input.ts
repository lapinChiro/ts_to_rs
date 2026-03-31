// Array destructuring patterns

// Basic single element
function getFirst(arr: number[]): number {
  const [a] = arr;
  return a;
}

// Swap two elements
function swap(arr: number[]): number[] {
  const [a, b] = arr;
  return [b, a];
}

// Skip elements
function getThird(arr: number[]): number {
  const [, , c] = arr;
  return c;
}

// Rest element
function headAndTail(arr: number[]): number {
  const [head, ...rest] = arr;
  return head;
}

// With default values
function withDefaults(arr: number[]): number {
  const [a = 0, b = 1] = arr;
  return a + b;
}

// Mutable destructured variable
function incrementFirst(arr: number[]): number {
  let [x] = arr;
  x = x + 1;
  return x;
}
