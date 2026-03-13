function acceptAny(x: any): any {
  return x;
}

function acceptUnknown(x: unknown): void {
  return;
}

interface Flexible {
  data: any;
  value: unknown;
}
