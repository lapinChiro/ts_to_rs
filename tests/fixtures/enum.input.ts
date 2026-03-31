// Numeric enum (auto-incrementing)
enum Color {
  Red,
  Green,
  Blue,
}

// Numeric enum (explicit values)
enum Status {
  Active = 1,
  Inactive = 0,
}

// String enum
enum Direction {
  Up = "UP",
  Down = "DOWN",
  Left = "LEFT",
  Right = "RIGHT",
}

// Export enum
export enum Visibility {
  Public,
  Private,
  Protected,
}

// Enum member access in functions
function isActive(s: Status): boolean {
  return s === Status.Active;
}

function getDirectionLabel(d: Direction): string {
  return `Going ${d}`;
}

// Enum as function parameter
function describeColor(c: Color): string {
  if (c === Color.Red) {
    return "red";
  } else if (c === Color.Green) {
    return "green";
  }
  return "blue";
}

// Const enum
const enum Priority {
  Low = 0,
  Medium = 1,
  High = 2,
}

function isHighPriority(p: Priority): boolean {
  return p === Priority.High;
}
