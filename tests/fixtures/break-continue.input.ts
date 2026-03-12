function findFirst(items: number[], target: number): number {
  let result = 0;
  for (const item of items) {
    if (item === target) {
      result = item;
      break;
    }
  }
  return result;
}

function nestedSearch(matrix: number[][]): boolean {
  let found = false;
  outer: for (const row of matrix) {
    for (const cell of row) {
      if (cell === 0) {
        found = true;
        break outer;
      }
    }
  }
  return found;
}

function labeledWhile(): number {
  let i = 0;
  outer: while (i < 10) {
    let j = 0;
    while (j < 10) {
      if (i + j > 15) {
        break outer;
      }
      j = j + 1;
    }
    i = i + 1;
  }
  return i;
}

function skipValues(items: number[]): number {
  let sum = 0;
  for (const item of items) {
    if (item === 0) {
      continue;
    }
    sum = sum + item;
  }
  return sum;
}

function labeledContinue(matrix: number[][]): number {
  let count = 0;
  outer: for (const row of matrix) {
    for (const cell of row) {
      if (cell < 0) {
        continue outer;
      }
    }
    count = count + 1;
  }
  return count;
}
