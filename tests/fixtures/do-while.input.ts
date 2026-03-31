// do-while loops

// Basic countdown
function countdown(n: number): number {
  let x: number = n;
  do {
    x = x - 1;
  } while (x > 0);
  return x;
}

// With break
function findFirstNegative(values: number[]): number {
  let i = 0;
  let result = 0;
  do {
    if (values[i] < 0) {
      result = values[i];
      break;
    }
    i++;
  } while (i < values.length);
  return result;
}

// Nested do-while
function multiplyUntilOverflow(a: number, b: number, limit: number): number {
  let result = 1;
  let i = 0;
  do {
    result = result * a;
    let j = 0;
    do {
      result = result + b;
      j++;
    } while (j < 2);
    i++;
  } while (result < limit);
  return result;
}

// With continue
function sumPositiveOnly(values: number[]): number {
  let i = 0;
  let total = 0;
  do {
    if (values[i] < 0) {
      i++;
      continue;
    }
    total = total + values[i];
    i++;
  } while (i < values.length);
  return total;
}

// do-while with accumulation
function sumUntilLimit(limit: number): number {
  let result = 0;
  let i = 1;
  do {
    result = result + i;
    i++;
  } while (result < limit);
  return result;
}
