// General for loop patterns

// Decrementing
function countdown(n: number) {
  for (let i = n; i >= 0; i--) {
    console.log(i);
  }
}

// Step by two
function stepByTwo(n: number) {
  for (let i = 0; i < n; i += 2) {
    console.log(i);
  }
}

// With break
function findFirst(items: number[], target: number): number {
  let result = -1;
  for (let i = 0; i < items.length; i++) {
    if (items[i] === target) {
      result = i;
      break;
    }
  }
  return result;
}

// With continue
function sumEven(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    if (i % 2 !== 0) {
      continue;
    }
    total = total + i;
  }
  return total;
}

// Nested for loops
function multiplicationTable(n: number): number {
  let count = 0;
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= n; j++) {
      count++;
    }
  }
  return count;
}

// For loop computing sum of squares
function sumOfSquares(n: number): number {
  let result = 0;
  for (let i = 1; i <= n; i++) {
    result = result + i * i;
  }
  return result;
}
