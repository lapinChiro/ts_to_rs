// Basic type annotations: primitives, compound types, special types

// Interface with various field types
interface User {
  name: string;
  age: number;
  active: boolean;
  tags: string[];
  metadata: Record<string, string>;
}

// Nullable and optional types in interface fields
interface Config {
  host: string;
  port: number | null;
  label: string | undefined;
  timeout?: number;
}

// Tuple types in function signatures
function swap(pair: [string, number]): [number, string] {
  return [pair[1], pair[0]];
}

// Literal union types
type Direction = "north" | "south" | "east" | "west";

function describeDirection(d: Direction): string {
  return `Going ${d}`;
}

// void and never in function signatures
function logMessage(msg: string): void {
  console.log(msg);
}

function throwError(msg: string): never {
  throw new Error(msg);
}

// Array types
function processNumbers(nums: number[]): number {
  let total = 0;
  for (const n of nums) {
    total += n;
  }
  return total;
}

function processNames(names: Array<string>): number {
  return names.length;
}

// unknown type in function parameter
function processUnknown(x: unknown): string {
  return typeof x === "string" ? x : "unknown";
}

// Type alias for union
type ID = string | number;
