function roundDown(x: number): number {
  return Math.floor(x);
}

function roundUp(x: number): number {
  return Math.ceil(x);
}

function absolute(x: number): number {
  return Math.abs(x);
}

function bigger(a: number, b: number): number {
  return Math.max(a, b);
}

function power(x: number, y: number): number {
  return Math.pow(x, y);
}

function rootOf(x: number): number {
  return Math.sqrt(x);
}

// Math.min
function smaller(a: number, b: number): number {
  return Math.min(a, b);
}

// Math.round
function roundNearest(x: number): number {
  return Math.round(x);
}

// Math.PI constant
function circleArea(radius: number): number {
  return Math.PI * radius * radius;
}

// Multiple Math methods in one function
function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

// Math.sign
function getSign(x: number): number {
  return Math.sign(x);
}

// Math.trunc
function truncate(x: number): number {
  return Math.trunc(x);
}

// Math.log
function naturalLog(x: number): number {
  return Math.log(x);
}

// Math.random()
function randomValue(): number {
  return Math.random();
}
