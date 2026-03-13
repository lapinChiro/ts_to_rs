interface Point {
  x: number;
  y: number;
}

function overrideX(p: Point): Point {
  const p2: Point = { ...p, x: 10 };
  return p2;
}

function overrideMultiple(p: Point): Point {
  const p2: Point = { ...p, x: 10, y: 20 };
  return p2;
}

function clonePoint(p: Point): Point {
  const p2: Point = { ...p };
  return p2;
}
