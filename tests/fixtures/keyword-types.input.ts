// Keyword types: any, unknown, never, void, undefined

function acceptAny(x: any): any {
  return x;
}

function acceptUnknown(x: unknown): void {
  return;
}

// never as return type (function that never returns)
function fail(message: string): never {
  throw new Error(message);
}

// void function
function doNothing(): void {
  // no return
}

// void with explicit return
function doNothingExplicit(): void {
  return;
}

// undefined as a type
function returnsUndefined(): undefined {
  return undefined;
}

interface Flexible {
  data: any;
  value: unknown;
}

// any and unknown in variable declarations
const anyVal: any = 42;
const unknownVal: unknown = "hello";

// any in array
const anyArray: any[] = [1, "two", true];

// Function taking never (useful for exhaustiveness checks)
function assertNever(x: never): never {
  throw new Error("Unexpected value: " + x);
}
