function toInt(s: string): number {
  return parseInt(s);
}

function toFloat(s: string): number {
  return parseFloat(s);
}

function checkNaN(x: number): boolean {
  return isNaN(x);
}

function checkFinite(x: number): boolean {
  return Number.isFinite(x);
}
