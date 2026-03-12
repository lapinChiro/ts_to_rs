export function countdown(n: number): number {
  let count: number = n;
  while (count > 0) {
    count = count - 1;
  }
  return count;
}

export function sumArray(items: number[]): number {
  let total: number = 0;
  for (const item of items) {
    total = total + item;
  }
  return total;
}

export function repeatN(n: number): number {
  let count: number = 0;
  for (let i = 0; i < n; i++) {
    count = count + 1;
  }
  return count;
}

export function countFive(): number {
  let count: number = 0;
  for (let i = 0; i < 5; i += 1) {
    count = count + 1;
  }
  return count;
}
