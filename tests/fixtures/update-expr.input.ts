// Update expressions: postfix and prefix increment/decrement

// Postfix increment
function incrementCounter(initial: number): number {
  let count: number = initial;
  count++;
  return count;
}

// Postfix decrement
function decrementCounter(initial: number): number {
  let count: number = initial;
  count--;
  return count;
}

// In while loop
function consumeWhitespace(s: string, start: number): number {
  let startIndex: number = start;
  while (startIndex < s.length) {
    startIndex++;
  }
  return startIndex;
}

// Prefix increment
function preIncrement(initial: number): number {
  let count: number = initial;
  ++count;
  return count;
}

// Prefix decrement
function preDecrement(initial: number): number {
  let count: number = initial;
  --count;
  return count;
}

// Increment in for loop
function sumRange(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    total = total + i;
  }
  return total;
}

// Expression use: postfix increment in index
function readSequence(arr: number[], count: number): number {
  let i = 0;
  let total = 0;
  while (i < count) {
    total = total + arr[i++];
  }
  return total;
}

// Decrement in loop condition
function countDown(start: number): number {
  let count = start;
  while (count > 0) {
    count--;
  }
  return count;
}
