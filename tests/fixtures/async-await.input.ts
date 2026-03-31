// Async/await patterns

async function fetchNumber(): Promise<number> {
  return 42;
}

async function fetchMessage(): Promise<string> {
  return "done";
}

async function noReturnType() {
  return;
}

// Sequential await
async function main(): Promise<void> {
  const num = await fetchNumber();
  console.log(num);
}

// Await chain: use result of one await in another
async function processData(): Promise<string> {
  const num = await fetchNumber();
  const msg = await fetchMessage();
  return `${msg}: ${num}`;
}

// Try/catch with await
async function safeFetch(): Promise<number> {
  try {
    const result = await fetchNumber();
    return result;
  } catch (e) {
    return -1;
  }
}

// Async function with parameters
async function delayedAdd(a: number, b: number): Promise<number> {
  const sum = a + b;
  return sum;
}

// Async function returning optional
async function maybeFetch(flag: boolean): Promise<string | null> {
  if (flag) {
    return await fetchMessage();
  }
  return null;
}
