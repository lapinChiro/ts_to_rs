// Mixed: combination of interfaces, classes, functions, type aliases, and enums

interface Point {
  x: number;
  y: number;
}

type Label = string;

enum Color {
  Red = "red",
  Green = "green",
  Blue = "blue",
}

class Circle {
  radius: number;

  constructor(radius: number) {
    this.radius = radius;
  }

  area(): number {
    return Math.PI * this.radius * this.radius;
  }
}

function distance(a: Point, b: Point): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}

function describePoint(p: Point): string {
  return `(${p.x}, ${p.y})`;
}

// Using multiple constructs together
function createLabeledCircle(
  label: Label,
  radius: number,
  color: Color
): string {
  const circle = new Circle(radius);
  return `${label}: ${color} circle with area ${circle.area()}`;
}

// Array of interface type
const points: Point[] = [
  { x: 0, y: 0 },
  { x: 1, y: 1 },
];
