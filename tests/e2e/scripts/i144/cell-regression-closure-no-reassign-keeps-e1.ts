// I-144 regression lock-in (negative): closure captures outer narrowed var
// without reassigning it. Narrow must stay alive → E1 shadow-let / direct T
// binding remains the ideal emission. This guards against over-migrating every
// closure-present case to E2.
// TS runtime: 10.

function f(): number {
    let x: number | null = 5;
    if (x === null) return -1;
    const read = () => x + 5;
    return read();
}

function main(): void {
    console.log(f());
}
