// Unary operators

// Logical NOT
function negate(x: boolean): boolean {
  return !x;
}

// Numeric negation
function abs(x: number): number {
  if (x < 0) {
    return -x;
  }
  return x;
}

// Double negation for range check
function isInRange(x: number, min: number, max: number): boolean {
  return !(x < min) && !(x > max);
}

// typeof operator (in condition)
function isString(x: any): boolean {
  return typeof x === "string";
}

// Bitwise NOT
function bitwiseComplement(x: number): number {
  return ~x;
}

// void operator
function consumeAndDiscard(x: number): void {
  void x;
}

// Unary plus (numeric coercion)
function toNumber(s: string): number {
  return +s;
}
