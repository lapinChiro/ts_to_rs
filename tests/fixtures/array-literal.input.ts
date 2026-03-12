// Array literal in function return
function getNumbers(): number[] {
  return [10, 20, 30];
}

// Empty array
function emptyArray(): number[] {
  return [];
}

// Nested array
function matrix(): number[][] {
  return [[1, 2], [3, 4]];
}

// Array with expressions
function computed(x: number): number[] {
  return [x, x + 1];
}
