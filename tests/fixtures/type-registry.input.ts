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
