async function fetchNumber(): Promise<number> {
  return 42;
}

async function fetchMessage(): Promise<string> {
  return "hello async";
}

async function doubleAsync(n: number): Promise<number> {
  return n * 2;
}

async function chainedCall(): Promise<string> {
  const num: number = await fetchNumber();
  const doubled: number = await doubleAsync(num);
  return `result: ${doubled}`;
}

async function main(): Promise<void> {
  // Basic await
  const num: number = await fetchNumber();
  console.log("num:", num);

  // String return
  const msg: string = await fetchMessage();
  console.log("msg:", msg);

  // Chained async calls
  const chained: string = await chainedCall();
  console.log("chained:", chained);

  // Multiple sequential awaits
  const a: number = await fetchNumber();
  const b: number = await doubleAsync(a);
  const c: number = await doubleAsync(b);
  console.log("sequential:", a, b, c);
}
