function sumArray(arr: number[]): number {
  return arr.reduce((acc: number, x: number) => acc + x, 0);
}

function findIndex(arr: number[], target: number): number {
  return arr.indexOf(target);
}

function joinWords(arr: string[], sep: string): string {
  return arr.join(sep);
}

function reverseArray(arr: number[]): void {
  arr.reverse();
}

function sortArray(arr: number[]): void {
  arr.sort();
}

function sortDescending(arr: number[]): void {
  arr.sort((a: number, b: number) => b - a);
}

function sliceArray(arr: number[]): number[] {
  return arr.slice(1, 3);
}

function spliceArray(arr: number[]): number[] {
  return arr.splice(1, 2);
}

function circleArea(r: number): number {
  return Math.PI * r * r;
}

function signOf(x: number): number {
  return Math.sign(x);
}

function truncate(x: number): number {
  return Math.trunc(x);
}

function naturalLog(x: number): number {
  return Math.log(x);
}

function isWholeNumber(x: number): boolean {
  return Number.isInteger(x);
}
