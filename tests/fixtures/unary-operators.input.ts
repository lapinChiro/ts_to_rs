function negate(x: boolean): boolean {
  return !x;
}

function abs(x: number): number {
  if (x < 0) {
    return -x;
  }
  return x;
}

function isInRange(x: number, min: number, max: number): boolean {
  return !(x < min) && !(x > max);
}
