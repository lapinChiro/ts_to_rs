interface Item {
  name: string;
  value: number;
}

function propertyAccess(x: Item | null): number | null {
  return x?.value;
}

function computedAccess(x: number[] | null): number | null {
  return x?.[0];
}
