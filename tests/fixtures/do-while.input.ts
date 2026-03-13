function countdown(n: number): number {
  let x: number = n;
  do {
    x = x - 1;
  } while (x > 0);
  return x;
}
