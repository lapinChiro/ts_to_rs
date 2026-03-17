async function fetchNumber(): Promise<number> {
  return 42;
}

async function fetchMessage(): Promise<string> {
  return "done";
}

async function noReturnType() {
  return;
}

async function main(): Promise<void> {
  const num = await fetchNumber();
  console.log(num);
}
