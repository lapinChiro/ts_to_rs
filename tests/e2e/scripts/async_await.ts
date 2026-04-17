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

// I-023 try/catch + return — the labeled block is `!`-typed because the try body
// always returns and there is no explicit throw. Post-fix, the Rust output is a
// plain inline body without the `_try_result` / `'try_block` / `if let Err`
// machinery (which would otherwise trigger the `unreachable_code` lint).
async function safeFetch(): Promise<number> {
  try {
    const result: number = await fetchNumber();
    return result;
  } catch (e) {
    return -1;
  }
}

// I-023 regression: throw nested inside a switch arm must be rewritten into
// `_try_result = Err; break 'try_block`, not left as raw `return Err(...)`.
// Pre-fix, `TryBodyRewrite::rewrite` did not recurse into `Stmt::Match`, so
// hidden throws escaped the rewrite and the I-023 short-circuit silently
// dropped the catch body. This fixture locks the correct behaviour in.
async function guardedByKind(flag: number): Promise<number> {
  try {
    switch (flag) {
      case 1:
        if (flag < 0) throw new Error("never-fires");
        return 100;
      default:
        return 200;
    }
  } catch (e) {
    return -1;
  }
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

  // I-023 try/catch + return — verifies post-fix runtime equivalence
  const safe: number = await safeFetch();
  console.log("safe:", safe);

  // I-023 regression: throw nested in switch arm — expected stdout is 100 and 200.
  const g1: number = await guardedByKind(1);
  const g2: number = await guardedByKind(2);
  console.log("guarded:", g1, g2);
}
