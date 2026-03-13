function cloneArray(arr: number[]): number[] {
  return [...arr];
}

function appendToArray(arr: number[]): number[] {
  return [...arr, 4];
}

function prependToArray(arr: number[]): number[] {
  return [1, ...arr];
}

function surroundArray(arr: number[]): number[] {
  return [1, ...arr, 2];
}

function concatArrays(arr1: number[], arr2: number[]): number[] {
  return [...arr1, ...arr2];
}
