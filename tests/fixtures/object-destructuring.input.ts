// Object destructuring patterns

interface Point {
  x: number;
  y: number;
}

// Basic destructuring
function getCoords(p: Point): number {
  const { x, y } = p;
  return x + y;
}

// Mutable destructured variables
function getMutableCoords(p: Point): number {
  let { x, y } = p;
  x = x + 1;
  return x + y;
}

// Rename during destructuring
function getRenamedCoord(p: Point): number {
  const { x: posX } = p;
  return posX;
}

// Default values
interface Options {
  width: number;
  height: number;
  color?: string;
}

function getColor(opts: Options): string {
  const { color = "black" } = opts;
  return color;
}

// Nested destructuring
interface Rect {
  origin: Point;
  size: { width: number; height: number };
}

function getOriginX(rect: Rect): number {
  const {
    origin: { x },
  } = rect;
  return x;
}

// Destructuring in function parameters
function distanceFromOrigin({ x, y }: Point): number {
  return x + y;
}

// Rest pattern
interface FullUser {
  name: string;
  age: number;
  email: string;
}

function getName(user: FullUser): string {
  const { name, ...rest } = user;
  return name;
}
