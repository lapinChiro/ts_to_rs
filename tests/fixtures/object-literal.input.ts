interface Point {
  x: number;
  y: number;
}

interface Config {
  count: number;
  active: boolean;
}

function createPoint(): Point {
  const p: Point = { x: 1, y: 2 };
  return p;
}

function createConfig(): Config {
  const c: Config = { count: 0, active: true };
  return c;
}
