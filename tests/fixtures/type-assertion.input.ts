// Type assertion patterns

function passThrough(x: any): any {
  return x as any;
}

function assertString(x: string): string {
  return x as string;
}

function assertNumber(x: number): number {
  return x as number;
}

// Assertion to a more specific type
function getLength(x: any): number {
  return (x as string).length;
}

// Assertion in variable declaration
function processValue(input: any): any {
  const str = input as string;
  return str;
}

// Double assertion (as unknown as T)
function forceConvert(x: number): string {
  return (x as unknown) as string;
}

// Assertion with union type
function narrowToString(x: string | number): string {
  return x as string;
}
