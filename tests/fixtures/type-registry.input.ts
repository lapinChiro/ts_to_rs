// Type registry: types referencing other registered types

interface Origin {
  x: number;
  y: number;
}

interface Size {
  w: number;
  h: number;
}

interface Rect {
  origin: Origin;
  size: Size;
}

enum Color {
  Red,
  Green,
  Blue,
}

function drawPoint(p: Origin): boolean {
  return true;
}

function createRect(): Rect {
  const r: Rect = { origin: { x: 0, y: 0 }, size: { w: 10, h: 20 } };
  return r;
}

function useEnum(): number {
  const c: Color = Color.Red;
  return 0;
}

function callWithObject(): boolean {
  drawPoint({ x: 5, y: 10 });
  return true;
}

// Generic type registration
interface Container<T> {
  value: T;
  label: string;
}

function wrapValue(v: number): Container<number> {
  return { value: v, label: "num" };
}

// Cross-referencing types
interface Line {
  start: Origin;
  end: Origin;
}

function lineLength(l: Line): number {
  const dx = l.end.x - l.start.x;
  const dy = l.end.y - l.start.y;
  return dx + dy;
}
