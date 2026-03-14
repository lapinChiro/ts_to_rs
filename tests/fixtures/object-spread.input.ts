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

function spreadAtEnd(p: Point): Point {
  const p2: Point = { x: 42, ...p };
  return p2;
}

interface Config {
  a: number;
  b: number;
  c: number;
}

function spreadInMiddle(cfg: Config): Config {
  const c2: Config = { a: 1, ...cfg, c: 3 };
  return c2;
}
